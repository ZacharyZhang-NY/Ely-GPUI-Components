mod stale;

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    ClipboardItem, Entity, EntityInputHandler, KeyUpEvent, Keystroke, TestAppContext,
    VisualTestContext,
};

use super::{CodeEditor, GhostText, InlayHint, LineNumbers, layout::Row};
use crate::theme::Theme;

pub(super) fn editor<'a>(
    text: &str,
    cx: &'a mut TestAppContext,
) -> (Entity<CodeEditor>, &'a mut VisualTestContext) {
    cx.update(Theme::init);
    let text = text.to_string();
    cx.add_window_view(move |window, cx| CodeEditor::new(text, window, cx))
}

#[gpui::test]
fn typed_pairs_close_and_one_undo_takes_back_the_burst(cx: &mut TestAppContext) {
    let (editor, cx) = editor("let x = ", cx);
    editor.update(cx, |editor, cx| {
        let end = 8..8;
        editor.select([end], cx);
        for ch in ["(", "1", ")"] {
            editor.type_text(ch, cx);
        }
        assert_eq!(
            editor.text(),
            "let x = (1)",
            "the typed close steps over its pair"
        );
        editor.undo(cx);
        assert_eq!(editor.text(), "let x = ");
        editor.redo(cx);
        assert_eq!(editor.text(), "let x = (1)");
    });
}

#[gpui::test]
fn select_next_takes_the_word_then_its_next_match(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a foo b foo c", cx);
    editor.update(cx, |editor, cx| {
        let inside = 3..3;
        editor.select([inside], cx);
        editor.select_next(cx);
        editor.select_next(cx);
        let ranges: Vec<_> = editor.selections().iter().map(|s| s.range()).collect();
        assert_eq!(ranges, [2..5, 8..11]);
        editor.type_text("bar", cx);
        assert_eq!(editor.text(), "a bar b bar c", "every cursor types");
    });
}

#[gpui::test]
fn toggling_comments_twice_restores_the_lines(cx: &mut TestAppContext) {
    let (editor, cx) = editor("fn a() {\n    b();\n}", cx);
    editor.update(cx, |editor, cx| {
        editor.select_all(cx);
        editor.toggle_comment(cx);
        assert_eq!(editor.text(), "// fn a() {\n//     b();\n// }");
        editor.toggle_comment(cx);
        assert_eq!(editor.text(), "fn a() {\n    b();\n}");
    });
}

#[gpui::test]
fn a_multi_line_ghost_moves_the_rest_of_its_line_below_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor("call(x)\nnext", cx);
    editor.update(cx, |editor, cx| {
        editor.set_ghost_text(
            Some(GhostText {
                offset: 5,
                text: "a,\n  b, ".into(),
            }),
            cx,
        );
        assert_eq!(
            editor.row_text(0),
            "call(",
            "the row stops where the ghost starts"
        );
        assert_eq!(editor.row_text(1), "next");
        let caret = 5..5;
        editor.select([caret], cx);
        editor.indent(cx);
        assert_eq!(
            editor.text(),
            "call(a,\n  b, x)\nnext",
            "Tab takes the ghost"
        );
        assert_eq!(editor.row_text(0), "call(a,", "a taken ghost cuts nothing");
    });
}

#[gpui::test]
fn a_selection_inside_a_fold_opens_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor("fn a() {\n    b();\n}\nc", cx);
    editor.update(cx, |editor, cx| {
        editor.toggle_fold(0, cx);
        assert!(!editor.frame_shows(1), "the body folds away");
        let body = 13..13;
        editor.select([body], cx);
        assert!(editor.frame_shows(1), "a cursor inside unfolds it");
    });
}

#[gpui::test]
fn hidden_numbers_draw(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a\nb", cx);
    editor.update(cx, |editor, cx| {
        editor.set_line_numbers(LineNumbers::Hidden, cx)
    });
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |editor, _| editor.gutter_columns()), 5);
}

#[gpui::test]
fn edits_with_nothing_to_change_leave_the_text(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a\n\nb", cx);
    editor.update(cx, |editor, cx| {
        editor.outdent(cx);
        let blank = 2..2;
        editor.select([blank], cx);
        editor.toggle_comment(cx);
        assert_eq!(editor.text(), "a\n\nb");
    });
}

#[gpui::test]
fn a_read_only_editor_ignores_typing_and_undo(cx: &mut TestAppContext) {
    let (editor, cx) = editor("x", cx);
    editor.update(cx, |editor, cx| {
        let end = 1..1;
        editor.select([end], cx);
        editor.type_text("y", cx);
        editor.set_read_only(true, cx);
        editor.type_text("(", cx);
        assert_eq!(editor.primary().head, 2, "the caret stays");
        editor.undo(cx);
        assert_eq!(editor.text(), "xy");
    });
}

#[gpui::test]
fn enter_presses_the_banner_button_inside_the_editor(cx: &mut TestAppContext) {
    let (editor, cx) = editor("x", cx);
    cx.update(|window, cx| {
        super::keys::bind_keys(cx);
        editor.update(cx, |editor, cx| editor.set_read_only(true, cx));
        window.refresh();
    });
    cx.run_until_parked();
    let focus = editor.read_with(cx, |editor, _| editor.focus.clone());
    cx.update(|window, cx| {
        window.focus(&focus, cx);
        window.focus_next(cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").expect("a key"),
    });
    cx.run_until_parked();
    assert!(!editor.read_with(cx, |editor, _| editor.is_read_only()));
}

#[gpui::test]
fn a_hint_past_a_multi_line_ghost_stays_in_its_row(cx: &mut TestAppContext) {
    let (editor, cx) = editor("call(x)\nnext", cx);
    editor.update(cx, |editor, cx| {
        editor.set_inlay_hints(
            vec![InlayHint {
                offset: 6,
                text: ": i32".into(),
            }],
            cx,
        );
        editor.set_ghost_text(
            Some(GhostText {
                offset: 5,
                text: "a,\n".into(),
            }),
            cx,
        );
        let row = editor.row_text(0).len();
        assert!(editor.notes(0, cx).iter().all(|(at, ..)| *at <= row));
    });
    cx.run_until_parked();
}

#[gpui::test]
fn cursors_added_above_keep_climbing(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a\nb\nc\nd", cx);
    editor.update(cx, |editor, cx| {
        let last = 6..6;
        editor.select([last], cx);
        editor.add_cursor(false, cx);
        editor.add_cursor(false, cx);
        assert_eq!(editor.selections().len(), 3);
        assert_eq!(editor.position().0, 2, "the newest cursor leads");
    });
}

#[gpui::test]
fn a_slow_composition_undoes_in_one_step(cx: &mut TestAppContext) {
    let (editor, cx) = editor("", cx);
    for pinyin in ["n", "ni"] {
        editor.update_in(cx, |editor, window, cx| {
            editor.replace_and_mark_text_in_range(None, pinyin, None, window, cx)
        });
        cx.executor().advance_clock(Duration::from_millis(950));
    }
    editor.update_in(cx, |editor, window, cx| {
        editor.replace_text_in_range(None, "你", window, cx);
        assert_eq!(editor.text(), "你");
        editor.undo(cx);
        assert_eq!(editor.text(), "", "the pinyin was never a step");
    });
}

#[gpui::test]
fn a_composition_cleared_to_nothing_ends(cx: &mut TestAppContext) {
    let (editor, cx) = editor("", cx);
    editor.update_in(cx, |editor, window, cx| {
        editor.replace_and_mark_text_in_range(None, "n", None, window, cx);
        editor.replace_and_mark_text_in_range(None, "", None, window, cx);
        editor.type_text("x", cx);
        editor.undo(cx);
        assert_eq!(editor.text(), "", "typing after the cleared mark undoes");
    });
}

impl CodeEditor {
    /// Whether line `line` has a row now.
    fn frame_shows(&self, line: usize) -> bool {
        let hide = super::layout::hidden(
            self.buffer.lines(),
            &super::syntax::folds(&self.buffer),
            &self.folded,
        );
        super::layout::rows(&self.buffer, &hide, &self.marks).contains(&Row::Line(line))
    }
}

#[gpui::test]
fn a_browser_paste_gives_each_cursor_its_line(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a\nb", cx);
    cx.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            let (first, second) = (1..1, 3..3);
            editor.select([first, second], cx);
            EntityInputHandler::paste(editor, ClipboardItem::new_string("1\n2".into()), window, cx);
            assert_eq!(editor.text(), "a1\nb2");
        })
    });
}

#[gpui::test]
fn only_a_focused_editor_blinks(cx: &mut TestAppContext) {
    let (editor, cx) = editor("a\nb", cx);
    let redraws = Rc::new(Cell::new(0));
    let counted = redraws.clone();
    let _count = cx.update(|_, cx| cx.observe(&editor, move |_, _| counted.set(counted.get() + 1)));
    editor.update(cx, |editor, cx| {
        let second = 2..2;
        editor.select([second], cx);
    });
    cx.run_until_parked();
    redraws.set(0);
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    assert_eq!(redraws.get(), 0, "an unfocused editor keeps still");
    cx.update(|window, cx| {
        window.activate_window();
        let focus = editor.read(cx).focus.clone();
        focus.focus(window, cx);
    });
    cx.run_until_parked();
    redraws.set(0);
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    assert!(redraws.get() > 2, "a focused one blinks");
}

#[gpui::test]
fn escape_in_the_find_query_closes_the_box(cx: &mut TestAppContext) {
    use gpui::{
        AppContext as _, Focusable as _, IntoElement, ParentElement as _, Render, Styled as _,
        Window, div,
    };

    use crate::forms::TextInput;

    struct Host {
        query: Entity<TextInput>,
        closed: Rc<Cell<bool>>,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let closed = self.closed.clone();
            let find = super::FindWidget::new("find", &self.query, None, 0)
                .on_close(move |_, _| closed.set(true));
            div().size_full().child(find)
        }
    }
    cx.update(Theme::init);
    let closed = Rc::new(Cell::new(false));
    let shut = closed.clone();
    let (host, cx) = cx.add_window_view(move |window, cx| Host {
        query: cx.new(|cx| TextInput::new(window, cx)),
        closed: shut,
    });
    let field = host.read_with(cx, |host, cx| host.query.focus_handle(cx));
    cx.update(|window, cx| window.focus(&field, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert!(closed.get(), "escape closes the find box");
}
