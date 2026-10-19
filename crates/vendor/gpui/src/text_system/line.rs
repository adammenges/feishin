use crate::{
    App, Bounds, Hsla, LineLayout, Pixels, Point, Result, SharedString, StrikethroughStyle,
    TextAlign, UnderlineStyle, Window, WrapBoundary, WrappedLineLayout, black, fill, point, px,
    size,
};
use derive_more::{Deref, DerefMut};
use smallvec::SmallVec;
use std::sync::Arc;

/// Set the text decoration for a run of text.
#[derive(Debug, Clone)]
pub struct DecorationRun {
    /// The length of the run in utf-8 bytes.
    pub len: u32,

    /// The color for this run
    pub color: Hsla,

    /// The background color for this run
    pub background_color: Option<Hsla>,

    /// The underline style for this run
    pub underline: Option<UnderlineStyle>,

    /// The overline style of the text.
    pub overline: Option<UnderlineStyle>,

    /// The strikethrough style for this run
    pub strikethrough: Option<StrikethroughStyle>,
}

impl DecorationRun {
    pub(crate) fn set_paint_color(&mut self, color: Hsla) {
        self.color = color;
        for decoration in [&mut self.underline, &mut self.overline]
            .into_iter()
            .flatten()
        {
            decoration.color = Some(color);
        }
        if let Some(decoration) = &mut self.strikethrough {
            decoration.color = Some(color);
        }
    }
}

/// A line of text that has been shaped and decorated.
#[derive(Clone, Default, Debug, Deref, DerefMut)]
pub struct ShapedLine {
    #[deref]
    #[deref_mut]
    pub(crate) layout: Arc<LineLayout>,
    /// The text that was shaped for this line.
    pub text: SharedString,
    pub(crate) decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl ShapedLine {
    /// The length of the line in utf-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len
    }

    /// Override the len, useful if you're rendering text a
    /// as text b (e.g. rendering invisibles).
    pub fn with_len(mut self, len: usize) -> Self {
        let layout = self.layout.as_ref();
        self.layout = Arc::new(LineLayout {
            font_size: layout.font_size,
            width: layout.width,
            ascent: layout.ascent,
            descent: layout.descent,
            runs: layout.runs.clone(),
            carets: layout.carets.clone(),
            cells: layout.cells.clone(),
            bidi_levels: layout.bidi_levels.clone(),
            visual_lines: layout.visual_lines.clone(),
            rtl: layout.rtl,
            len,
        });
        self
    }

    /// Paint the line of text to the window.
    pub fn paint(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        paint_line(
            origin,
            &self.layout,
            line_height,
            TextAlign::default(),
            None,
            &self.decoration_runs,
            &[],
            &[],
            window,
            cx,
        )?;

        Ok(())
    }

    /// Paint the background of the line to the window.
    pub fn paint_background(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        paint_line_background(
            origin,
            &self.layout,
            line_height,
            TextAlign::default(),
            None,
            &self.decoration_runs,
            &[],
            &[],
            window,
            cx,
        )?;

        Ok(())
    }
}

/// A line of text that has been shaped, decorated, and wrapped by the text layout system.
#[derive(Clone, Default, Debug, Deref, DerefMut)]
pub struct WrappedLine {
    #[deref]
    #[deref_mut]
    pub(crate) layout: Arc<WrappedLineLayout>,
    /// The text that was shaped for this line.
    pub text: SharedString,
    pub(crate) decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl WrappedLine {
    /// The length of the underlying, unwrapped layout, in utf-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len()
    }

    /// Extract physical rows without reshaping them outside their paragraph context.
    pub fn shaped_rows(&self) -> SmallVec<[ShapedLine; 1]> {
        let source = &self.unwrapped_layout;
        self.unwrapped_layout
            .fragments(&self.wrap_boundaries)
            .into_iter()
            .map(|row| {
                let text: SharedString = self.text[row.range.clone()].to_owned().into();
                let mut layout = LineLayout {
                    font_size: source.font_size,
                    width: row.width,
                    ascent: source.ascent,
                    descent: source.descent,
                    rtl: source.rtl,
                    len: row.range.len(),
                    ..Default::default()
                };
                for source in &source.runs {
                    let mut run = source.clone();
                    run.glyphs.retain(|glyph| row.range.contains(&glyph.index));
                    if run.glyphs.is_empty() {
                        continue;
                    }
                    for glyph in &mut run.glyphs {
                        glyph.index -= row.range.start;
                        glyph.position.x -= row.x;
                    }
                    layout.runs.push(run);
                }
                layout.set_cells(
                    &text,
                    source
                        .cells
                        .iter()
                        .filter(|cell| row.range.contains(&cell.range.start))
                        .cloned()
                        .map(|mut cell| {
                            cell.range.start -= row.range.start;
                            cell.range.end -= row.range.start;
                            cell.start_x -= row.x;
                            cell.end_x -= row.x;
                            cell
                        })
                        .collect(),
                );
                if let Some(levels) = source.bidi_levels.get(row.range.clone()) {
                    layout.bidi_levels = levels.to_vec();
                }
                let mut decoration_runs = SmallVec::new();
                for (range, source) in decoration_ranges(&self.decoration_runs) {
                    let overlap = range.start.max(row.range.start)..range.end.min(row.range.end);
                    if !overlap.is_empty() {
                        let mut run = source.clone();
                        run.len = overlap.len() as u32;
                        decoration_runs.push(run);
                    }
                }
                ShapedLine {
                    layout: Arc::new(layout),
                    text,
                    decoration_runs,
                }
            })
            .collect()
    }

    /// Paint this line of text to the window.
    pub fn paint(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        align: TextAlign,
        bounds: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        let align_width = match bounds {
            Some(bounds) => Some(bounds.size.width),
            None => self.layout.wrap_width,
        };

        paint_line(
            origin,
            &self.layout.unwrapped_layout,
            line_height,
            align,
            align_width,
            &self.decoration_runs,
            &self.wrap_boundaries,
            &self.layout.row_metrics,
            window,
            cx,
        )?;

        Ok(())
    }

    /// Paint the background of line of text to the window.
    pub fn paint_background(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        align: TextAlign,
        bounds: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        let align_width = match bounds {
            Some(bounds) => Some(bounds.size.width),
            None => self.layout.wrap_width,
        };

        paint_line_background(
            origin,
            &self.layout.unwrapped_layout,
            line_height,
            align,
            align_width,
            &self.decoration_runs,
            &self.wrap_boundaries,
            &self.layout.row_metrics,
            window,
            cx,
        )?;

        Ok(())
    }
}

fn aligned_row_origin(
    origin: Point<Pixels>,
    width: Pixels,
    align: TextAlign,
    row: &crate::TextFragment,
    rtl: bool,
) -> Pixels {
    origin.x + row.alignment_offset(align, width, rtl)
}

fn decoration_ranges(runs: &[DecorationRun]) -> Vec<(std::ops::Range<usize>, &DecorationRun)> {
    let mut offset = 0;
    runs.iter()
        .map(|run| {
            let range = offset..offset + run.len as usize;
            offset = range.end;
            (range, run)
        })
        .collect()
}

fn paint_line(
    origin: Point<Pixels>,
    layout: &LineLayout,
    line_height: Pixels,
    align: TextAlign,
    align_width: Option<Pixels>,
    decoration_runs: &[DecorationRun],
    wrap_boundaries: &[WrapBoundary],
    row_metrics: &[crate::TextRowMetrics],
    window: &mut Window,
    cx: &mut App,
) -> Result<()> {
    let mut result = Ok(());
    window.with_text_shadows(cx, |offset, color, window, cx| {
        let mut mask;
        let runs = if let Some(color) = color {
            mask = decoration_runs.to_vec();
            for run in &mut mask {
                run.set_paint_color(color);
            }
            mask.as_slice()
        } else {
            decoration_runs
        };
        let painted = paint_line_without_shadows(
            origin + offset,
            layout,
            line_height,
            align,
            align_width,
            runs,
            wrap_boundaries,
            row_metrics,
            window,
            cx,
        );
        if result.is_ok() {
            result = painted;
        }
    });
    result
}

fn paint_line_without_shadows(
    origin: Point<Pixels>,
    layout: &LineLayout,
    line_height: Pixels,
    align: TextAlign,
    align_width: Option<Pixels>,
    decoration_runs: &[DecorationRun],
    wrap_boundaries: &[WrapBoundary],
    row_metrics: &[crate::TextRowMetrics],
    window: &mut Window,
    cx: &mut App,
) -> Result<()> {
    let rows = layout.fragments(wrap_boundaries);
    let decorations = decoration_ranges(decoration_runs);
    let width = align_width.unwrap_or(layout.width);
    let height = row_metrics
        .last()
        .map_or(line_height * rows.len(), |row| row.offset + row.height);
    let bounds = Bounds::new(origin, size(layout.width, height));
    window.paint_layer(bounds, |window| {
        let text_system = cx.text_system().clone();
        let css_thickness = window.text_style().css_decoration_thickness;
        let primary_font = layout.runs.first().map(|run| run.font_id);
        let font_ascent = primary_font.map_or(layout.ascent, |font| {
            text_system.ascent(font, layout.font_size)
        });
        let thickness = |native: Pixels| {
            if native <= px(0.) {
                return native;
            }
            match css_thickness {
                Some(crate::CssDecorationThickness::Auto) => (layout.font_size / 10.).max(px(1.)),
                Some(crate::CssDecorationThickness::FromFont) => {
                    primary_font.map_or(px(1.), |font| {
                        text_system
                            .underline_thickness(font, layout.font_size)
                            .max(px(1.))
                    })
                }
                Some(crate::CssDecorationThickness::Length(value)) => value.max(px(1.)),
                None => native,
            }
        };
        for (row_index, row) in rows.iter().enumerate() {
            let metrics = row_metrics
                .get(row_index)
                .copied()
                .unwrap_or(crate::TextRowMetrics {
                    offset: line_height * row_index,
                    height: line_height,
                    baseline: (line_height + layout.ascent - layout.descent) / 2.,
                });
            let baseline = metrics.baseline;
            let padding_top = baseline - layout.ascent;
            let decoration_top = baseline - font_ascent;
            let row_origin = point(
                aligned_row_origin(origin, width, align, row, layout.rtl),
                origin.y + metrics.offset,
            );
            for (range, style) in &decorations {
                let selected = range.start.max(row.range.start)..range.end.min(row.range.end);
                for span in layout.selection_ranges(selected) {
                    let x = row_origin.x + span.start - row.x;
                    let length = span.end - span.start;
                    if let Some(underline) = &style.underline {
                        let mut underline = underline.clone();
                        underline.color = Some(underline.color.unwrap_or(style.color));
                        underline.thickness = thickness(underline.thickness);
                        let y = if css_thickness.is_some() {
                            decoration_top
                                + (font_ascent + (underline.thickness / 2.).ceil().max(px(1.)))
                                    .floor()
                        } else {
                            baseline + layout.descent * 0.618
                        };
                        window.paint_underline(point(x, row_origin.y + y), length, &underline);
                    }
                    if let Some(overline) = &style.overline {
                        let mut overline = overline.clone();
                        overline.color = Some(overline.color.unwrap_or(style.color));
                        overline.thickness = thickness(overline.thickness);
                        let y = if css_thickness.is_some() {
                            decoration_top - overline.thickness.floor()
                        } else {
                            padding_top
                        };
                        window.paint_underline(point(x, row_origin.y + y), length, &overline);
                    }
                }
            }
            for run in &layout.runs {
                let max_glyph_size = text_system.bounding_box(run.font_id, run.font_size).size;
                for glyph in &run.glyphs {
                    if !row.range.contains(&glyph.index) {
                        continue;
                    }
                    let color = decorations
                        .get(decorations.partition_point(|(range, _)| range.end <= glyph.index))
                        .map_or(black(), |(_, style)| style.color);
                    let glyph_origin = row_origin
                        + point(glyph.position.x - row.x, glyph.position.y)
                        + glyph.paint_offset;
                    if !Bounds::new(glyph_origin, max_glyph_size)
                        .intersects(&window.content_mask().bounds)
                    {
                        continue;
                    }
                    let glyph_origin = glyph_origin + point(px(0.), baseline);
                    if glyph.is_emoji {
                        window.with_element_opacity(Some(color.a), |window| {
                            window.paint_emoji(glyph_origin, run.font_id, glyph.id, run.font_size)
                        })?;
                    } else {
                        window.paint_glyph(
                            glyph_origin,
                            run.font_id,
                            glyph.id,
                            run.font_size,
                            color,
                        )?;
                    }
                }
            }
            // CSS line-through paints above glyphs; underlines and overlines paint below.
            for (range, style) in &decorations {
                if let Some(strikethrough) = &style.strikethrough {
                    let selected = range.start.max(row.range.start)..range.end.min(row.range.end);
                    let mut strikethrough = *strikethrough;
                    strikethrough.color = Some(strikethrough.color.unwrap_or(style.color));
                    strikethrough.thickness = thickness(strikethrough.thickness);
                    let y = if css_thickness.is_some() {
                        decoration_top + font_ascent * (2. / 3.) - strikethrough.thickness / 2.
                    } else {
                        (layout.ascent * 0.5 + baseline) * 0.5
                    };
                    for span in layout.selection_ranges(selected) {
                        window.paint_strikethrough(
                            point(row_origin.x + span.start - row.x, row_origin.y + y),
                            span.end - span.start,
                            &strikethrough,
                        );
                    }
                }
            }
        }
        Ok(())
    })
}

fn paint_line_background(
    origin: Point<Pixels>,
    layout: &LineLayout,
    line_height: Pixels,
    align: TextAlign,
    align_width: Option<Pixels>,
    decoration_runs: &[DecorationRun],
    wrap_boundaries: &[WrapBoundary],
    row_metrics: &[crate::TextRowMetrics],
    window: &mut Window,
    _cx: &mut App,
) -> Result<()> {
    let rows = layout.fragments(wrap_boundaries);
    let width = align_width.unwrap_or(layout.width);
    let height = row_metrics
        .last()
        .map_or(line_height * rows.len(), |row| row.offset + row.height);
    let bounds = Bounds::new(origin, size(layout.width, height));
    window.paint_layer(bounds, |window| {
        for (row_index, row) in rows.iter().enumerate() {
            let x = aligned_row_origin(origin, width, align, row, layout.rtl);
            let (offset, height) = row_metrics
                .get(row_index)
                .map_or((line_height * row_index, line_height), |row| {
                    (row.offset, row.height)
                });
            let y = origin.y + offset;
            for (range, style) in decoration_ranges(decoration_runs) {
                let Some(color) = style.background_color else {
                    continue;
                };
                let selected = range.start.max(row.range.start)..range.end.min(row.range.end);
                for span in layout.selection_ranges(selected) {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(x + span.start - row.x, y),
                            size(span.end - span.start, height),
                        ),
                        color,
                    ));
                }
            }
        }
        Ok(())
    })
}
