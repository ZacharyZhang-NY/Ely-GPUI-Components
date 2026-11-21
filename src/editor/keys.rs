use super::{buffer::Buffer, cursor::Motion, state::CodeEditor};
use gpui::{
    Action, App, Context, InteractiveElement, KeyBinding, KeyBindingContextPredicate, Window,
    actions,
};
use std::rc::Rc;

actions!(
    ely_editor,
    [
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectLineStart,
        SelectLineEnd,
        DocumentStart,
        DocumentEnd,
        SelectDocumentStart,
        SelectDocumentEnd,
        PageUp,
        PageDown,
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteLineLeft,
        Newline,
        Indent,
        Outdent,
        Undo,
        Redo,
        SelectAll,
        Copy,
        Cut,
        Paste,
        AddCursorAbove,
        AddCursorBelow,
        SelectNext,
        SingleCursor,
        ToggleComment,
        Fold,
        Unfold,
    ]
);

macro_rules! word_key {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("alt-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

fn binding<A: Action>(cx: &App, strokes: &str, action: A, context: Option<&str>) -> KeyBinding {
    let predicate =
        context.map(|c| Rc::new(KeyBindingContextPredicate::parse(c).expect("context")));
    KeyBinding::load(strokes, Box::new(action), predicate, false, None, cx)
        .unwrap_or_else(|error| panic!("bad keystroke {strokes:?}: {error}"))
}

pub(crate) const CONTEXT: &str = "ElyEditor";

/// Binds the editor's keys. `init` calls it.
pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new(word_key!("left"), WordLeft, context),
        KeyBinding::new(word_key!("right"), WordRight, context),
        KeyBinding::new(word_key!("shift-left"), SelectWordLeft, context),
        KeyBinding::new(word_key!("shift-right"), SelectWordRight, context),
        KeyBinding::new("home", LineStart, context),
        KeyBinding::new("end", LineEnd, context),
        KeyBinding::new("shift-home", SelectLineStart, context),
        KeyBinding::new("shift-end", SelectLineEnd, context),
        KeyBinding::new("pageup", PageUp, context),
        KeyBinding::new("pagedown", PageDown, context),
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("shift-backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new(word_key!("backspace"), DeleteWordLeft, context),
        KeyBinding::new("enter", Newline, context),
        KeyBinding::new("tab", Indent, context),
        KeyBinding::new("shift-tab", Outdent, context),
        binding(cx, "secondary-z", Undo, context),
        binding(cx, "secondary-shift-z", Redo, context),
        binding(cx, "secondary-a", SelectAll, context),
        binding(cx, "secondary-c", Copy, context),
        binding(cx, "secondary-x", Cut, context),
        binding(cx, "secondary-d", SelectNext, context),
        binding(cx, "escape", SingleCursor, context),
        binding(cx, "secondary-/", ToggleComment, context),
        binding(cx, "secondary-alt-[", Fold, context),
        binding(cx, "secondary-alt-]", Unfold, context),
    ];
    // The browser's paste event reaches the input handler.
    if !cfg!(target_family = "wasm") {
        bindings.push(binding(cx, "secondary-v", Paste, context));
    }
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-left", LineStart, context),
            KeyBinding::new("cmd-right", LineEnd, context),
            KeyBinding::new("cmd-shift-left", SelectLineStart, context),
            KeyBinding::new("cmd-shift-right", SelectLineEnd, context),
            KeyBinding::new("cmd-up", DocumentStart, context),
            KeyBinding::new("cmd-down", DocumentEnd, context),
            KeyBinding::new("cmd-shift-up", SelectDocumentStart, context),
            KeyBinding::new("cmd-shift-down", SelectDocumentEnd, context),
            KeyBinding::new("cmd-backspace", DeleteLineLeft, context),
            KeyBinding::new("cmd-alt-up", AddCursorAbove, context),
            KeyBinding::new("cmd-alt-down", AddCursorBelow, context),
        ]);
    } else {
        bindings.extend([
            KeyBinding::new("ctrl-home", DocumentStart, context),
            KeyBinding::new("ctrl-end", DocumentEnd, context),
            KeyBinding::new("ctrl-shift-home", SelectDocumentStart, context),
            KeyBinding::new("ctrl-shift-end", SelectDocumentEnd, context),
            KeyBinding::new("ctrl-alt-up", AddCursorAbove, context),
            KeyBinding::new("ctrl-alt-down", AddCursorBelow, context),
            KeyBinding::new("ctrl-y", Redo, context),
        ]);
    }
    cx.bind_keys(bindings);
}

/// Rows a page key moves.
const PAGE: isize = 20;

fn line_left(buffer: &Buffer, at: usize) -> std::ops::Range<usize> {
    buffer.line_range(buffer.line_of(at)).start..at
}

/// Wires every action to the editor that renders `root`.
pub(crate) fn listen<E: InteractiveElement>(root: E, cx: &mut Context<CodeEditor>) -> E {
    macro_rules! on {
        ($root:expr, $action:ty, $body:expr) => {
            $root.on_action(cx.listener(
                move |editor: &mut CodeEditor,
                      _: &$action,
                      window: &mut Window,
                      cx: &mut Context<CodeEditor>| {
                    let run: fn(&mut CodeEditor, &mut Window, &mut Context<CodeEditor>) = $body;
                    run(editor, window, cx)
                },
            ))
        };
    }
    let root = on!(root, Left, |e, _, cx| e.motion(Motion::Left, false, cx));
    let root = on!(root, Right, |e, _, cx| e.motion(Motion::Right, false, cx));
    let root = on!(root, Up, |e, _, cx| e.motion(Motion::Up, false, cx));
    let root = on!(root, Down, |e, _, cx| e.motion(Motion::Down, false, cx));
    let root = on!(root, SelectLeft, |e, _, cx| e.motion(
        Motion::Left,
        true,
        cx
    ));
    let root = on!(root, SelectRight, |e, _, cx| e.motion(
        Motion::Right,
        true,
        cx
    ));
    let root = on!(root, SelectUp, |e, _, cx| e.motion(Motion::Up, true, cx));
    let root = on!(root, SelectDown, |e, _, cx| e.motion(
        Motion::Down,
        true,
        cx
    ));
    let root = on!(root, WordLeft, |e, _, cx| e.motion(
        Motion::WordLeft,
        false,
        cx
    ));
    let root = on!(root, WordRight, |e, _, cx| e.motion(
        Motion::WordRight,
        false,
        cx
    ));
    let root = on!(root, SelectWordLeft, |e, _, cx| e.motion(
        Motion::WordLeft,
        true,
        cx
    ));
    let root = on!(root, SelectWordRight, |e, _, cx| e.motion(
        Motion::WordRight,
        true,
        cx
    ));
    let root = on!(root, LineStart, |e, _, cx| e.motion(
        Motion::LineStart,
        false,
        cx
    ));
    let root = on!(root, LineEnd, |e, _, cx| e.motion(
        Motion::LineEnd,
        false,
        cx
    ));
    let root = on!(root, SelectLineStart, |e, _, cx| e.motion(
        Motion::LineStart,
        true,
        cx
    ));
    let root = on!(root, SelectLineEnd, |e, _, cx| e.motion(
        Motion::LineEnd,
        true,
        cx
    ));
    let root = on!(root, DocumentStart, |e, _, cx| e.motion(
        Motion::Start,
        false,
        cx
    ));
    let root = on!(root, DocumentEnd, |e, _, cx| e.motion(
        Motion::End,
        false,
        cx
    ));
    let root = on!(root, SelectDocumentStart, |e, _, cx| e.motion(
        Motion::Start,
        true,
        cx
    ));
    let root = on!(root, SelectDocumentEnd, |e, _, cx| e.motion(
        Motion::End,
        true,
        cx
    ));
    let root = on!(root, PageUp, |e, _, cx| e.motion(
        Motion::Page(-PAGE),
        false,
        cx
    ));
    let root = on!(root, PageDown, |e, _, cx| e.motion(
        Motion::Page(PAGE),
        false,
        cx
    ));
    let root = on!(root, Backspace, |e, _, cx| {
        e.delete_by(|buffer, at| buffer.previous(at)..at, cx)
    });
    let root = on!(root, Delete, |e, _, cx| e
        .delete_by(|buffer, at| at..buffer.next(at), cx));
    let root = on!(root, DeleteWordLeft, |e, _, cx| {
        e.delete_by(|buffer, at| buffer.word_start(at)..at, cx)
    });
    let root = on!(root, DeleteLineLeft, |e, _, cx| e.delete_by(line_left, cx));
    let root = on!(root, Newline, |e, window, cx| {
        if e.focus.is_focused(window) {
            e.newline(cx)
        } else {
            cx.propagate()
        }
    });
    let root = on!(root, Indent, |e, _, cx| e.indent(cx));
    let root = on!(root, Outdent, |e, _, cx| e.outdent(cx));
    let root = on!(root, Undo, |e, _, cx| e.undo(cx));
    let root = on!(root, Redo, |e, _, cx| e.redo(cx));
    let root = on!(root, SelectAll, |e, _, cx| e.select_all(cx));
    let root = on!(root, Copy, |e, _, cx| e.copy(cx));
    let root = on!(root, Cut, |e, _, cx| e.cut(cx));
    let root = on!(root, Paste, |e, _, cx| {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            e.paste(&text, cx)
        }
    });
    let root = on!(root, AddCursorAbove, |e, _, cx| e.add_cursor(false, cx));
    let root = on!(root, AddCursorBelow, |e, _, cx| e.add_cursor(true, cx));
    let root = on!(root, SelectNext, |e, _, cx| e.select_next(cx));
    let root = on!(root, SingleCursor, |e, window, cx| {
        if e.marks.chat.is_some() {
            e.set_chat(None, window, cx)
        } else {
            e.single_cursor(cx)
        }
    });
    let root = on!(root, ToggleComment, |e, _, cx| e.toggle_comment(cx));
    let root = on!(root, Fold, |e, _, cx| {
        let line = e.buffer.line_of(e.primary().head);
        let header = super::syntax::folds(&e.buffer)
            .into_iter()
            .filter(|(header, end)| *header <= line && line <= *end)
            .map(|(header, _)| header)
            .next_back();
        if let Some(header) = header.filter(|header| !e.folded.contains(header)) {
            e.toggle_fold(header, cx);
        }
    });
    on!(root, Unfold, |e, _, cx| {
        let line = e.buffer.line_of(e.primary().head);
        if e.folded.contains(&line) {
            e.toggle_fold(line, cx);
        }
    })
}
