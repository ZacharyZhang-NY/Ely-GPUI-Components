use std::ops::Range;

use alacritty_terminal::{
    grid::Scroll,
    index::{Column, Line, Point as Cell, Side},
    selection::{Selection, SelectionType},
    term::TermMode,
};
use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, InteractiveElement, KeyBinding,
    KeyDownEvent, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    Point, ScrollEvent, UTF16Selection, Window, actions,
};

use super::{
    keys,
    links::{Target, links},
    view::{Terminal, TerminalEvent},
};

actions!(
    ely_terminal,
    [
        SendTab,
        SendBackTab,
        Copy,
        Paste,
        Clear,
        PageUp,
        PageDown,
        Find
    ]
);

pub(crate) const CONTEXT: &str = "ElyTerminal";

/// Tab and Shift-Tab go to the program here, ahead of focus moves; the rest are the app's own.
pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let (copy, paste, clear, find) = if cfg!(target_os = "macos") {
        ("cmd-c", "cmd-v", "cmd-k", "cmd-f")
    } else {
        (
            "ctrl-shift-c",
            "ctrl-shift-v",
            "ctrl-shift-k",
            "ctrl-shift-f",
        )
    };
    cx.bind_keys([
        KeyBinding::new("tab", SendTab, context),
        KeyBinding::new("shift-tab", SendBackTab, context),
        KeyBinding::new(copy, Copy, context),
        KeyBinding::new(paste, Paste, context),
        KeyBinding::new(clear, Clear, context),
        KeyBinding::new("shift-pageup", PageUp, context),
        KeyBinding::new("shift-pagedown", PageDown, context),
        KeyBinding::new(find, Find, context),
    ]);
}

pub(crate) fn listen<E: InteractiveElement>(root: E, cx: &mut Context<Terminal>) -> E {
    root.on_action(cx.listener(|terminal, _: &SendTab, _, cx| terminal.input(b"\t".to_vec(), cx)))
        .on_action(
            cx.listener(|terminal, _: &SendBackTab, _, cx| terminal.input(b"\x1b[Z".to_vec(), cx)),
        )
        .on_action(cx.listener(|terminal, _: &Copy, _, cx| terminal.copy(cx)))
        .on_action(cx.listener(|terminal, _: &Paste, _, cx| terminal.paste(cx)))
        .on_action(cx.listener(|terminal, _: &Clear, _, cx| terminal.clear(cx)))
        .on_action(cx.listener(|terminal, _: &PageUp, _, cx| terminal.scroll(Scroll::PageUp, cx)))
        .on_action(
            cx.listener(|terminal, _: &PageDown, _, cx| terminal.scroll(Scroll::PageDown, cx)),
        )
        .on_action(cx.listener(|_, _: &Find, _, cx| cx.emit(TerminalEvent::Find)))
        .on_key_down(cx.listener(Terminal::key_down))
        .on_mouse_down(MouseButton::Left, cx.listener(Terminal::press))
        .on_mouse_move(cx.listener(Terminal::moved))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|terminal, _, _, _| terminal.selecting = false),
        )
        .on_scroll(cx.listener(Terminal::wheel))
        .on_modifiers_changed(
            cx.listener(|terminal, event: &ModifiersChangedEvent, _, cx| {
                if !event.modifiers.platform && terminal.hovered.take().is_some() {
                    cx.notify();
                }
            }),
        )
}

impl Terminal {
    /// Bytes from the keyboard: back to the newest line, no selection.
    pub(crate) fn input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        {
            let mut term = self.term.lock();
            term.scroll_display(Scroll::Bottom);
            term.selection = None;
        }
        self.write(bytes);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let app_cursor = self.term.lock().mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = keys::bytes(&event.keystroke, app_cursor) {
            self.input(bytes, cx);
            cx.stop_propagation();
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.term.lock().selection_to_string() {
            log::info!("terminal: copied {} characters", text.chars().count());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        self.input(keys::paste(&text, bracketed), cx);
    }

    /// The cell under a window point, and which half of it.
    fn cell_at(&self, position: Point<Pixels>) -> (usize, Cell, Side) {
        let x = ((position.x - self.origin.x) / self.cell.width).max(0.0);
        let y = ((position.y - self.origin.y) / self.cell.height).max(0.0);
        let (columns, lines) = self.size;
        let (column, row) = ((x as usize).min(columns - 1), (y as usize).min(lines - 1));
        let offset = self.term.lock().grid().display_offset() as i32;
        let side = if x.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        (
            row,
            Cell::new(Line(row as i32 - offset), Column(column)),
            side,
        )
    }

    /// The link a row shows at `column`: its columns and target.
    pub(crate) fn link_at(
        &self,
        row: usize,
        column: usize,
    ) -> Option<(usize, Range<usize>, Target)> {
        let cells = self.shown.rows.get(row)?.cells();
        let text = cells.concat();
        let column_of = |byte: usize| {
            let mut from = 0;
            cells
                .iter()
                .position(|cell| {
                    let past = from >= byte;
                    from += cell.len();
                    past
                })
                .unwrap_or(cells.len())
        };
        links(&text).into_iter().find_map(|(bytes, target)| {
            let (start, end) = (column_of(bytes.start), column_of(bytes.end));
            (start..end)
                .contains(&column)
                .then_some((row, start..end, target))
        })
    }

    fn press(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (row, cell, side) = self.cell_at(event.position);
        if event.modifiers.platform
            && let Some((_, _, target)) = self.link_at(row, cell.column.0)
        {
            log::info!("terminal: open {target:?}");
            match &target {
                Target::Url(url) => cx.open_url(url),
                Target::Path { .. } => cx.emit(TerminalEvent::Open(target)),
            }
            return;
        }
        let kind = match event.click_count {
            2 => SelectionType::Semantic,
            count if count >= 3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.term.lock().selection = Some(Selection::new(kind, cell, side));
        self.selecting = true;
        cx.notify();
    }

    fn moved(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (row, cell, side) = self.cell_at(event.position);
        if self.selecting && event.pressed_button == Some(MouseButton::Left) {
            if let Some(selection) = self.term.lock().selection.as_mut() {
                selection.update(cell, side);
            }
            cx.notify();
            return;
        }
        let hovered = event
            .modifiers
            .platform
            .then(|| self.link_at(row, cell.column.0))
            .flatten();
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }

    fn wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.rest += event.delta.pixel_delta(self.cell.height).y;
        let lines = (self.rest / self.cell.height).trunc() as i32;
        if lines == 0 {
            return;
        }
        self.rest -= self.cell.height * lines as f32;
        let mode = *self.term.lock().mode();
        if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            let arrow: &[u8] = match (lines > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            self.write(arrow.repeat(lines.unsigned_abs() as usize));
        } else {
            self.scroll(Scroll::Delta(lines), cx);
        }
    }
}

impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input(text.as_bytes().to_vec(), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (row, column, _) = self.shown.cursor?;
        let origin = self.origin
            + gpui::point(
                self.cell.width * column as f32,
                self.cell.height * row as f32,
            );
        Some(Bounds::new(origin, self.cell))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
