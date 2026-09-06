//! OSC 133 prompt marks and shell state.

use std::cmp::{max, min};

use crate::grid::{Dimensions, RowMark};
use crate::index::{Column, Direction, Line};
use crate::term::cell::Flags;
use crate::term::{Term, TermMode};
use crate::vte::ansi::{PromptKind, PromptRedraw, PromptStart};

/// Part of the prompt cycle the shell is in.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum PromptZone {
    /// The shell is drawing its prompt.
    Prompt,
    /// The shell is reading user input.
    Input,
    /// A command is running, or no marker was received yet.
    #[default]
    Output,
}

/// Shell state reported through OSC 133 markers.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct PromptState {
    /// Whether any marker was received since the last reset.
    pub seen: bool,

    /// Current zone of the prompt cycle.
    pub zone: PromptZone,

    /// How the shell repaints its prompt after a resize.
    pub redraw: PromptRedraw,

    /// Mouse click reporting accepted by the shell at its prompt.
    pub click_events: Option<u8>,
}

impl<T> Term<T> {
    /// Shell state reported through OSC 133.
    pub fn prompt_state(&self) -> PromptState {
        self.prompt
    }

    /// Whether the shell is drawing its prompt or reading input on the primary screen.
    ///
    /// This is `false` until the shell reports a marker.
    pub fn cursor_at_prompt(&self) -> bool {
        !self.mode.contains(TermMode::ALT_SCREEN)
            && self.prompt.seen
            && self.prompt.zone != PromptZone::Output
    }

    /// OSC 133 marker of a row.
    pub fn row_mark(&self, line: Line) -> RowMark {
        self.grid[line].mark
    }

    /// Nearest prompt row strictly above (`Left`) or below (`Right`) a line.
    pub fn find_prompt(&self, from: Line, direction: Direction) -> Option<Line> {
        let topmost = self.topmost_line().0;
        let bottommost = self.bottommost_line().0;
        let is_prompt =
            |line: i32| (self.grid[Line(line)].mark == RowMark::Prompt).then_some(Line(line));

        match direction {
            Direction::Left => (topmost..min(from.0, bottommost + 1)).rev().find_map(is_prompt),
            Direction::Right => (max(from.0 + 1, topmost)..=bottommost).find_map(is_prompt),
        }
    }

    /// First and last row of the command output which contains or follows a line.
    ///
    /// The output starts after the prompt and its continuation rows and ends before the next
    /// prompt, without trailing empty rows. Rows above the first prompt count as output.
    pub fn output_bounds(&self, line: Line) -> Option<(Line, Line)> {
        let topmost = self.topmost_line();
        let bottommost = self.bottommost_line();

        // Skip the prompt which starts the command and its continuation rows.
        let prompt = self.find_prompt(line + 1i32, Direction::Left).unwrap_or(topmost);
        let mut start =
            if self.grid[prompt].mark == RowMark::Prompt { prompt + 1i32 } else { prompt };
        while start <= bottommost
            && matches!(
                self.grid[start].mark,
                RowMark::PromptSecondary | RowMark::PromptContinuation
            )
        {
            start += 1i32;
        }

        // Stop before the next prompt and drop trailing empty rows.
        let next_prompt = self.find_prompt(start - 1i32, Direction::Right);
        let mut end = next_prompt.map_or(bottommost, |next| next - 1i32);
        while end >= start && self.grid[end].is_clear() {
            end -= 1i32;
        }

        (start <= end).then_some((start, end))
    }

    /// Handle a prompt start marker.
    pub(super) fn mark_prompt_start(&mut self, prompt: PromptStart) {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            return;
        }

        let line = self.grid.cursor.point.line;
        self.grid[line].mark = match prompt.kind {
            PromptKind::Primary => RowMark::Prompt,
            PromptKind::Continuation => RowMark::PromptSecondary,
        };

        self.prompt.seen = true;
        self.prompt.zone = PromptZone::Prompt;

        // Only `A` and `N` markers carry the resize and click settings.
        if let Some(options) = prompt.options {
            self.prompt.redraw = options.redraw;
            self.prompt.click_events = options.click_events;
        }
    }

    /// Handle a command start or command finished marker.
    pub(super) fn set_prompt_zone(&mut self, zone: PromptZone) {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            return;
        }

        self.prompt.seen = true;
        self.prompt.zone = zone;
    }

    /// Handle a command executed marker.
    pub(super) fn mark_command_executed(&mut self) {
        if self.mode.contains(TermMode::ALT_SCREEN) {
            return;
        }

        self.prompt.seen = true;
        self.prompt.zone = PromptZone::Output;

        // Output starts on this row when the shell moved to a fresh line before the marker.
        let cursor = self.grid.cursor.point;
        if cursor.column == Column(0) {
            self.grid[cursor.line].mark = RowMark::OutputStart;
        }
    }

    /// Clear the prompt before a width change, so the shell can repaint it on empty rows.
    ///
    /// Shells repaint their prompt after a resize. A reflowed right prompt or multi-line prompt
    /// would leave fragments behind the repaint. The `redraw` option of the prompt start marker
    /// states what the shell repaints: the whole prompt, only its last line, or nothing.
    ///
    /// The shell moves the cursor relative to its last position, so the cursor row must not move
    /// during the reflow: the cursor column is clamped to the new width and the prompt is cut off
    /// from a wrapped row above it.
    pub(super) fn clear_prompt_for_redraw(&mut self, columns: usize) {
        if !self.cursor_at_prompt() {
            return;
        }

        let cursor_line = self.grid.cursor.point.line;
        let screen_lines = Line(self.screen_lines() as i32);
        let (start, end) = match self.prompt.redraw {
            PromptRedraw::None => return,
            PromptRedraw::LastLine => (cursor_line, cursor_line + 1i32),
            PromptRedraw::Full => {
                // Stop at the active primary or secondary prompt, preserving accepted input.
                let mut start = cursor_line;
                while self.grid[start].mark == RowMark::PromptContinuation && start > Line(0) {
                    start -= 1i32;
                }
                let is_prompt = matches!(
                    self.grid[start].mark,
                    RowMark::Prompt | RowMark::PromptSecondary | RowMark::PromptContinuation
                );
                if !is_prompt && start != cursor_line {
                    start += 1i32;
                }
                (start, screen_lines)
            },
        };

        // Keep the first cleared row identifiable until the shell sends its markers again; bash
        // repaints its last prompt line without a marker.
        let start_mark = self.grid[start].mark;
        self.grid.reset_region(start..end);
        self.grid[start].mark = start_mark;

        // The prompt starts a fresh line; do not join it with the row above during the reflow.
        if self.prompt.redraw == PromptRedraw::Full && start > self.topmost_line() {
            let last_column = self.last_column();
            self.grid[start - 1i32][last_column].flags.remove(Flags::WRAPLINE);
        }

        // Keep the cursor on its row; the reflow would wrap it beyond the new width.
        self.grid.cursor.input_needs_wrap = false;
        self.grid.cursor.point.column = min(self.grid.cursor.point.column, Column(columns - 1));

        self.mark_fully_damaged();
    }

    /// Extend the prompt onto the row the cursor moved to by a line feed or a wrap.
    ///
    /// This covers multi-line prompts and wrapped input from shells which do not send `k=s`.
    #[inline]
    pub(super) fn inherit_prompt_mark(&mut self) {
        if self.prompt.zone == PromptZone::Output || self.mode.contains(TermMode::ALT_SCREEN) {
            return;
        }

        let line = self.grid.cursor.point.line;
        if self.grid[line].mark == RowMark::None {
            self.grid[line].mark = RowMark::PromptContinuation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::event::VoidListener;
    use crate::grid::{Dimensions, Row};
    use crate::index::Point;
    use crate::term::Config;
    use crate::term::cell::Cell;
    use crate::term::test::TermSize;
    use crate::vi_mode::ViMotion;
    use crate::vte::ansi::{self, PromptOptions};

    fn term(columns: usize, lines: usize, history: usize) -> Term<VoidListener> {
        let config = Config { scrolling_history: history, ..Default::default() };
        Term::new(config, &TermSize::new(columns, lines), VoidListener)
    }

    fn feed(term: &mut Term<VoidListener>, bytes: &[u8]) {
        let mut parser = ansi::Processor::<ansi::StdSyncHandler>::new();
        parser.advance(term, bytes);
    }

    /// All marked rows from the top of the scrollback to the bottom of the screen.
    fn marks(term: &Term<VoidListener>) -> Vec<(i32, RowMark)> {
        (term.topmost_line().0..=term.bottommost_line().0)
            .filter_map(|line| {
                let mark = term.row_mark(Line(line));
                (mark != RowMark::None).then_some((line, mark))
            })
            .collect()
    }

    fn mark_sequence(term: &Term<VoidListener>) -> Vec<RowMark> {
        marks(term).into_iter().map(|(_, mark)| mark).collect()
    }

    fn row_text(term: &Term<VoidListener>, line: i32) -> String {
        let row = &term.grid()[Line(line)];
        (0..row.len()).map(|column| row[Column(column)].c).collect::<String>().trim_end().into()
    }

    /// Output, a two-line prompt with a right prompt, and typed input, on a 20 column screen.
    fn prompt_with_right_prompt(options: &[u8]) -> Term<VoidListener> {
        let mut term = term(20, 5, 10);
        let mut bytes = b"old\r\n\x1b]133;A".to_vec();
        bytes.extend_from_slice(options);
        bytes.extend_from_slice(
            b"\x07first\r\nsecond> \x1b]133;B\x07typed\x1b[3;16HRIGHT\x1b[3;14H",
        );
        feed(&mut term, &bytes);
        assert_eq!(row_text(&term, 1), "first");
        assert_eq!(row_text(&term, 2), "second> typed  RIGHT");
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(13)));
        term
    }

    #[test]
    fn markers_set_zone_and_row_marks() {
        let mut term = term(20, 5, 10);
        assert_eq!(term.prompt_state(), PromptState::default());
        assert!(!term.cursor_at_prompt());

        feed(&mut term, b"\x1b]133;A\x07$ ");
        assert_eq!(term.prompt_state().zone, PromptZone::Prompt);
        assert!(term.prompt_state().seen);
        assert!(term.cursor_at_prompt());
        assert_eq!(term.row_mark(Line(0)), RowMark::Prompt);

        feed(&mut term, b"\x1b]133;B\x07ls\r\n");
        assert_eq!(term.prompt_state().zone, PromptZone::Input);
        assert!(term.cursor_at_prompt());
        assert_eq!(term.row_mark(Line(1)), RowMark::PromptContinuation);

        feed(&mut term, b"\x1b]133;C\x07out\r\n");
        assert_eq!(term.prompt_state().zone, PromptZone::Output);
        assert!(!term.cursor_at_prompt());
        assert_eq!(term.row_mark(Line(1)), RowMark::OutputStart);
        assert_eq!(term.row_mark(Line(2)), RowMark::None);

        feed(&mut term, b"\x1b]133;D;0\x07");
        assert_eq!(term.prompt_state().zone, PromptZone::Output);

        feed(&mut term, b"\x1b]133;A\x07");
        assert_eq!(term.row_mark(Line(2)), RowMark::Prompt);
        assert_eq!(marks(&term), [
            (0, RowMark::Prompt),
            (1, RowMark::OutputStart),
            (2, RowMark::Prompt)
        ]);
    }

    #[test]
    fn continuation_on_linefeed_and_wrap() {
        let mut term = term(10, 12, 0);
        feed(&mut term, b"\x1b]133;A\x07line1\r\nline2\x1b]133;B\x07abcdefghijkl\r\n");
        feed(&mut term, b"\x1b]133;P;k=s\x07> \r\n\x1b]133;A;k=s\x07> \r\n");
        feed(&mut term, b"\x1b]133;C\x07out\r\nmore\r\n\x1b]133;D\x07");

        assert_eq!(marks(&term), [
            (0, RowMark::Prompt),
            (1, RowMark::PromptContinuation),
            (2, RowMark::PromptContinuation),
            (3, RowMark::PromptSecondary),
            (4, RowMark::PromptSecondary),
            (5, RowMark::OutputStart),
        ]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(7), Column(0)));
    }

    #[test]
    fn options_only_from_a_and_n_markers() {
        let mut term = term(20, 5, 0);
        feed(&mut term, b"\x1b]133;A;redraw=last;click_events=1\x07");
        assert_eq!(term.prompt_state(), PromptState {
            seen: true,
            zone: PromptZone::Prompt,
            redraw: PromptRedraw::LastLine,
            click_events: Some(1),
        });

        feed(&mut term, b"\x1b]133;P;k=i\x07");
        assert_eq!(term.prompt_state().redraw, PromptRedraw::LastLine);
        assert_eq!(term.prompt_state().click_events, Some(1));
        assert_eq!(term.row_mark(Line(0)), RowMark::Prompt);

        feed(&mut term, b"\x1b]133;N\x07");
        assert_eq!(term.prompt_state().redraw, PromptRedraw::Full);
        assert_eq!(term.prompt_state().click_events, None);

        let options = PromptOptions { redraw: PromptRedraw::None, click_events: Some(2) };
        term.mark_prompt_start(PromptStart { kind: PromptKind::Primary, options: Some(options) });
        assert_eq!(term.prompt_state().redraw, PromptRedraw::None);
        assert_eq!(term.prompt_state().click_events, Some(2));
    }

    #[test]
    fn output_then_prompt_on_one_row() {
        let mut term = term(20, 5, 0);
        feed(&mut term, b"\x1b]133;C\x07partial\x1b]133;D;1\x07\x1b]133;A\x07$ ");
        assert_eq!(term.row_mark(Line(0)), RowMark::Prompt);
        assert_eq!(term.prompt_state().zone, PromptZone::Prompt);

        // A command executed marker away from the first column does not mark the row.
        feed(&mut term, b"\x1b]133;B\x07\x1b]133;C\x07");
        assert_eq!(term.row_mark(Line(0)), RowMark::Prompt);
        assert_eq!(term.prompt_state().zone, PromptZone::Output);
    }

    #[test]
    fn marks_follow_scrollback_and_clearing() {
        let mut term = term(10, 3, 20);
        feed(&mut term, b"\x1b]133;A\x07$ \x1b]133;B\x07\r\n\x1b]133;C\x07");
        assert_eq!(marks(&term), [(0, RowMark::Prompt), (1, RowMark::OutputStart)]);

        feed(&mut term, b"\r\n\r\n\r\n\r\n");
        assert_eq!(marks(&term), [(-3, RowMark::Prompt), (-2, RowMark::OutputStart)]);

        // Clearing the screen moves the viewport into the scrollback.
        feed(&mut term, b"x\x1b[2J");
        assert_eq!(marks(&term), [(-6, RowMark::Prompt), (-5, RowMark::OutputStart)]);

        feed(&mut term, b"\x1b[3J");
        assert_eq!(marks(&term), []);
    }

    #[test]
    fn marks_evicted_with_history() {
        let mut term = term(10, 3, 2);
        feed(&mut term, b"\x1b]133;A\x07$ \x1b]133;B\x07\r\n\x1b]133;C\x07");
        feed(&mut term, &b"\r\n".repeat(10));
        assert_eq!(marks(&term), []);
    }

    #[test]
    fn alternate_screen_ignores_markers() {
        let mut term = term(10, 3, 0);
        feed(&mut term, b"\x1b]133;A\x07$ \x1b]133;B\x07");
        assert!(term.cursor_at_prompt());

        feed(&mut term, b"\x1b[?1049h");
        assert!(!term.cursor_at_prompt());
        assert_eq!(term.row_mark(Line(0)), RowMark::None);

        feed(&mut term, b"\x1b]133;A\x07\x1b]133;C\x07\r\n");
        assert_eq!(term.row_mark(Line(0)), RowMark::None);
        assert_eq!(term.row_mark(Line(1)), RowMark::None);
        assert_eq!(term.prompt_state().zone, PromptZone::Input);

        feed(&mut term, b"\x1b[?1049l");
        assert!(term.cursor_at_prompt());
        assert_eq!(term.row_mark(Line(0)), RowMark::Prompt);
    }

    #[test]
    fn reset_clears_marks_and_state() {
        let mut term = term(10, 3, 0);
        feed(&mut term, b"\x1b]133;A;redraw=0\x07$ \x1b]133;B\x07\x1bc");
        assert_eq!(term.prompt_state(), PromptState::default());
        assert_eq!(term.row_mark(Line(0)), RowMark::None);
        assert!(!term.cursor_at_prompt());
    }

    #[test]
    fn reflow_keeps_prompt_marks() {
        let mut term = term(20, 6, 20);
        feed(&mut term, b"\x1b]133;A\x07");
        feed(&mut term, &b"p".repeat(30));
        feed(&mut term, b"\x1b]133;B\x07\r\n\x1b]133;C\x07output\r\n");
        assert_eq!(marks(&term), [
            (0, RowMark::Prompt),
            (1, RowMark::PromptContinuation),
            (2, RowMark::OutputStart)
        ]);

        term.resize(TermSize::new(10, 6));
        assert_eq!(mark_sequence(&term), [
            RowMark::Prompt,
            RowMark::PromptContinuation,
            RowMark::PromptContinuation,
            RowMark::OutputStart,
        ]);

        term.resize(TermSize::new(20, 6));
        assert_eq!(mark_sequence(&term), [
            RowMark::Prompt,
            RowMark::PromptContinuation,
            RowMark::OutputStart
        ]);

        // The wrapped prompt joins into one row when it fits.
        term.resize(TermSize::new(40, 6));
        assert_eq!(marks(&term), [(0, RowMark::Prompt), (1, RowMark::OutputStart)]);
    }

    #[test]
    fn reflow_keeps_marks_with_wide_characters() {
        // Five wide characters fill the row, the sixth wraps.
        let mut term = term(10, 6, 20);
        feed(&mut term, b"\x1b]133;A\x07");
        feed(&mut term, "日本語日本語".as_bytes());
        feed(&mut term, b"\x1b]133;B\x07\r\n\x1b]133;C\x07out\r\n");
        assert_eq!(marks(&term), [
            (0, RowMark::Prompt),
            (1, RowMark::PromptContinuation),
            (2, RowMark::OutputStart)
        ]);

        // A wide character straddling the new width moves to the next row with a spacer.
        for columns in [7, 4, 10, 12, 20] {
            term.resize(TermSize::new(columns, 6));
            let sequence = mark_sequence(&term);
            assert_eq!(sequence.first(), Some(&RowMark::Prompt), "{columns} columns");
            assert_eq!(sequence.last(), Some(&RowMark::OutputStart), "{columns} columns");
            assert!(
                sequence[1..sequence.len() - 1]
                    .iter()
                    .all(|mark| *mark == RowMark::PromptContinuation),
                "{columns} columns: {sequence:?}"
            );
        }
        assert_eq!(marks(&term), [(0, RowMark::Prompt), (1, RowMark::OutputStart)]);
    }

    /// Three commands on a four-line screen; the first two scroll into history.
    ///
    /// Rows from the top: prompt, `one`, prompt, `two`, empty, empty, prompt, empty.
    fn three_commands() -> Term<VoidListener> {
        let mut term = term(10, 4, 20);
        feed(
            &mut term,
            b"\x1b]133;A\x07$ \x1b]133;B\x07a\r\n\x1b]133;C\x07one\r\n\x1b]133;D;0\x07",
        );
        feed(&mut term, b"\x1b]133;A\x07$ \x1b]133;B\x07b\r\n\x1b]133;C\x07two\r\n\r\n\r\n");
        feed(&mut term, b"\x1b]133;D;0\x07\x1b]133;A\x07$ \x1b]133;B\x07c\r\n\x1b]133;C\x07");
        assert_eq!(marks(&term), [
            (-4, RowMark::Prompt),
            (-3, RowMark::OutputStart),
            (-2, RowMark::Prompt),
            (-1, RowMark::OutputStart),
            (2, RowMark::Prompt),
            (3, RowMark::OutputStart),
        ]);
        term
    }

    #[test]
    fn find_prompt_across_history() {
        let empty = term(10, 4, 20);
        assert_eq!(empty.find_prompt(Line(3), Direction::Left), None);
        assert_eq!(empty.find_prompt(Line(0), Direction::Right), None);

        let term = three_commands();

        assert_eq!(term.find_prompt(Line(3), Direction::Left), Some(Line(2)));
        assert_eq!(term.find_prompt(Line(2), Direction::Left), Some(Line(-2)));
        assert_eq!(term.find_prompt(Line(-2), Direction::Left), Some(Line(-4)));
        assert_eq!(term.find_prompt(Line(-4), Direction::Left), None);
        assert_eq!(term.find_prompt(Line(10), Direction::Left), Some(Line(2)));

        assert_eq!(term.find_prompt(Line(-4), Direction::Right), Some(Line(-2)));
        assert_eq!(term.find_prompt(Line(-2), Direction::Right), Some(Line(2)));
        assert_eq!(term.find_prompt(Line(2), Direction::Right), None);
        assert_eq!(term.find_prompt(Line(-10), Direction::Right), Some(Line(-4)));
    }

    #[test]
    fn output_bounds_per_command() {
        let term = three_commands();

        // Anchors on the prompt row and on the output row select the same output.
        assert_eq!(term.output_bounds(Line(-4)), Some((Line(-3), Line(-3))));
        assert_eq!(term.output_bounds(Line(-3)), Some((Line(-3), Line(-3))));

        // Trailing empty rows are not part of the output.
        assert_eq!(term.output_bounds(Line(-2)), Some((Line(-1), Line(-1))));
        assert_eq!(term.output_bounds(Line(-1)), Some((Line(-1), Line(-1))));
        assert_eq!(term.output_bounds(Line(1)), Some((Line(-1), Line(-1))));

        // The running command has no output yet.
        assert_eq!(term.output_bounds(Line(2)), None);
        assert_eq!(term.output_bounds(Line(3)), None);
    }

    #[test]
    fn output_bounds_without_prompt_row() {
        // Output before the first prompt.
        let mut before = term(10, 6, 0);
        feed(&mut before, b"pre\r\n\x1b]133;A\x07$ \x1b]133;B\x07a\r\n\x1b]133;C\x07one\r\n");
        assert_eq!(before.output_bounds(Line(0)), Some((Line(0), Line(0))));
        assert_eq!(before.output_bounds(Line(3)), Some((Line(2), Line(2))));

        // The prompt row was evicted, only the wrapped input row remains above the output.
        let mut evicted = term(10, 3, 0);
        feed(&mut evicted, b"\x1b]133;A\x07$ \x1b]133;B\x07abcdefghijkl\r\n\x1b]133;C\x07one\r\n");
        assert_eq!(marks(&evicted), [(0, RowMark::PromptContinuation), (1, RowMark::OutputStart)]);
        assert_eq!(evicted.output_bounds(Line(2)), Some((Line(1), Line(1))));
        assert_eq!(evicted.output_bounds(Line(0)), Some((Line(1), Line(1))));

        // Nothing but empty rows.
        let empty = term(10, 3, 0);
        assert_eq!(empty.output_bounds(Line(1)), None);
    }

    #[test]
    fn vi_prompt_motions() {
        let mut term = three_commands();
        term.toggle_vi_mode();
        term.vi_mode_cursor.point = Point::new(Line(3), Column(2));

        term.vi_motion(ViMotion::PromptUp);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(2), Column(0)));
        term.vi_motion(ViMotion::PromptUp);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(-2), Column(0)));
        assert_eq!(term.grid().display_offset(), 2);
        term.vi_motion(ViMotion::PromptUp);
        term.vi_motion(ViMotion::PromptUp);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(-4), Column(0)));
        assert_eq!(term.grid().display_offset(), 4);

        term.vi_motion(ViMotion::PromptDown);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(-2), Column(0)));
        term.vi_motion(ViMotion::PromptDown);
        term.vi_motion(ViMotion::PromptDown);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(2), Column(0)));
        // The viewport scrolls just far enough to show the prompt.
        assert_eq!(term.grid().display_offset(), 1);
        term.vi_motion(ViMotion::PromptDown);
        assert_eq!(term.vi_mode_cursor.point, Point::new(Line(2), Column(0)));
    }

    #[test]
    fn secondary_prompt_marks_survive_reflow() {
        let mut term = term(10, 8, 10);
        feed(
            &mut term,
            b"\x1b]133;A\x07$ \x1b]133;B\x07command\r\n\
              \x1b]133;P;k=s\x07abcdefghijklmno\x1b]133;B\x07\r\n\x1b]133;C\x07out\r\n",
        );

        for columns in [7, 12, 20] {
            term.resize(TermSize::new(columns, 8));
            let boundaries: Vec<_> = marks(&term)
                .into_iter()
                .filter(|(_, mark)| *mark != RowMark::PromptContinuation)
                .collect();
            assert_eq!(boundaries.iter().map(|(_, mark)| *mark).collect::<Vec<_>>(), [
                RowMark::Prompt,
                RowMark::PromptSecondary,
                RowMark::OutputStart,
            ]);
            let primary = Line(boundaries[0].0);
            let secondary = Line(boundaries[1].0);
            let output = Line(boundaries[2].0);
            assert_eq!(term.find_prompt(term.bottommost_line(), Direction::Left), Some(primary));
            assert_eq!(term.output_bounds(primary), Some((output, output)));
            assert_eq!(term.output_bounds(secondary), Some((output, output)));
            assert_eq!(row_text(&term, output.0), "out");
        }
    }

    #[test]
    fn resize_clears_prompt_for_redraw() {
        let mut term = prompt_with_right_prompt(b"");
        term.resize(TermSize::new(10, 5));

        assert_eq!(row_text(&term, 0), "old");
        assert!((1..5).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(1, RowMark::Prompt)]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(9)));
        assert!(term.cursor_at_prompt());

        // The shell repaints the prompt on the cleared rows.
        feed(&mut term, b"\r\x1b[A\x1b[J\x1b]133;A\x07first\r\nsecond> \x1b]133;B\x07typed");
        assert_eq!(row_text(&term, 1), "first");
        assert_eq!(row_text(&term, 2), "second> ty");
        assert_eq!(row_text(&term, 3), "ped");
        assert_eq!(marks(&term), [
            (1, RowMark::Prompt),
            (2, RowMark::PromptContinuation),
            (3, RowMark::PromptContinuation)
        ]);
    }

    #[test]
    fn resize_preserves_accepted_input_at_secondary_prompt() {
        // zsh only redraws the active PS2, even when the primary prompt spans multiple lines.
        for marker in [b"\x1b]133;A;k=s\x07".as_slice(), b"\x1b]133;P;k=s\x07"] {
            let mut term = term(40, 8, 10);
            feed(
                &mut term,
                b"old\r\n\x1b]133;A\x07FIRST\r\nPROMPT> \x1b]133;B\x07printf '%s\\n' \\\r\n",
            );
            feed(&mut term, marker);
            feed(&mut term, b"CONT> hello");

            term.resize(TermSize::new(30, 8));
            assert_eq!(row_text(&term, 0), "old");
            assert_eq!(row_text(&term, 1), "FIRST", "marker: {marker:?}");
            assert_eq!(row_text(&term, 2), "PROMPT> printf '%s\\n' \\");
            assert!(term.grid()[Line(3)].is_clear());
            assert_eq!(term.row_mark(Line(3)), RowMark::PromptSecondary);

            // Replay zsh's repaint: it never sends the accepted command line again.
            feed(&mut term, b"\r\r\x1b[0m\x1b[27m\x1b[24m\x1b[J");
            feed(&mut term, marker);
            feed(&mut term, b"CONT> hello");
            assert_eq!(row_text(&term, 3), "CONT> hello");
            assert_eq!(term.output_bounds(Line(1)), None);

            feed(&mut term, b"\r\n\x1b]133;C\x07hello\r\n\x1b]133;D;0\x07\x1b]133;A\x07$ ");
            assert_eq!(term.output_bounds(Line(1)), Some((Line(4), Line(4))));
        }
    }

    #[test]
    fn resize_clears_only_latest_secondary_prompt() {
        let mut term = term(20, 8, 10);
        feed(
            &mut term,
            b"\x1b]133;A\x07$ \x1b]133;B\x07one \\\r\n\
              \x1b]133;A;k=s\x07> \x1b]133;B\x07two \\\r\n\
              \x1b]133;P;k=s\x07first\r\n> \x1b]133;B\x07abcdefghijklmnopqrstuvwx",
        );
        assert_eq!(term.grid().cursor.point.line, Line(4));

        // A wrapped, multi-line PS2 and another resize before its repaint.
        for columns in [10, 12] {
            term.resize(TermSize::new(columns, 8));
            assert_eq!(row_text(&term, 0), "$ one \\");
            assert_eq!(row_text(&term, 1), "> two \\");
            assert!((2..8).all(|line| term.grid()[Line(line)].is_clear()));
            assert_eq!(term.row_mark(Line(2)), RowMark::PromptSecondary);
        }

        feed(
            &mut term,
            b"\r\x1b[2A\x1b[J\x1b]133;P;k=s\x07first\r\n> \x1b]133;B\x07abcdefghijklmnopqrstuvwx",
        );
        assert_eq!(row_text(&term, 2), "first");
        assert_eq!(term.output_bounds(Line(0)), None);
        assert_eq!(term.find_prompt(Line(5), Direction::Left), Some(Line(0)));
    }

    #[test]
    fn resize_clears_input_wrapped_at_bottom_margin() {
        let mut term = term(10, 3, 10);
        feed(&mut term, b"old\r\n\x1b]133;A\x07first\r\n> \x1b]133;B\x07abcdefghijkl");
        assert_eq!(row_text(&term, -1), "old");

        // The wrap scrolled the prompt row up; clearing still starts at the prompt row.
        term.resize(TermSize::new(8, 3));
        assert_eq!(row_text(&term, -1), "old");
        assert!((0..3).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(0, RowMark::Prompt)]);
    }

    #[test]
    fn resize_respects_redraw_option() {
        // The shell does not repaint: nothing is cleared, the prompt row reflows.
        let mut term = prompt_with_right_prompt(b";redraw=0");
        term.resize(TermSize::new(10, 5));
        assert_eq!(row_text(&term, -1), "old");
        assert_eq!(row_text(&term, 0), "first");
        assert_eq!(row_text(&term, 1), "second> ty");
        assert_eq!(row_text(&term, 2), "ped  RIGHT");

        // Bash repaints only the last line: only the cursor row is cleared, and it keeps its
        // mark because the repaint carries no marker.
        let mut term = prompt_with_right_prompt(b";redraw=last");
        term.resize(TermSize::new(10, 5));
        assert_eq!(row_text(&term, 1), "first");
        assert!((2..5).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(1, RowMark::Prompt), (2, RowMark::PromptContinuation)]);
        feed(&mut term, b"\rrdslw> \x1b]133;B\x07typed");
        assert_eq!(term.output_bounds(Line(1)), None);
    }

    #[test]
    fn resize_keeps_prompt_when_not_repainted() {
        // Only width changes reflow the prompt.
        let mut term = prompt_with_right_prompt(b"");
        term.resize(TermSize::new(20, 8));
        assert_eq!(row_text(&term, 2), "second> typed  RIGHT");

        // A running command owns the screen.
        let mut term = prompt_with_right_prompt(b"");
        feed(&mut term, b"\r\n\x1b]133;C\x07");
        term.resize(TermSize::new(10, 5));
        assert_eq!(row_text(&term, 0), "first");
        assert_eq!(row_text(&term, 1), "second> ty");

        // The alternate screen is never cleared, and the primary prompt is kept for later.
        let mut term = prompt_with_right_prompt(b"");
        feed(&mut term, b"\x1b[?1049h\x1b[Happ");
        term.resize(TermSize::new(10, 5));
        assert_eq!(row_text(&term, 0), "app");
        feed(&mut term, b"\x1b[?1049l");
        assert_eq!(row_text(&term, 0), "first");
        assert_eq!(row_text(&term, 1), "second> ty");
        assert!(term.cursor_at_prompt());
    }

    #[test]
    fn resize_cuts_prompt_from_wrapped_row_above() {
        // Like zsh's `%` end-of-output marker: the row above the prompt wrapped into it.
        let mut term = term(20, 5, 10);
        feed(&mut term, b"no newline%                   ");
        feed(&mut term, b"\r \r\x1b[J\x1b]133;A\x07first\r\nsecond> \x1b]133;B\x07typed");
        assert_eq!(row_text(&term, 0), "no newline%");
        assert_eq!(row_text(&term, 1), "first");

        term.resize(TermSize::new(30, 5));
        assert_eq!(row_text(&term, 0), "no newline%");
        assert!((1..5).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(1, RowMark::Prompt)]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(13)));
    }

    #[test]
    fn resize_twice_before_repaint_and_with_height_change() {
        let mut term = prompt_with_right_prompt(b"");

        // Width and height change together; the rows are cleared and the cursor row stays.
        term.resize(TermSize::new(10, 8));
        assert_eq!(row_text(&term, 0), "old");
        assert!((1..8).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(1, RowMark::Prompt)]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(9)));

        // A second resize before the shell repaints finds blank rows and the kept mark.
        term.resize(TermSize::new(6, 8));
        assert_eq!(row_text(&term, 0), "old");
        assert!((1..8).all(|line| term.grid()[Line(line)].is_clear()));
        assert_eq!(marks(&term), [(1, RowMark::Prompt)]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(5)));
        assert!(term.cursor_at_prompt());

        // Growing back keeps the state as well.
        term.resize(TermSize::new(20, 8));
        assert_eq!(marks(&term), [(1, RowMark::Prompt)]);
        assert_eq!(term.grid().cursor.point, Point::new(Line(2), Column(5)));
    }

    #[test]
    fn resize_clears_from_cursor_without_prompt_row() {
        // Input from a shell which never sent a prompt start marker.
        let mut term = term(20, 5, 10);
        feed(&mut term, b"old\r\n$ \x1b]133;B\x07typed");
        assert!(term.cursor_at_prompt());
        term.resize(TermSize::new(10, 5));
        assert_eq!(row_text(&term, 0), "old");
        assert!((1..5).all(|line| term.grid()[Line(line)].is_clear()));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn row_mark_serde() {
        let mut row = Row::<Cell>::new(2);
        for mark in [RowMark::Prompt, RowMark::PromptSecondary] {
            row.mark = mark;
            let json = serde_json::to_string(&row).unwrap();
            let restored: Row<Cell> = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.mark, mark);
        }

        // Grids stored before the mark existed still load.
        let mut value: serde_json::Value = serde_json::to_value(&row).unwrap();
        value.as_object_mut().unwrap().remove("mark").unwrap();
        let legacy: Row<Cell> = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.mark, RowMark::None);
    }
}
