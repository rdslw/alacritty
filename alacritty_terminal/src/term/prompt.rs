//! OSC 133 prompt marks and shell state.

use crate::grid::RowMark;
use crate::index::{Column, Line};
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
