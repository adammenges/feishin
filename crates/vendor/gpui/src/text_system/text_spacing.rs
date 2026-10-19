use super::{LineLayout, TextFragment, TextRun, WindowTextSystem, WrapBoundary, WrappedLineLayout};
use crate::{Pixels, SharedString, TextWrap, px};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{ops::Range, sync::Arc};
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

/// Contextual spacing of fullwidth punctuation. SpaceAll preserves GPUI's
/// original behavior; browser-compatible interfaces can inherit Normal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum TextSpacingTrim {
    /// Collapse adjacent punctuation while preserving fullwidth line starts.
    Normal,
    /// Preserve forced line starts and trim soft-wrapped opening punctuation.
    SpaceFirst,
    /// Trim opening punctuation at every line start.
    TrimStart,
    /// Preserve all fullwidth punctuation spacing.
    #[default]
    SpaceAll,
}

impl TextSpacingTrim {
    /// Parse a supported CSS keyword. Global CSS keywords are resolved by the caller.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "space-first" => Some(Self::SpaceFirst),
            "trim-start" => Some(Self::TrimStart),
            "space-all" => Some(Self::SpaceAll),
            _ => None,
        }
    }
}

pub(super) fn allows_trim(run: &TextRun) -> bool {
    run.text_spacing_trim != TextSpacingTrim::SpaceAll && !has_spacing_features(&run.font.features)
}

pub(super) fn has_spacing_features(features: &super::FontFeatures) -> bool {
    features.0.iter().any(|(tag, value)| {
        *value != 0
            && matches!(
                tag.as_str(),
                "halt" | "hwid" | "palt" | "pwid" | "qwid" | "twid" | "valt" | "vhal" | "vpal"
            )
    })
}

pub(super) fn opening(character: char) -> bool {
    matches!(character, '\u{2018}' | '\u{201c}')
        || (matches!(character as u32, 0x3000..=0x303f | 0xff00..=0xffef)
            && character.general_category() == GeneralCategory::OpenPunctuation)
}

pub(super) fn closing(character: char) -> bool {
    matches!(
        character,
        '\u{2019}' | '\u{201d}' | '、' | '。' | '，' | '．'
    ) || (matches!(character as u32, 0x3000..=0x303f | 0xff00..=0xffef)
        && character.general_category() == GeneralCategory::ClosePunctuation)
}

fn closing_boundary(character: char) -> bool {
    closing(character) && !matches!(character, '、' | '。' | '，' | '．')
}

pub(super) fn has_cjk(text: &str) -> bool {
    text.chars().any(cjk_character)
}

fn cjk_character(character: char) -> bool {
    matches!(character as u32,
        0x3000..=0x9fff | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xff00..=0xffef)
}

pub(super) fn slice_runs(runs: &[TextRun], range: Range<usize>, wrapped: bool) -> Vec<TextRun> {
    let mut start = 0;
    runs.iter()
        .filter_map(|run| {
            let end = start + run.len;
            let overlap = range.start.max(start)..range.end.min(end);
            start = end;
            if overlap.is_empty() {
                return None;
            }
            let mut run = run.clone();
            run.len = overlap.len();
            if wrapped && run.text_spacing_trim == TextSpacingTrim::SpaceFirst {
                run.text_spacing_trim = TextSpacingTrim::TrimStart;
            }
            Some(run)
        })
        .collect()
}

impl WindowTextSystem {
    fn layout_fragment_in_context(
        &self,
        text: &str,
        range: Range<usize>,
        font_size: Pixels,
        runs: &[TextRun],
        bidi: &unicode_bidi::ParagraphBidiInfo<'_>,
    ) -> Arc<LineLayout> {
        if !bidi.has_rtl() && !bidi.paragraph_level.is_rtl() {
            return self.layout_line(&text[range], font_size, runs, None);
        }
        let (_, mut visual) = bidi.visual_runs(range.clone());
        let controls_start = range.start
            + text[range.clone()]
                .trim_end_matches(|character| {
                    unicode_bidi::bidi_class(character) == unicode_bidi::BidiClass::BN
                })
                .len();
        if controls_start < range.end
            && let Some(previous) = visual.iter_mut().find(|run| run.end == controls_start)
        {
            // Blink attaches trailing zero-width controls to their original item,
            // rather than moving them to the paragraph edge under bidi rule L1.
            previous.end = range.end;
            visual.retain(|run| run.start < controls_start);
        }
        let mut combined = LineLayout {
            font_size,
            len: range.len(),
            rtl: bidi.paragraph_level.is_rtl(),
            ..Default::default()
        };
        for source in visual {
            let relative = source.start - range.start..source.end - range.start;
            let mut segment_runs = slice_runs(runs, relative.clone(), false);
            for run in &mut segment_runs {
                // L1 moves trailing whitespace to the paragraph's visual edge,
                // while Blink retains the original item's direction for editing.
                run.text_direction = if bidi.levels[source.start].is_rtl() {
                    super::TextDirection::Rtl
                } else {
                    super::TextDirection::Ltr
                };
            }
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            let contextual = text[source.clone()].chars().any(|character| {
                unicode_bidi::bidi_class(character) == unicode_bidi::BidiClass::AL
            });
            #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
            let contextual = false;
            let layout = if contextual {
                let mut font_runs = Vec::new();
                let mut offset = 0;
                let fragment = &text[source.clone()];
                for run in &segment_runs {
                    self.append_font_runs(
                        fragment,
                        offset..offset + run.len,
                        font_size,
                        run,
                        true,
                        &mut font_runs,
                    );
                    offset += run.len;
                }
                let mut layout = self.platform_text_system.layout_line_with_context(
                    fragment,
                    font_size,
                    &font_runs,
                    &text[..source.start],
                    &text[source.end..],
                );
                layout.apply_letter_spacing(fragment, &font_runs);
                Arc::new(layout)
            } else {
                self.layout_line(&text[source], font_size, &segment_runs, None)
            };
            for mut run in layout.runs.iter().cloned() {
                for glyph in &mut run.glyphs {
                    glyph.index += relative.start;
                    glyph.position.x += combined.width;
                }
                combined.runs.push(run);
            }
            combined
                .cells
                .extend(layout.cells.iter().cloned().map(|mut cell| {
                    cell.range.start += relative.start;
                    cell.range.end += relative.start;
                    cell.start_x += combined.width;
                    cell.end_x += combined.width;
                    cell
                }));
            combined.width += layout.width;
            combined.ascent = combined.ascent.max(layout.ascent);
            combined.descent = combined.descent.max(layout.descent);
        }
        let cells = std::mem::take(&mut combined.cells);
        combined.set_cells(&text[range.clone()], cells);
        combined.bidi_levels = bidi.levels[range]
            .iter()
            .map(|level| level.number())
            .collect();
        Arc::new(combined)
    }

    pub(super) fn layout_cjk_wrapped_line(
        &self,
        text: &SharedString,
        font_size: Pixels,
        runs: &[TextRun],
        width: Pixels,
        max_lines: usize,
        wrap: TextWrap,
    ) -> Arc<WrappedLineLayout> {
        let mut breaks = unicode_linebreak::linebreaks(text)
            .map(|(index, _)| index)
            .filter(|index| {
                let before = text[..*index].chars().next_back();
                if matches!(wrap, TextWrap::KeepAll | TextWrap::KeepAllBreakWord) {
                    let letter = |character: char| {
                        matches!(
                            character.general_category(),
                            GeneralCategory::UppercaseLetter
                                | GeneralCategory::LowercaseLetter
                                | GeneralCategory::TitlecaseLetter
                                | GeneralCategory::ModifierLetter
                                | GeneralCategory::OtherLetter
                                | GeneralCategory::DecimalNumber
                                | GeneralCategory::LetterNumber
                                | GeneralCategory::OtherNumber
                        ) && unicode_linebreak::break_property(character as u32)
                            != unicode_linebreak::BreakClass::ComplexContext
                    };
                    let before = text[..*index]
                        .graphemes(true)
                        .next_back()
                        .and_then(|grapheme| grapheme.chars().next());
                    let after = text[*index..].chars().next();
                    if before.is_some_and(letter) && after.is_some_and(letter) {
                        return false;
                    }
                }
                // Blink retains an internal slash with its following alphabetic word.
                before != Some('/')
                    || !text[*index..].chars().next().is_some_and(|character| {
                        character.is_alphanumeric() && !cjk_character(character)
                    })
            })
            .collect::<Vec<_>>();
        // Blink's normal line breaking permits a word-internal hyphen break.
        breaks.extend(text.char_indices().filter_map(|(index, ch)| {
            (matches!(ch, '-' | '\u{2010}')
                && index != 0
                && text[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|c| !c.is_whitespace()))
            .then_some(index + ch.len_utf8())
        }));
        if wrap == TextWrap::BreakAll {
            breaks.extend(text.grapheme_indices(true).filter_map(|(index, _)| {
                let before = text[..index].chars().next_back()?;
                let after = text[index..].chars().next()?;
                (!opening(before)
                    && !closing(after)
                    && !before.is_whitespace()
                    && !after.is_whitespace())
                .then_some(index)
            }));
        }
        breaks.sort_unstable();
        breaks.dedup();
        let mut combined = LineLayout {
            font_size,
            len: text.len(),
            ..Default::default()
        };
        let mut boundaries = SmallVec::new();
        let mut start = 0;
        let direction = runs
            .first()
            .and_then(|run| run.text_direction.rtl())
            .map(|rtl| {
                if rtl {
                    unicode_bidi::Level::rtl()
                } else {
                    unicode_bidi::Level::ltr()
                }
            });
        let bidi = unicode_bidi::ParagraphBidiInfo::new(text, direction);
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let paragraph = self.layout_line(text, font_size, runs, None);
        while start < text.len() {
            let mut selected = None;
            for end in breaks.iter().copied().filter(|end| *end > start) {
                let end = if boundaries.len() + usize::from(start != 0) + 1 >= max_lines {
                    text.len()
                } else {
                    end
                };
                let fragment = &text[start..end];
                let mut fragment_runs = slice_runs(runs, start..end, start != 0);
                let mut layout = self.layout_fragment_in_context(
                    text,
                    start..end,
                    font_size,
                    &fragment_runs,
                    &bidi,
                );
                let mut fits = layout.width <= width;
                if !fits
                    && (!has_cjk(fragment)
                        || matches!(wrap, TextWrap::KeepAll | TextWrap::KeepAllBreakWord))
                {
                    let end = fragment.trim_end_matches([' ', '\t']).len();
                    let hanging = layout
                        .selection_ranges(end..fragment.len())
                        .iter()
                        .map(|span| span.end - span.start)
                        .fold(px(0.), |total, width| total + width);
                    fits = layout.width - hanging <= width;
                }
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                let start_prefix = (start != 0
                    && fragment.chars().next().is_some_and(opening)
                    && fragment_runs
                        .first()
                        .is_some_and(|run| run.text_spacing_trim == TextSpacingTrim::TrimStart))
                .then(|| {
                    fragment
                        .char_indices()
                        .find(|(_, character)| !opening(*character))
                        .map_or(fragment.len(), |(index, _)| index)
                });
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                let reshapes_close = fits
                    && end < text.len()
                    && text[end..].chars().next().is_some_and(opening)
                    && fragment.chars().next_back().is_some_and(closing_boundary)
                    && fragment_runs.last().is_some_and(allows_trim);
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                if reshapes_close {
                    // Breaking before an opening punctuation restores the preceding
                    // close. Chromium tests this reshaped end without end trimming.
                    fits = self.line_end_fits(
                        &layout,
                        &paragraph,
                        start,
                        end - fragment.chars().next_back().unwrap().len_utf8(),
                        start_prefix,
                        width,
                    );
                    if !fits {
                        if selected.is_none() {
                            selected = Some((end, layout));
                        }
                        break;
                    }
                }
                if !fits
                    && let Some(character) = fragment.chars().next_back()
                    && closing_boundary(character)
                    && fragment_runs.last().is_some_and(allows_trim)
                {
                    let last = fragment_runs.last_mut().unwrap();
                    let mut punctuation = last.clone();
                    punctuation.len = character.len_utf8();
                    last.len -= punctuation.len;
                    let mut features = punctuation.font.features.0.as_ref().clone();
                    features.retain(|(tag, _)| tag != "halt");
                    features.push(("halt".into(), 1));
                    punctuation.font.features = super::FontFeatures(Arc::new(features));
                    fragment_runs.push(punctuation);
                    let trimmed = self.layout_fragment_in_context(
                        text,
                        start..end,
                        font_size,
                        &fragment_runs,
                        &bidi,
                    );
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    let fits_end = self.line_end_fits(
                        &trimmed,
                        &paragraph,
                        start,
                        end - character.len_utf8(),
                        start_prefix,
                        width,
                    );
                    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
                    let fits_end = trimmed.width <= width;
                    if fits_end {
                        fits = true;
                        // Electron's pinned Chromium restores the paragraph-final
                        // bracket's full advance after testing its trimmed fit.
                        if end < text.len() {
                            layout = trimmed;
                        }
                    }
                }
                if !fits
                    && selected.is_none()
                    && matches!(wrap, TextWrap::BreakWord | TextWrap::KeepAllBreakWord)
                {
                    // Emergency breaks only apply when the first whole word does not fit.
                    // Shape in paragraph context so Arabic forms and bidi neutrals survive.
                    for (local_end, _) in fragment.grapheme_indices(true).skip(1) {
                        let end = start + local_end;
                        let mut runs = slice_runs(runs, start..end, start != 0);
                        if let Some(character) = text[..end].chars().next_back()
                            && closing(character)
                            && let Some(last) = runs.last_mut().filter(|run| allows_trim(run))
                            && should_trim(text, end - character.len_utf8(), last.text_spacing_trim)
                        {
                            // Emergency breaks retain spacing that was shaped in
                            // the paragraph, including an adjacent closing mark.
                            let mut punctuation = last.clone();
                            punctuation.len = character.len_utf8();
                            last.len -= punctuation.len;
                            let mut features = punctuation.font.features.0.as_ref().clone();
                            features.push(("halt".into(), 1));
                            punctuation.font.features = super::FontFeatures(Arc::new(features));
                            runs.push(punctuation);
                        }
                        let candidate = self.layout_fragment_in_context(
                            text,
                            start..end,
                            font_size,
                            &runs,
                            &bidi,
                        );
                        if candidate.width <= width || selected.is_none() {
                            selected = Some((end, candidate));
                        } else {
                            break;
                        }
                    }
                }
                if fits || selected.is_none() {
                    selected = Some((end, layout));
                }
                if !fits || end == text.len() {
                    break;
                }
            }
            let (end, layout) = selected.unwrap();
            combined.rtl = layout.rtl;
            if start != 0 {
                boundaries.push(WrapBoundary {
                    run_ix: combined.runs.len(),
                    glyph_ix: 0,
                });
            }
            for mut run in layout.runs.iter().cloned() {
                for glyph in &mut run.glyphs {
                    glyph.index += start;
                    glyph.position.x += combined.width;
                }
                combined.runs.push(run);
            }
            combined.visual_lines.push(TextFragment {
                range: start..end,
                x: combined.width,
                width: layout.width,
            });
            combined.carets.retain(|caret| caret.index != start);
            combined
                .carets
                .extend(layout.carets.iter().cloned().map(|mut caret| {
                    caret.index += start;
                    caret.x += combined.width;
                    caret.secondary_x += combined.width;
                    caret
                }));
            combined
                .cells
                .extend(layout.cells.iter().cloned().map(|mut cell| {
                    cell.range.start += start;
                    cell.range.end += start;
                    cell.start_x += combined.width;
                    cell.end_x += combined.width;
                    cell
                }));
            combined.width += layout.width;
            combined.bidi_levels.extend_from_slice(&layout.bidi_levels);
            combined.ascent = combined.ascent.max(layout.ascent);
            combined.descent = combined.descent.max(layout.descent);
            start = end;
        }
        Arc::new(WrappedLineLayout {
            row_metrics: Vec::new(),
            unwrapped_layout: Arc::new(combined),
            wrap_boundaries: boundaries,
            wrap_width: Some(width.max(px(0.))),
        })
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn line_end_fits(
        &self,
        layout: &LineLayout,
        paragraph: &LineLayout,
        start: usize,
        end_prefix: usize,
        start_prefix: Option<usize>,
        available: Pixels,
    ) -> bool {
        // Chromium ceilings cached paragraph positions to 1/64px, then tests
        // the reshaped end against their difference. Preserve that rounding
        // across soft lines. FreeType scales 26.6 pixel sizes using a 16.16
        // factor and rounds linear advances to 16.16 pixels before layout.
        let position_error = |source: &LineLayout, index| {
            let mut glyphs = source
                .runs
                .iter()
                .flat_map(|run| {
                    let units = self.units_per_em(run.font_id) as f32;
                    run.glyphs
                        .iter()
                        .map(move |glyph| (glyph, units, run.font_size))
                })
                .collect::<Vec<_>>();
            glyphs.sort_by_key(|(glyph, _, _)| glyph.position.x);
            let mut position = 0_f64;
            let mut nominal = 0_f64;
            for (ix, (glyph, units, font_size)) in glyphs.iter().enumerate() {
                if glyph.index >= index {
                    continue;
                }
                let next = glyphs
                    .get(ix + 1)
                    .map_or(source.width, |(glyph, _, _)| glyph.position.x);
                let font_units = f64::from((next - glyph.position.x).0) * f64::from(*units)
                    / f64::from(font_size.0);
                let scale = (f64::from(font_size.0) * 64. * 65536. / f64::from(*units)).round();
                position += (font_units * scale / 64.).round() / 65536.;
                nominal += f64::from((next - glyph.position.x).0);
            }
            (position * 64.).ceil() / 64. - nominal
        };
        f64::from(layout.width.0) + position_error(paragraph, end_prefix)
            - position_error(paragraph, start)
            + start_prefix.map_or(0., |index| position_error(layout, index))
            <= (f64::from(available.0) * 64.).floor() / 64.
    }
}

fn middle_or_space(character: char) -> bool {
    matches!(
        character,
        '\u{00b7}' | '\u{2027}' | '・' | '：' | '；' | '　'
    )
}

pub(super) fn should_trim(text: &str, index: usize, spacing: TextSpacingTrim) -> bool {
    if spacing == TextSpacingTrim::SpaceAll {
        return false;
    }
    let character = text[index..].chars().next().unwrap();
    if opening(character) {
        if index == 0 {
            return spacing == TextSpacingTrim::TrimStart;
        }
        let previous = text[..index].chars().next_back().unwrap();
        opening(previous)
            || closing(previous)
            || middle_or_space(previous)
            || previous.general_category() == GeneralCategory::OpenPunctuation
    } else if closing(character) {
        text[index + character.len_utf8()..]
            .chars()
            .next()
            .is_some_and(|next| {
                closing(next)
                    || middle_or_space(next)
                    || next.general_category() == GeneralCategory::ClosePunctuation
            })
    } else {
        false
    }
}
