use crate::{FontId, GlyphId, Pixels, PlatformTextSystem, Point, SharedString, Size, point, px};
use collections::FxHashMap;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use smallvec::SmallVec;
use std::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use super::LineWrapper;
use unicode_segmentation::UnicodeSegmentation;

/// A laid out and styled line of text
#[derive(Default, Debug)]
pub struct LineLayout {
    /// The font size for this line
    pub font_size: Pixels,
    /// The width of the line
    pub width: Pixels,
    /// The ascent of the line
    pub ascent: Pixels,
    /// The descent of the line
    pub descent: Pixels,
    /// The shaped runs that make up this line
    pub runs: Vec<ShapedRun>,
    /// Native insertion positions, ordered by UTF-8 source index.
    pub carets: Vec<TextCaret>,
    /// Grapheme cells, ordered by UTF-8 source index, with logical leading/trailing edges.
    pub cells: Vec<TextCell>,
    /// Resolved bidi levels per source byte, retained across soft line breaks.
    pub bidi_levels: Vec<u8>,
    /// Independently shaped physical rows, present when wrapping reshapes the text.
    pub visual_lines: Vec<TextFragment>,
    /// The paragraph's base writing direction.
    pub rtl: bool,
    /// The length of the line in utf-8 bytes
    pub len: usize,
}

/// The two visual insertion positions at a bidirectional text boundary.
#[derive(Clone, Debug)]
pub struct TextCaret {
    /// A boundary in UTF-8 source bytes.
    pub index: usize,
    /// The insertion position used for the text cursor.
    pub x: Pixels,
    /// The other edge at a change of writing direction.
    pub secondary_x: Pixels,
}

/// Which visual edge to use at a bidirectional insertion boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAffinity {
    /// The primary insertion edge.
    #[default]
    Primary,
    /// The alternate insertion edge at a direction change.
    Secondary,
    /// Browser editing's primary insertion edge at a bidi run boundary.
    Editing,
}

/// A selectable grapheme, including its visual writing direction.
#[derive(Clone, Debug)]
pub struct TextCell {
    /// The grapheme's UTF-8 source range.
    pub range: Range<usize>,
    /// Its logical leading edge.
    pub start_x: Pixels,
    /// Its logical trailing edge.
    pub end_x: Pixels,
}

/// A independently shaped physical row within a wrapped paragraph.
#[derive(Clone, Debug)]
pub struct TextFragment {
    /// The row's UTF-8 source range.
    pub range: Range<usize>,
    /// Its origin in the concatenated line coordinate system.
    pub x: Pixels,
    /// Its actual shaped width, including whitespace.
    pub width: Pixels,
}

impl TextFragment {
    /// The physical row offset for its alignment and inherited paragraph direction.
    pub fn alignment_offset(&self, align: crate::TextAlign, width: Pixels, rtl: bool) -> Pixels {
        use crate::TextAlign;
        match align {
            TextAlign::Left => px(0.),
            TextAlign::Right => width - self.width,
            TextAlign::Center => (width - self.width) / 2.,
            TextAlign::Start => {
                if rtl {
                    width - self.width
                } else {
                    px(0.)
                }
            }
            TextAlign::End => {
                if rtl {
                    px(0.)
                } else {
                    width - self.width
                }
            }
        }
    }
}

/// A run of text that has been shaped .
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// Actual size used to shape and paint this run.
    pub font_size: Pixels,
    /// The font id for this run
    pub font_id: FontId,
    /// The glyphs that make up this run
    pub glyphs: Vec<ShapedGlyph>,
}

/// A single glyph, ready to paint.
#[derive(Clone, Debug)]
pub struct ShapedGlyph {
    /// The ID for this glyph, as determined by the text system.
    pub id: GlyphId,

    /// The position of this glyph in its containing line.
    pub position: Point<Pixels>,

    /// Drawing displacement from the logical position used for caret and selection.
    pub paint_offset: Point<Pixels>,
    /// Whether native ink displacement has already been separated from its advance.
    pub position_is_advance: bool,

    /// The index of this glyph in the original text.
    pub index: usize,

    /// Whether this glyph is an emoji
    pub is_emoji: bool,
}

struct EditingRun {
    source: Range<usize>,
    level: u8,
    left: Pixels,
    right: Pixels,
}

impl EditingRun {
    fn rtl(&self) -> bool {
        self.level % 2 != 0
    }
    fn edge(&self, left: bool) -> (usize, Pixels) {
        (
            if left != self.rtl() {
                self.source.start
            } else {
                self.source.end
            },
            if left { self.left } else { self.right },
        )
    }
}

impl LineLayout {
    pub(super) fn apply_letter_spacing(&mut self, text: &str, runs: &[FontRun]) {
        if !runs.iter().any(|run| run.letter_spacing != px(0.)) {
            return;
        }
        let mut end = 0;
        let ends = runs
            .iter()
            .map(|run| {
                end += run.len;
                end
            })
            .collect::<Vec<_>>();
        let mut cells = self.cells.clone();
        let mut visual = (0..cells.len()).collect::<Vec<_>>();
        visual.sort_by_key(|index| cells[*index].start_x.min(cells[*index].end_x));
        let mut offsets = vec![px(0.); cells.len()];
        let mut offset = px(0.);
        for index in visual {
            let cell = &mut cells[index];
            let source = ends.partition_point(|end| *end <= cell.range.start);
            // Formatting controls and cursive letters do not receive tracking.
            // Adding gaps to Arabic letters would break their joining forms.
            let spacing = if text[cell.range.clone()].chars().all(|character| {
                use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
                matches!(
                    character.general_category(),
                    GeneralCategory::Control
                        | GeneralCategory::Format
                        | GeneralCategory::NonspacingMark
                ) || unicode_bidi::bidi_class(character) == unicode_bidi::BidiClass::AL
            }) {
                px(0.)
            } else {
                runs[source].letter_spacing
            };
            offsets[index] = offset;
            if cell.start_x <= cell.end_x {
                cell.start_x += offset;
                cell.end_x += offset + spacing;
            } else {
                cell.start_x += offset + spacing;
                cell.end_x += offset;
            }
            offset += spacing;
        }
        for run in &mut self.runs {
            for glyph in &mut run.glyphs {
                let cell = cells.partition_point(|cell| cell.range.end <= glyph.index);
                if let Some(offset) = offsets.get(cell) {
                    glyph.position.x += *offset;
                }
            }
        }
        self.width += offset;
        self.set_cells(&text, cells);
    }

    /// The insertion position for an index and its visual affinity.
    pub fn caret_x(&self, index: usize, affinity: TextAffinity) -> Pixels {
        if affinity == TextAffinity::Editing {
            return self.editing_caret(index).1;
        }
        if affinity == TextAffinity::Secondary
            && let Ok(at) = self
                .carets
                .binary_search_by_key(&index, |caret| caret.index)
        {
            return self.carets[at].secondary_x;
        }
        self.x_for_index(index)
    }

    fn editing_runs(&self) -> Vec<EditingRun> {
        let mut runs: Vec<EditingRun> = Vec::new();
        for cell in &self.cells {
            let level = self
                .bidi_levels
                .get(cell.range.start)
                .copied()
                .unwrap_or(u8::from(self.rtl));
            let left = cell.start_x.min(cell.end_x);
            let right = cell.start_x.max(cell.end_x);
            if let Some(last) = runs.last_mut().filter(|run| {
                run.level == level
                    && run.source.end == cell.range.start
                    && (run.edge(run.rtl()).1 - cell.start_x).abs() < px(0.01)
            }) {
                last.source.end = cell.range.end;
                last.left = last.left.min(left);
                last.right = last.right.max(right);
            } else {
                runs.push(EditingRun {
                    source: cell.range.clone(),
                    level,
                    left,
                    right,
                });
            }
        }
        runs.sort_by_key(|run| run.left);
        runs
    }

    // Chromium 150's legacy editing adjustment traverses resolved physical runs.
    // Pointer hit adjustment and primary caret resolution differ at outer edges.
    fn editing_edge(&self, runs: &[EditingRun], at: usize, left: bool, hit: bool) -> (usize, bool) {
        let step = if left { 1 } else { -1 };
        let neighbor =
            |at: usize, step: isize| at.checked_add_signed(step).filter(|at| *at < runs.len());
        let level = runs[at].level;
        let backwards = neighbor(at, -step);
        let walk = |mut at: usize, step, min_level, strict| {
            while let Some(next) = neighbor(at, step) {
                if runs[next].level < min_level || strict && runs[next].level == min_level {
                    break;
                }
                at = next;
            }
            at
        };
        if runs[at].rtl() == self.rtl {
            if hit {
                return (at, left);
            }
            let Some(back) = backwards else {
                return (at, left);
            };
            if runs[back].level >= level || left && runs[back].rtl() == runs[at].rtl() {
                return (at, left);
            }
            let target_level = runs[back].level;
            let mut forward = neighbor(at, step);
            while let Some(next) = forward {
                if runs[next].level <= target_level {
                    break;
                }
                forward = neighbor(next, step);
            }
            if forward.is_some_and(|next| runs[next].level == target_level) {
                return (at, left);
            }
            return (walk(at, -step, target_level, false), left);
        }
        match backwards {
            Some(back) if runs[back].level == level => (at, left),
            Some(back) if runs[back].level > level => {
                let boundary = walk(if hit { back } else { at }, -step, level, true);
                (boundary, if hit { !left } else { left })
            }
            _ => {
                let boundary = walk(at, step, level, false);
                (
                    boundary,
                    if hit && runs[boundary].rtl() != runs[at].rtl() {
                        left
                    } else {
                        !left
                    },
                )
            }
        }
    }

    /// The browser editing insertion position, including bidi boundary adjustment.
    pub fn editing_caret(&self, index: usize) -> (usize, Pixels) {
        let runs = self.editing_runs();
        let at = runs
            .iter()
            .position(|run| run.source.contains(&index) || run.source.end == index);
        let Some(at) = at else {
            return (index, self.x_for_index(index));
        };
        let run = &runs[at];
        if index != run.source.start && index != run.source.end {
            return (index, self.x_for_index(index));
        }
        let left = (index == run.source.start) != run.rtl();
        let (at, left) = self.editing_edge(&runs, at, left, false);
        runs[at].edge(left)
    }

    /// Home/End resolve the paragraph's leading/trailing physical leaf.
    pub fn editing_line_boundary(&self, end: bool) -> usize {
        let runs = self.editing_runs();
        if runs.is_empty() {
            return 0;
        }
        let left = self.rtl == end;
        let at = if left { 0 } else { runs.len() - 1 };
        let (at, left) = self.editing_edge(&runs, at, left, true);
        runs[at].edge(left).0
    }

    /// Vertical navigation uses browser hit adjustment at the target row.
    pub fn editing_index_for_x(&self, x: Pixels) -> usize {
        let (index, affinity) = self.caret_for_x(x);
        let runs = self.editing_runs();
        let edge = self.caret_x(index, affinity);
        let at = runs
            .iter()
            .position(|run| {
                run.source.start == index && (run.edge(!run.rtl()).1 - edge).abs() < px(0.01)
            })
            .or_else(|| {
                runs.iter().position(|run| {
                    run.source.end == index && (run.edge(run.rtl()).1 - edge).abs() < px(0.01)
                })
            });
        let Some(at) = at else {
            return index;
        };
        let left = (index == runs[at].source.start) != runs[at].rtl();
        let (at, left) = self.editing_edge(&runs, at, left, true);
        runs[at].edge(left).0
    }

    fn affinity_at(&self, index: usize, x: Pixels) -> TextAffinity {
        if (self.caret_x(index, TextAffinity::Secondary) - x).abs()
            < (self.caret_x(index, TextAffinity::Primary) - x).abs()
        {
            TextAffinity::Secondary
        } else {
            TextAffinity::Primary
        }
    }

    /// The closest insertion edge, preserving which side of a bidi boundary was hit.
    pub fn caret_for_x(&self, x: Pixels) -> (usize, TextAffinity) {
        if let Some(cell) = self.cells.iter().find(|cell| {
            x >= cell.start_x.min(cell.end_x)
                && x <= cell.start_x.max(cell.end_x)
                && (cell.end_x - cell.start_x).abs() > px(0.001)
        }) {
            let (index, edge) = if (x - cell.start_x).abs() < (x - cell.end_x).abs() {
                (cell.range.start, cell.start_x)
            } else {
                (cell.range.end, cell.end_x)
            };
            return (index, self.affinity_at(index, edge));
        }
        let index = self.closest_index_for_x(x);
        (index, self.affinity_at(index, x))
    }

    /// Move to the adjacent physical grapheme edge.
    pub fn visual_caret(
        &self,
        index: usize,
        affinity: TextAffinity,
        right: bool,
    ) -> Option<(usize, TextAffinity)> {
        let x = self.caret_x(index, affinity);
        let cell = if right {
            self.cells
                .iter()
                .filter(|cell| cell.start_x.max(cell.end_x) > x + px(0.01))
                .min_by_key(|cell| cell.start_x.max(cell.end_x))
        } else {
            self.cells
                .iter()
                .filter(|cell| cell.start_x.min(cell.end_x) < x - px(0.01))
                .max_by_key(|cell| cell.start_x.min(cell.end_x))
        }?;
        let (next, edge) = if right == (cell.start_x < cell.end_x) {
            (cell.range.end, cell.end_x)
        } else {
            (cell.range.start, cell.start_x)
        };
        Some((next, self.affinity_at(next, edge)))
    }

    pub(crate) fn set_cells(&mut self, text: &str, mut cells: Vec<TextCell>) {
        let base = if self.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        self.bidi_levels = unicode_bidi::ParagraphBidiInfo::new(text, Some(base))
            .levels
            .iter()
            .map(|level| level.number())
            .collect();
        cells.sort_by_key(|cell| cell.range.start);
        self.carets = text
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .map(|index| {
                let after = cells.partition_point(|cell| cell.range.start <= index);
                let next = after
                    .checked_sub(1)
                    .and_then(|at| cells.get(at))
                    .filter(|cell| index < cell.range.end);
                let previous = cells
                    .get(cells.partition_point(|cell| cell.range.end < index))
                    .filter(|cell| cell.range.end == index);
                let (x, secondary_x) = match (previous, next) {
                    (_, Some(cell)) if cell.range.start < index => {
                        let x = cell.start_x.min(cell.end_x);
                        (x, x)
                    }
                    (Some(previous), Some(next)) => (
                        previous.end_x.min(next.start_x),
                        previous.end_x.max(next.start_x),
                    ),
                    (None, Some(next)) => (next.start_x, next.start_x),
                    (Some(previous), None) => (previous.end_x, previous.end_x),
                    _ => (Pixels::ZERO, Pixels::ZERO),
                };
                TextCaret {
                    index,
                    x,
                    secondary_x,
                }
            })
            .collect();
        self.cells = cells;
    }
    /// The index for the character at the given x coordinate
    pub fn index_for_x(&self, x: Pixels) -> Option<usize> {
        if !self.cells.is_empty() {
            return self
                .cells
                .iter()
                .find(|cell| cell.start_x.min(cell.end_x) <= x && x < cell.start_x.max(cell.end_x))
                .map(|cell| cell.range.start);
        }
        if x >= self.width {
            None
        } else {
            for run in self.runs.iter().rev() {
                for glyph in run.glyphs.iter().rev() {
                    if glyph.position.x <= x {
                        return Some(glyph.index);
                    }
                }
            }
            Some(0)
        }
    }

    /// closest_index_for_x returns the character boundary closest to the given x coordinate
    /// (e.g. to handle aligning up/down arrow keys)
    pub fn closest_index_for_x(&self, x: Pixels) -> usize {
        if !self.cells.is_empty() {
            if let Some(cell) = self
                .cells
                .iter()
                .find(|cell| cell.start_x.min(cell.end_x) <= x && x < cell.start_x.max(cell.end_x))
            {
                return if (cell.start_x - x).abs() < (cell.end_x - x).abs() {
                    cell.range.start
                } else {
                    cell.range.end
                };
            }
            return self
                .cells
                .iter()
                .flat_map(|cell| {
                    [
                        (cell.start_x, cell.range.start),
                        (cell.end_x, cell.range.end),
                    ]
                })
                .min_by(|a, b| (a.0 - x).abs().partial_cmp(&(b.0 - x).abs()).unwrap())
                .map_or(0, |(_, index)| index);
        }
        let mut prev_index = 0;
        let mut prev_x = px(0.);

        for run in self.runs.iter() {
            for glyph in run.glyphs.iter() {
                if glyph.position.x >= x {
                    if glyph.position.x - x < x - prev_x {
                        return glyph.index;
                    } else {
                        return prev_index;
                    }
                }
                prev_index = glyph.index;
                prev_x = glyph.position.x;
            }
        }

        if self.width - x < x - prev_x {
            self.len
        } else {
            prev_index
        }
    }

    /// The x position of the character at the given index
    pub fn x_for_index(&self, index: usize) -> Pixels {
        if let Ok(index) = self
            .carets
            .binary_search_by_key(&index, |caret| caret.index)
        {
            return self.carets[index].x;
        }
        for run in &self.runs {
            for glyph in &run.glyphs {
                if glyph.index >= index {
                    return glyph.position.x;
                }
            }
        }
        self.width
    }

    /// The corresponding Font at the given index
    pub fn font_id_for_index(&self, index: usize) -> Option<FontId> {
        self.runs
            .iter()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(move |glyph| (glyph.index, run.font_id))
            })
            .filter(|(start, _)| *start <= index)
            .max_by_key(|(start, _)| *start)
            .map(|(_, font)| font)
    }

    /// The disjoint physical intervals occupied by a logical selection.
    pub fn selection_ranges(&self, range: Range<usize>) -> Vec<Range<Pixels>> {
        let mut spans = self
            .cells
            .iter()
            .filter(|cell| cell.range.start < range.end && range.start < cell.range.end)
            .map(|cell| cell.start_x.min(cell.end_x)..cell.start_x.max(cell.end_x))
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| span.start);
        let mut merged: Vec<Range<Pixels>> = Vec::new();
        for span in spans {
            if let Some(last) = merged.last_mut()
                && span.start <= last.end + px(0.01)
            {
                last.end = last.end.max(span.end);
            } else {
                merged.push(span);
            }
        }
        merged
    }

    /// Independently positioned rows, including layouts that wrap without reshaping.
    pub fn fragments(&self, boundaries: &[WrapBoundary]) -> Vec<TextFragment> {
        if !self.visual_lines.is_empty() {
            return self.visual_lines.clone();
        }
        let starts = std::iter::once((0, px(0.)))
            .chain(boundaries.iter().map(|boundary| {
                let glyph = &self.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
                (glyph.index, glyph.position.x)
            }))
            .chain(std::iter::once((self.len, self.width)))
            .collect::<Vec<_>>();
        starts
            .windows(2)
            .map(|row| TextFragment {
                range: row[0].0..row[1].0,
                x: row[0].1,
                width: row[1].1 - row[0].1,
            })
            .collect()
    }

    fn compute_wrap_boundaries(
        &self,
        text: &str,
        wrap_width: Pixels,
        max_lines: Option<usize>,
    ) -> SmallVec<[WrapBoundary; 1]> {
        let mut boundaries = SmallVec::new();
        let mut first_non_whitespace_ix = None;
        let mut last_candidate_ix = None;
        let mut last_candidate_x = px(0.);
        let mut last_boundary = WrapBoundary {
            run_ix: 0,
            glyph_ix: 0,
        };
        let mut last_boundary_x = px(0.);
        let mut prev_ch = '\0';
        let mut glyphs = self
            .runs
            .iter()
            .enumerate()
            .flat_map(move |(run_ix, run)| {
                run.glyphs.iter().enumerate().map(move |(glyph_ix, glyph)| {
                    let character = text[glyph.index..].chars().next().unwrap();
                    (
                        WrapBoundary { run_ix, glyph_ix },
                        character,
                        glyph.position.x,
                    )
                })
            })
            .peekable();

        while let Some((boundary, ch, x)) = glyphs.next() {
            if ch == '\n' {
                continue;
            }

            // Here is very similar to `LineWrapper::wrap_line` to determine text wrapping,
            // but there are some differences, so we have to duplicate the code here.
            if LineWrapper::is_word_char(ch) {
                if prev_ch == ' ' && ch != ' ' && first_non_whitespace_ix.is_some() {
                    last_candidate_ix = Some(boundary);
                    last_candidate_x = x;
                }
            } else {
                if ch != ' ' && first_non_whitespace_ix.is_some() {
                    last_candidate_ix = Some(boundary);
                    last_candidate_x = x;
                }
            }

            if ch != ' ' && first_non_whitespace_ix.is_none() {
                first_non_whitespace_ix = Some(boundary);
            }

            let next_x = glyphs.peek().map_or(self.width, |(_, _, x)| *x);
            let width = next_x - last_boundary_x;

            if width > wrap_width && boundary > last_boundary {
                // When used line_clamp, we should limit the number of lines.
                if let Some(max_lines) = max_lines
                    && boundaries.len() >= max_lines - 1
                {
                    break;
                }

                if let Some(last_candidate_ix) = last_candidate_ix.take() {
                    last_boundary = last_candidate_ix;
                    last_boundary_x = last_candidate_x;
                } else {
                    last_boundary = boundary;
                    last_boundary_x = x;
                }
                boundaries.push(last_boundary);
            }
            prev_ch = ch;
        }

        boundaries
    }
}

/// A line of text that has been wrapped to fit a given width
#[derive(Default, Debug)]
pub struct WrappedLineLayout {
    /// Optional per-row metrics for inline spans with different font or line sizes.
    pub row_metrics: Vec<TextRowMetrics>,
    /// The line layout, pre-wrapping.
    pub unwrapped_layout: Arc<LineLayout>,

    /// The boundaries at which the line was wrapped
    pub wrap_boundaries: SmallVec<[WrapBoundary; 1]>,

    /// The width of the line, if it was wrapped
    pub wrap_width: Option<Pixels>,
}

/// A boundary at which a line was wrapped
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WrapBoundary {
    /// The index in the run just before the line was wrapped
    pub run_ix: usize,
    /// The index of the glyph just before the line was wrapped
    pub glyph_ix: usize,
}

/// Baseline and vertical extent of an inline row.
#[derive(Copy, Clone, Debug)]
pub struct TextRowMetrics {
    /// Distance from the paragraph's top to the row's top.
    pub offset: Pixels,
    /// Full row height, including inline leading.
    pub height: Pixels,
    /// Baseline position measured from the row's top.
    pub baseline: Pixels,
}

impl WrappedLineLayout {
    /// Resolve uniform or inline metrics for a physical row.
    pub fn row_metrics(&self, row: usize, line_height: Pixels) -> TextRowMetrics {
        self.row_metrics
            .get(row)
            .copied()
            .unwrap_or(TextRowMetrics {
                offset: line_height * row,
                height: line_height,
                baseline: (line_height + self.ascent() - self.descent()) / 2.,
            })
    }

    /// Locate a physical row by its vertical coordinate.
    pub fn row_for_y(&self, y: Pixels, line_height: Pixels) -> usize {
        if self.row_metrics.is_empty() {
            (y / line_height) as usize
        } else {
            self.row_metrics
                .partition_point(|row| row.offset + row.height <= y)
        }
    }

    /// The length of the underlying text, in utf8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.unwrapped_layout.len
    }

    /// The width of this line, in pixels, whether or not it was wrapped.
    pub fn width(&self) -> Pixels {
        self.wrap_width
            .unwrap_or(Pixels::MAX)
            .min(self.unwrapped_layout.width)
    }

    /// The size of the whole wrapped text, for the given line_height.
    /// can span multiple lines if there are multiple wrap boundaries.
    pub fn size(&self, line_height: Pixels) -> Size<Pixels> {
        Size {
            width: self.width(),
            height: self
                .row_metrics
                .last()
                .map_or(line_height * (self.wrap_boundaries.len() + 1), |row| {
                    row.offset + row.height
                }),
        }
    }

    /// The ascent of a line in this layout
    pub fn ascent(&self) -> Pixels {
        self.unwrapped_layout.ascent
    }

    /// The descent of a line in this layout
    pub fn descent(&self) -> Pixels {
        self.unwrapped_layout.descent
    }

    /// The wrap boundaries in this layout
    pub fn wrap_boundaries(&self) -> &[WrapBoundary] {
        &self.wrap_boundaries
    }

    /// The font size of this layout
    pub fn font_size(&self) -> Pixels {
        self.unwrapped_layout.font_size
    }

    /// The runs in this layout, sans wrapping
    pub fn runs(&self) -> &[ShapedRun] {
        &self.unwrapped_layout.runs
    }

    /// The index corresponding to a given position in this layout for the given line height.
    ///
    /// See also [`Self::closest_index_for_position`].
    pub fn index_for_position(
        &self,
        position: Point<Pixels>,
        line_height: Pixels,
    ) -> Result<usize, usize> {
        self._index_for_position(position, line_height, false)
    }

    /// The closest index to a given position in this layout for the given line height.
    ///
    /// Closest means the character boundary closest to the given position.
    ///
    /// See also [`LineLayout::closest_index_for_x`].
    pub fn closest_index_for_position(
        &self,
        position: Point<Pixels>,
        line_height: Pixels,
    ) -> Result<usize, usize> {
        self._index_for_position(position, line_height, true)
    }

    fn _index_for_position(
        &self,
        mut position: Point<Pixels>,
        line_height: Pixels,
        closest: bool,
    ) -> Result<usize, usize> {
        if !self.unwrapped_layout.visual_lines.is_empty() {
            let row = self.row_for_y(position.y, line_height);
            let Some(fragment) = self.unwrapped_layout.visual_lines.get(row) else {
                return Err(self.len());
            };
            let x = fragment.x + position.x;
            let cells = self.unwrapped_layout.cells.iter().filter(|cell| {
                cell.range.start >= fragment.range.start && cell.range.end <= fragment.range.end
            });
            let nearest = cells
                .clone()
                .flat_map(|cell| {
                    [
                        (cell.start_x, cell.range.start),
                        (cell.end_x, cell.range.end),
                    ]
                })
                .min_by(|a, b| (a.0 - x).abs().partial_cmp(&(b.0 - x).abs()).unwrap())
                .map_or(fragment.range.start, |(_, index)| index);
            if closest {
                return Ok(cells
                    .clone()
                    .find(|cell| {
                        cell.start_x.min(cell.end_x) <= x && x < cell.start_x.max(cell.end_x)
                    })
                    .map_or(nearest, |cell| {
                        if (cell.start_x - x).abs() < (cell.end_x - x).abs() {
                            cell.range.start
                        } else {
                            cell.range.end
                        }
                    }));
            }
            return cells
                .into_iter()
                .find(|cell| cell.start_x.min(cell.end_x) <= x && x < cell.start_x.max(cell.end_x))
                .map_or(Err(nearest), |cell| Ok(cell.range.start));
        }
        let wrapped_line_ix = self.row_for_y(position.y, line_height);

        let wrapped_line_start_index;
        let wrapped_line_start_x;
        if wrapped_line_ix > 0 {
            let Some(line_start_boundary) = self.wrap_boundaries.get(wrapped_line_ix - 1) else {
                return Err(0);
            };
            let run = &self.unwrapped_layout.runs[line_start_boundary.run_ix];
            let glyph = &run.glyphs[line_start_boundary.glyph_ix];
            wrapped_line_start_index = glyph.index;
            wrapped_line_start_x = glyph.position.x;
        } else {
            wrapped_line_start_index = 0;
            wrapped_line_start_x = Pixels::ZERO;
        };

        let wrapped_line_end_index;
        let wrapped_line_end_x;
        if wrapped_line_ix < self.wrap_boundaries.len() {
            let next_wrap_boundary_ix = wrapped_line_ix;
            let next_wrap_boundary = self.wrap_boundaries[next_wrap_boundary_ix];
            let run = &self.unwrapped_layout.runs[next_wrap_boundary.run_ix];
            let glyph = &run.glyphs[next_wrap_boundary.glyph_ix];
            wrapped_line_end_index = glyph.index;
            wrapped_line_end_x = glyph.position.x;
        } else {
            wrapped_line_end_index = self.unwrapped_layout.len;
            wrapped_line_end_x = self.unwrapped_layout.width;
        };

        let mut position_in_unwrapped_line = position;
        position_in_unwrapped_line.x += wrapped_line_start_x;
        if position_in_unwrapped_line.x < wrapped_line_start_x {
            Err(wrapped_line_start_index)
        } else if position_in_unwrapped_line.x >= wrapped_line_end_x {
            Err(wrapped_line_end_index)
        } else {
            if closest {
                Ok(self
                    .unwrapped_layout
                    .closest_index_for_x(position_in_unwrapped_line.x))
            } else {
                Ok(self
                    .unwrapped_layout
                    .index_for_x(position_in_unwrapped_line.x)
                    .unwrap())
            }
        }
    }

    /// Returns the pixel position for the given byte index.
    pub fn position_for_index(&self, index: usize, line_height: Pixels) -> Option<Point<Pixels>> {
        if !self.unwrapped_layout.visual_lines.is_empty() {
            let (row, fragment) =
                self.unwrapped_layout
                    .visual_lines
                    .iter()
                    .enumerate()
                    .find(|(_, fragment)| {
                        fragment.range.start <= index
                            && (index < fragment.range.end
                                || index == self.len() && fragment.range.end == self.len())
                    })?;
            return Some(point(
                self.unwrapped_layout.x_for_index(index) - fragment.x,
                self.row_metrics(row, line_height).offset,
            ));
        }
        if self.wrap_boundaries.is_empty() {
            return (index <= self.len())
                .then(|| point(self.unwrapped_layout.x_for_index(index), px(0.)));
        }
        let mut line_start_ix = 0;
        let mut line_end_indices = self
            .wrap_boundaries
            .iter()
            .map(|wrap_boundary| {
                let run = &self.unwrapped_layout.runs[wrap_boundary.run_ix];
                let glyph = &run.glyphs[wrap_boundary.glyph_ix];
                glyph.index
            })
            .chain([self.len()])
            .enumerate();
        for (ix, line_end_ix) in line_end_indices {
            let line_y = self.row_metrics(ix, line_height).offset;
            if index < line_start_ix {
                break;
            } else if index > line_end_ix || (index == line_end_ix && line_end_ix < self.len()) {
                line_start_ix = line_end_ix;
                continue;
            } else {
                let line_start_x = if ix == 0 {
                    px(0.)
                } else {
                    let boundary = self.wrap_boundaries[ix - 1];
                    self.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix]
                        .position
                        .x
                };
                let x = self.unwrapped_layout.x_for_index(index) - line_start_x;
                return Some(point(x, line_y));
            }
        }

        None
    }

    /// Physical rectangles for selecting a logical range, including mixed direction rows.
    pub fn selection_rects(
        &self,
        range: Range<usize>,
        line_height: Pixels,
    ) -> Vec<crate::Bounds<Pixels>> {
        let fragments = self.unwrapped_layout.fragments(&self.wrap_boundaries);
        fragments
            .iter()
            .enumerate()
            .flat_map(|(row, fragment)| {
                let selected =
                    range.start.max(fragment.range.start)..range.end.min(fragment.range.end);
                self.unwrapped_layout
                    .selection_ranges(selected)
                    .into_iter()
                    .map(move |span| {
                        crate::Bounds::new(
                            point(
                                span.start - fragment.x,
                                self.row_metrics(row, line_height).offset,
                            ),
                            crate::size(
                                span.end - span.start,
                                self.row_metrics(row, line_height).height,
                            ),
                        )
                    })
            })
            .collect()
    }
}

pub(crate) struct LineLayoutCache {
    previous_frame: Mutex<FrameCache>,
    current_frame: RwLock<FrameCache>,
    platform_text_system: Arc<dyn PlatformTextSystem>,
}

#[derive(Default)]
struct FrameCache {
    lines: FxHashMap<Arc<CacheKey>, Arc<LineLayout>>,
    wrapped_lines: FxHashMap<Arc<CacheKey>, Arc<WrappedLineLayout>>,
    used_lines: Vec<Arc<CacheKey>>,
    used_wrapped_lines: Vec<Arc<CacheKey>>,
}

#[derive(Clone, Default)]
pub(crate) struct LineLayoutIndex {
    lines_index: usize,
    wrapped_lines_index: usize,
}

impl LineLayoutCache {
    pub fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        Self {
            previous_frame: Mutex::default(),
            current_frame: RwLock::default(),
            platform_text_system,
        }
    }

    pub fn layout_index(&self) -> LineLayoutIndex {
        let frame = self.current_frame.read();
        LineLayoutIndex {
            lines_index: frame.used_lines.len(),
            wrapped_lines_index: frame.used_wrapped_lines.len(),
        }
    }

    pub fn reuse_layouts(&self, range: Range<LineLayoutIndex>) {
        let mut previous_frame = &mut *self.previous_frame.lock();
        let mut current_frame = &mut *self.current_frame.write();

        for key in &previous_frame.used_lines[range.start.lines_index..range.end.lines_index] {
            if let Some((key, line)) = previous_frame.lines.remove_entry(key) {
                current_frame.lines.insert(key, line);
            }
            current_frame.used_lines.push(key.clone());
        }

        for key in &previous_frame.used_wrapped_lines
            [range.start.wrapped_lines_index..range.end.wrapped_lines_index]
        {
            if let Some((key, line)) = previous_frame.wrapped_lines.remove_entry(key) {
                current_frame.wrapped_lines.insert(key, line);
            }
            current_frame.used_wrapped_lines.push(key.clone());
        }
    }

    pub fn truncate_layouts(&self, index: LineLayoutIndex) {
        let mut current_frame = &mut *self.current_frame.write();
        current_frame.used_lines.truncate(index.lines_index);
        current_frame
            .used_wrapped_lines
            .truncate(index.wrapped_lines_index);
    }

    pub fn finish_frame(&self) {
        let mut prev_frame = self.previous_frame.lock();
        let mut curr_frame = self.current_frame.write();
        std::mem::swap(&mut *prev_frame, &mut *curr_frame);
        curr_frame.lines.clear();
        curr_frame.wrapped_lines.clear();
        curr_frame.used_lines.clear();
        curr_frame.used_wrapped_lines.clear();
    }

    pub fn layout_wrapped_line<Text>(
        &self,
        text: Text,
        font_size: Pixels,
        runs: &[FontRun],
        wrap_width: Option<Pixels>,
        max_lines: Option<usize>,
    ) -> Arc<WrappedLineLayout>
    where
        Text: AsRef<str>,
        SharedString: From<Text>,
    {
        let key = &CacheKeyRef {
            text: text.as_ref(),
            font_size,
            runs,
            wrap_width,
            max_lines,
            force_width: None,
        } as &dyn AsCacheKeyRef;

        let current_frame = self.current_frame.upgradable_read();
        if let Some(layout) = current_frame.wrapped_lines.get(key) {
            return layout.clone();
        }

        let previous_frame_entry = self.previous_frame.lock().wrapped_lines.remove_entry(key);
        if let Some((key, layout)) = previous_frame_entry {
            let mut current_frame = RwLockUpgradableReadGuard::upgrade(current_frame);
            current_frame
                .wrapped_lines
                .insert(key.clone(), layout.clone());
            current_frame.used_wrapped_lines.push(key);
            layout
        } else {
            drop(current_frame);
            let text = SharedString::from(text);
            let unwrapped_layout = self.layout_line::<&SharedString>(&text, font_size, runs, None);
            let wrap_boundaries = if let Some(wrap_width) = wrap_width {
                unwrapped_layout.compute_wrap_boundaries(text.as_ref(), wrap_width, max_lines)
            } else {
                SmallVec::new()
            };
            let layout = Arc::new(WrappedLineLayout {
                row_metrics: Vec::new(),
                unwrapped_layout,
                wrap_boundaries,
                wrap_width,
            });
            let key = Arc::new(CacheKey {
                text,
                font_size,
                runs: SmallVec::from(runs),
                wrap_width,
                max_lines,
                force_width: None,
            });

            let mut current_frame = self.current_frame.write();
            current_frame
                .wrapped_lines
                .insert(key.clone(), layout.clone());
            current_frame.used_wrapped_lines.push(key);

            layout
        }
    }

    pub fn layout_line<Text>(
        &self,
        text: Text,
        font_size: Pixels,
        runs: &[FontRun],
        force_width: Option<Pixels>,
    ) -> Arc<LineLayout>
    where
        Text: AsRef<str>,
        SharedString: From<Text>,
    {
        let key = &CacheKeyRef {
            text: text.as_ref(),
            font_size,
            runs,
            wrap_width: None,
            max_lines: None,
            force_width,
        } as &dyn AsCacheKeyRef;

        let current_frame = self.current_frame.upgradable_read();
        if let Some(layout) = current_frame.lines.get(key) {
            return layout.clone();
        }

        let mut current_frame = RwLockUpgradableReadGuard::upgrade(current_frame);
        if let Some((key, layout)) = self.previous_frame.lock().lines.remove_entry(key) {
            current_frame.lines.insert(key.clone(), layout.clone());
            current_frame.used_lines.push(key);
            layout
        } else {
            let text = SharedString::from(text);
            let mut layout = self
                .platform_text_system
                .layout_line(&text, font_size, runs);
            if runs.iter().any(|run| run.trim_punctuation) {
                let mut end = 0;
                let ends = runs
                    .iter()
                    .map(|run| {
                        end += run.len;
                        end
                    })
                    .collect::<Vec<_>>();
                let mut offsets = FxHashMap::default();
                for run in &mut layout.runs {
                    for glyph in &mut run.glyphs {
                        if glyph.position_is_advance {
                            continue;
                        }
                        let source = ends.partition_point(|end| *end <= glyph.index);
                        if !runs[source].trim_punctuation {
                            continue;
                        }
                        let character = text[glyph.index..].chars().next().unwrap();
                        let offset = *offsets
                            .entry((run.font_id, run.font_size, character))
                            .or_insert_with(|| {
                                let mut bytes = [0; 4];
                                let character = character.encode_utf8(&mut bytes);
                                let isolated = self.platform_text_system.layout_line(
                                    character,
                                    font_size,
                                    &[FontRun {
                                        font_size: run.font_size,
                                        len: character.len(),
                                        font_id: run.font_id,
                                        letter_spacing: crate::px(0.),
                                        trim_punctuation: false,
                                        text_direction: runs[source].text_direction,
                                    }],
                                );
                                isolated
                                    .runs
                                    .first()
                                    .and_then(|run| run.glyphs.first())
                                    .map_or(px(0.), |glyph| glyph.position.x)
                            });
                        // OpenType spacing features move glyph ink within its cell.
                        // Preserve that displacement while keeping logical caret positions.
                        glyph.position.x -= offset;
                        glyph.paint_offset.x += offset;
                        glyph.position_is_advance = true;
                    }
                }
            }

            if let Some(force_width) = force_width {
                let mut glyph_pos = 0;
                for run in layout.runs.iter_mut() {
                    for glyph in run.glyphs.iter_mut() {
                        if (glyph.position.x - glyph_pos * force_width).abs() > px(1.) {
                            glyph.position.x = glyph_pos * force_width;
                        }
                        glyph_pos += 1;
                    }
                }
            }

            if layout.cells.is_empty() || force_width.is_some() {
                let cells = text
                    .grapheme_indices(true)
                    .map(|(start, grapheme)| {
                        let end = start + grapheme.len();
                        TextCell {
                            range: start..end,
                            start_x: layout.x_for_index(start),
                            end_x: layout.x_for_index(end),
                        }
                    })
                    .collect();
                layout.set_cells(&text, cells);
            }

            if force_width.is_none() {
                layout.apply_letter_spacing(&text, runs);
            }

            let key = Arc::new(CacheKey {
                text,
                font_size,
                runs: SmallVec::from(runs),
                wrap_width: None,
                max_lines: None,
                force_width,
            });
            let layout = Arc::new(layout);
            current_frame.lines.insert(key.clone(), layout.clone());
            current_frame.used_lines.push(key);
            layout
        }
    }
}

/// A run of text with a single font.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FontRun {
    pub(crate) font_size: Pixels,
    pub(crate) len: usize,
    pub(crate) font_id: FontId,
    pub(crate) trim_punctuation: bool,
    pub(crate) letter_spacing: Pixels,
    pub(crate) text_direction: super::TextDirection,
}

trait AsCacheKeyRef {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_>;
}

#[derive(Clone, Debug, Eq)]
struct CacheKey {
    text: SharedString,
    font_size: Pixels,
    runs: SmallVec<[FontRun; 1]>,
    wrap_width: Option<Pixels>,
    max_lines: Option<usize>,
    force_width: Option<Pixels>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct CacheKeyRef<'a> {
    text: &'a str,
    font_size: Pixels,
    runs: &'a [FontRun],
    wrap_width: Option<Pixels>,
    max_lines: Option<usize>,
    force_width: Option<Pixels>,
}

impl PartialEq for dyn AsCacheKeyRef + '_ {
    fn eq(&self, other: &dyn AsCacheKeyRef) -> bool {
        self.as_cache_key_ref() == other.as_cache_key_ref()
    }
}

impl Eq for dyn AsCacheKeyRef + '_ {}

impl Hash for dyn AsCacheKeyRef + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_cache_key_ref().hash(state)
    }
}

impl AsCacheKeyRef for CacheKey {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_> {
        CacheKeyRef {
            text: &self.text,
            font_size: self.font_size,
            runs: self.runs.as_slice(),
            wrap_width: self.wrap_width,
            max_lines: self.max_lines,
            force_width: self.force_width,
        }
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_cache_key_ref().eq(&other.as_cache_key_ref())
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_cache_key_ref().hash(state);
    }
}

impl<'a> Borrow<dyn AsCacheKeyRef + 'a> for Arc<CacheKey> {
    fn borrow(&self) -> &(dyn AsCacheKeyRef + 'a) {
        self.as_ref() as &dyn AsCacheKeyRef
    }
}

impl AsCacheKeyRef for CacheKeyRef<'_> {
    fn as_cache_key_ref(&self) -> CacheKeyRef<'_> {
        *self
    }
}
