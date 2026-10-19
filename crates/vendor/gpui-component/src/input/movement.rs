use gpui::{Context, Point, Window};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::input::{
    InputState, MoveDown, MoveEnd, MoveHome, MoveLeft, MovePageDown, MovePageUp, MoveRight,
    MoveToEnd, MoveToNextWord, MoveToPreviousWord, MoveToStart, MoveUp, RopeExt as _,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveDirection {
    Up,
    Down,
}

impl InputState {
    /// Called after moving the cursor. Updates preferred_column if we know where the cursor now is.
    pub(super) fn update_preferred_column(&mut self) {
        let Some(last_layout) = &self.last_layout else {
            self.preferred_column = None;
            return;
        };
        if last_layout.text_revision != self.text_revision {
            self.preferred_column = None;
            return;
        }

        let point = self.text.offset_to_point(self.cursor());
        let row = point.row.saturating_sub(last_layout.visible_range.start);
        let Some(line) = last_layout.lines.get(row) else {
            self.preferred_column = None;
            return;
        };

        let pos = if self.text.len() == 0 {
            gpui::point(line.empty_caret_x(), gpui::px(0.))
        } else {
            let Some(pos) = line.caret_position(
                point.column,
                self.cursor_affinity,
                last_layout.line_height,
                self.cursor_upstream,
            ) else {
                self.preferred_column = None;
                return;
            };
            pos
        };

        // Browser vertical movement preserves the pixel-rounded left edge of
        // its one-pixel caret, rather than the fractional glyph insertion edge.
        let width = last_layout
            .wrap_width
            .unwrap_or(self.input_bounds.size.width);
        self.preferred_column = Some((
            pos.x
                .floor()
                .max(gpui::px(0.))
                .min((width - gpui::px(1.)).max(gpui::px(0.))),
            point.column,
        ));
    }

    /// Move the cursor to the given offset.
    ///
    /// The offset is the UTF-8 offset.
    ///
    /// Ensure the offset use self.next_boundary or self.previous_boundary to get the correct offset.
    pub(crate) fn move_to(
        &mut self,
        offset: usize,
        direction: Option<MoveDirection>,
        cx: &mut Context<Self>,
    ) {
        let offset = offset.clamp(0, self.text.len());
        self.cursor_affinity = gpui::TextAffinity::Editing;
        self.cursor_upstream = false;
        self.selected_range = (offset..offset).into();
        self.scroll_to(offset, direction, cx);
        self.pause_blink_cursor(cx);
        self.update_preferred_column();
        self.hide_context_menu(cx);
        self.clear_inline_completion(cx);
        cx.notify()
    }

    /// Move the cursor vertically by one line (up or down) while preserving the column if possible.
    ///
    /// move_lines: Number of lines to move vertically (positive for down, negative for up).
    pub(super) fn move_vertical(
        &mut self,
        move_lines: isize,
        select: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode.is_single_line() {
            return;
        }
        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let was_preferred_column = self.preferred_column;

        let mut display_point = self.cursor_display_point();
        if (move_lines < 0 && display_point.row < move_lines.unsigned_abs())
            || (move_lines > 0
                && display_point.row + move_lines as usize >= self.text_wrapper.len())
        {
            let offset = if move_lines < 0 { 0 } else { self.text.len() };
            if select {
                self.select_to(offset, cx);
            } else {
                self.move_to(offset, None, cx);
            }
            self.preferred_column = was_preferred_column;
            return;
        }
        display_point.row = display_point.row.saturating_add_signed(move_lines);
        display_point.column = 0;
        let mut new_offset = self.text_wrapper.display_point_to_offset(display_point);
        let mut affinity = gpui::TextAffinity::Primary;
        let mut upstream = false;

        if let Some((preferred_x, column)) = was_preferred_column {
            // Get display point again to update local_row.
            let mut next_display_point = self.text_wrapper.offset_to_display_point(new_offset);
            next_display_point.column = 0;
            let next_point = self.text_wrapper.display_point_to_point(next_display_point);
            let line_start_offset = self.text.line_start_offset(next_point.row);

            // If in visible range, prefer to use position to get column.
            if let Some(line) = last_layout.line(next_point.row) {
                if let Some((x, visual, wrapped)) = line.editing_index_for_position(
                    Point {
                        x: preferred_x,
                        y: next_display_point.local_row * last_layout.line_height,
                    },
                    last_layout.line_height,
                ) {
                    new_offset = line_start_offset + x;
                    affinity = visual;
                    upstream = wrapped;
                }
            } else {
                // Not in visible range, use column directly.
                let max_line_len = self.text.slice_line(next_point.row).len();
                new_offset = line_start_offset + column.min(max_line_len);
            }
        }

        self.pause_blink_cursor(cx);
        let direction = if move_lines < 0 {
            MoveDirection::Up
        } else {
            MoveDirection::Down
        };
        if select {
            self.select_to(new_offset, cx);
            self.scroll_to(new_offset, Some(direction), cx);
        } else {
            self.move_to(new_offset, Some(direction), cx);
        }
        self.cursor_affinity = affinity;
        self.cursor_upstream = upstream;
        // Set back the preferred_column
        self.preferred_column = was_preferred_column;
        cx.notify();
    }

    pub(super) fn paragraph_rtl(&self) -> bool {
        self.last_layout
            .as_ref()
            .and_then(|layout| layout.line(self.text.offset_to_point(self.cursor()).row))
            .and_then(|line| line.wrapped_lines.first())
            .is_some_and(|line| line.rtl)
    }

    // Chromium input arrows move by logical graphemes relative to paragraph direction.
    pub(super) fn adjacent_caret(&self, right: bool) -> usize {
        let cursor = self.cursor();
        let point = self.text.offset_to_point(cursor);
        let text = self.text.slice_line(point.row).to_string();
        let boundary = text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(text.len()))
            .find(|index| *index >= point.column)
            .unwrap_or(text.len());
        let forward = right != self.paragraph_rtl();
        let adjacent = if forward {
            text.grapheme_indices(true)
                .map(|(index, _)| index)
                .chain(std::iter::once(text.len()))
                .find(|index| *index > boundary)
        } else {
            text.grapheme_indices(true)
                .map(|(index, _)| index)
                .take_while(|index| *index < boundary)
                .last()
        };
        adjacent.map_or_else(
            || {
                if forward {
                    self.next_boundary(cursor)
                } else {
                    self.previous_boundary(cursor)
                }
            },
            |index| self.text.line_start_offset(point.row) + index,
        )
    }

    pub(super) fn left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            self.adjacent_caret(false)
        } else if self.paragraph_rtl() {
            self.selected_range.end
        } else {
            self.selected_range.start
        };
        self.move_to(offset, None, cx);
    }

    pub(super) fn right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.selected_range.is_empty() {
            self.adjacent_caret(true)
        } else if self.paragraph_rtl() {
            self.selected_range.start
        } else {
            self.selected_range.end
        };
        self.move_to(offset, None, cx);
    }

    pub(super) fn up(&mut self, action: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
            return;
        }

        if self.mode.is_single_line() {
            return;
        }

        if !self.selected_range.is_empty() {
            self.move_to(
                self.previous_boundary(self.selected_range.start.saturating_sub(1)),
                Some(MoveDirection::Up),
                cx,
            );
        }
        self.pause_blink_cursor(cx);
        self.move_vertical(-1, false, window, cx);
    }

    pub(super) fn down(&mut self, action: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
            return;
        }

        if self.mode.is_single_line() {
            return;
        }

        if !self.selected_range.is_empty() {
            self.move_to(
                self.next_boundary(self.selected_range.end.saturating_sub(1)),
                Some(MoveDirection::Down),
                cx,
            );
        }

        self.pause_blink_cursor(cx);
        self.move_vertical(1, false, window, cx);
    }

    pub(super) fn page_up(&mut self, _: &MovePageUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_single_line() {
            return;
        }

        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let display_lines = (self.input_bounds.size.height / last_layout.line_height) as isize;
        self.move_vertical(-display_lines, false, window, cx);
    }

    pub(super) fn page_down(
        &mut self,
        _: &MovePageDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode.is_single_line() {
            return;
        }

        let Some(last_layout) = &self.last_layout else {
            return;
        };

        let display_lines = (self.input_bounds.size.height / last_layout.line_height) as isize;
        self.move_vertical(display_lines, false, window, cx);
    }

    pub(super) fn home(&mut self, _: &MoveHome, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        let offset = self.start_of_line();
        self.move_to(offset, Some(MoveDirection::Up), cx);
    }

    pub(super) fn end(&mut self, _: &MoveEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.pause_blink_cursor(cx);
        let offset = self.end_of_line();
        self.move_to(offset, Some(MoveDirection::Down), cx);
        self.cursor_upstream = true;
        self.update_preferred_column();
    }

    pub(super) fn move_to_start(
        &mut self,
        _: &MoveToStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(0, None, cx);
    }

    pub(super) fn move_to_end(&mut self, _: &MoveToEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), None, cx);
    }

    pub(super) fn move_to_previous_word(
        &mut self,
        _: &MoveToPreviousWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.previous_start_of_word();
        self.move_to(offset, None, cx);
    }

    pub(super) fn move_to_next_word(
        &mut self,
        _: &MoveToNextWord,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.next_end_of_word();
        self.move_to(offset, None, cx);
    }
}
