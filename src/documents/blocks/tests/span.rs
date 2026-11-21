use gpui::{
    Entity, EntityInputHandler, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

use super::{BlockData, BlockEditor, BlockKind, blocks, caret, open, settle, text};
use crate::theme::Theme;

/// Three paragraphs, settled where they stand: reduced motion ends the rows' first glide.
fn three(cx: &mut TestAppContext) -> (Entity<BlockEditor>, &mut VisualTestContext) {
    let paragraph = |text: &str| BlockData::new(BlockKind::Paragraph, [text]);
    let blocks = vec![paragraph("alpha"), paragraph("beta"), paragraph("gamma")];
    let (editor, cx) = open(blocks, cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.reduced_motion = true));
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(5));
    settle(cx);
    (editor, cx)
}

/// Where byte `at` of block `ix`'s first field sits in the window.
fn at(
    editor: &Entity<BlockEditor>,
    ix: usize,
    at: usize,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    cx.update(|_, cx| {
        let field = editor.read(cx).blocks[ix].fields[0].read(cx);
        let caret = field.bounds_for(at).expect("laid out");
        caret.origin + point(px(0.5), caret.size.height / 2.0)
    })
}

/// Drags from `alpha` after two letters to `gamma` after three.
fn drag(editor: &Entity<BlockEditor>, cx: &mut VisualTestContext) {
    let (from, to) = (at(editor, 0, 2, cx), at(editor, 2, 3, cx));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    settle(cx);
}

fn spanned(editor: &Entity<BlockEditor>, cx: &mut VisualTestContext) -> Option<(usize, usize)> {
    cx.update(|_, cx| {
        editor
            .read(cx)
            .span()
            .map(|(start, end)| (start.at, end.at))
    })
}

#[gpui::test]
fn a_drag_selects_across_blocks_and_typing_takes_its_place(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    assert_eq!(spanned(&editor, cx), Some((2, 3)));
    assert_eq!(cx.update(|_, cx| editor.read(cx).washed()), Some(1..2));
    let selected = |ix: usize, cx: &mut VisualTestContext| {
        cx.update(|_, cx| editor.read(cx).blocks[ix].fields[0].read(cx).selection())
    };
    assert_eq!((selected(0, cx), selected(2, cx)), (2..5, 0..3));
    cx.simulate_input("X");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "alXma")]
    );
    cx.simulate_keystrokes("secondary-z");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![
            text(BlockKind::Paragraph, "alpha"),
            text(BlockKind::Paragraph, "beta"),
            text(BlockKind::Paragraph, "gamma"),
        ]
    );
}

#[gpui::test]
fn shift_press_selects_from_the_caret_and_backspace_joins(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    caret(&editor, 0, 1, cx);
    let to = at(&editor, 2, 3, cx);
    cx.simulate_mouse_down(to, MouseButton::Left, Modifiers::shift());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::shift());
    settle(cx);
    assert_eq!(spanned(&editor, cx), Some((1, 3)));
    cx.simulate_keystrokes("backspace");
    settle(cx);
    assert_eq!(blocks(&editor, cx), vec![text(BlockKind::Paragraph, "ama")]);
}

#[gpui::test]
fn copy_takes_the_selection_as_markdown(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    cx.simulate_keystrokes("secondary-c");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("pha\n\nbeta\n\ngam"));
    cx.simulate_keystrokes("secondary-x");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "alma")]
    );
}

#[gpui::test]
fn a_key_that_moves_the_caret_lets_the_selection_go(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    cx.simulate_keystrokes("left");
    settle(cx);
    assert_eq!(spanned(&editor, cx), None);
    let gamma = cx.update(|_, cx| editor.read(cx).blocks[2].fields[0].read(cx).selection());
    assert!(gamma.is_empty(), "{gamma:?}");
    cx.simulate_input("Y");
    settle(cx);
    assert_eq!(blocks(&editor, cx).len(), 3);
}

#[gpui::test]
fn enter_replaces_the_selection_with_a_break(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![
            text(BlockKind::Paragraph, "al"),
            text(BlockKind::Paragraph, "ma"),
        ]
    );
}

fn original() -> Vec<(BlockKind, Vec<String>)> {
    ["alpha", "beta", "gamma"]
        .map(|words| text(BlockKind::Paragraph, words))
        .to_vec()
}

#[gpui::test]
fn enter_over_a_selection_undoes_in_one_step(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    cx.simulate_keystrokes("secondary-z");
    settle(cx);
    assert_eq!(blocks(&editor, cx), original());
}

#[gpui::test]
fn pasting_what_was_selected_still_takes_the_blocks_between(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("pha".into()));
    cx.simulate_keystrokes("secondary-v");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "alphama")]
    );
}

#[gpui::test]
fn backspace_from_the_end_of_a_block_takes_the_selection(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    let (from, to) = (at(&editor, 0, 5, cx), at(&editor, 2, 3, cx));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    settle(cx);
    assert_eq!(spanned(&editor, cx), Some((5, 3)));
    cx.simulate_keystrokes("backspace");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "alphama")]
    );
}

#[gpui::test]
fn a_composition_takes_the_selection_once_it_commits(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    drag(&editor, cx);
    let field = cx.update(|_, cx| editor.read(cx).blocks[0].fields[0].clone());
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "n", None, window, cx);
            input.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        })
    });
    settle(cx);
    assert_eq!(blocks(&editor, cx).len(), 3, "composing waits");
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_text_in_range(None, "你", window, cx)
        })
    });
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "al你ma")]
    );
}

#[gpui::test]
fn typing_after_shift_press_or_an_upward_drag_lands_once(cx: &mut TestAppContext) {
    let (editor, cx) = three(cx);
    caret(&editor, 0, 1, cx);
    let to = at(&editor, 2, 3, cx);
    cx.simulate_mouse_down(to, MouseButton::Left, Modifiers::shift());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::shift());
    settle(cx);
    cx.simulate_input("Z");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "aZma")]
    );
    cx.simulate_keystrokes("secondary-z");
    settle(cx);
    let (from, to) = (at(&editor, 2, 3, cx), at(&editor, 0, 2, cx));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    settle(cx);
    assert_eq!(spanned(&editor, cx), Some((2, 3)));
    cx.simulate_input("Q");
    settle(cx);
    assert_eq!(
        blocks(&editor, cx),
        vec![text(BlockKind::Paragraph, "alQma")]
    );
}
