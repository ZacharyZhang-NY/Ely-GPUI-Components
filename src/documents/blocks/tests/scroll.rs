use gpui::{
    AppContext as _, Entity, Modifiers, ParentElement, Pixels, Render, ScrollDelta, ScrollEvent,
    ScrollHandle, ScrollSource, Styled, TestAppContext, TouchPhase, VisualTestContext, div, point,
    prelude::*, px,
};

use super::{BlockData, BlockEditor, BlockKind, caret, settle};
use crate::{forms, theme::Theme};

const TALL: Pixels = px(200.0);

/// The editor in a box of its own at the top of a page that scrolls.
struct Page(Entity<BlockEditor>, ScrollHandle);

impl Render for Page {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.1)
            .child(div().h(TALL).child(self.0.clone()))
            .child(div().h(px(2000.0)))
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<BlockEditor>, ScrollHandle, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        crate::documents::bind_keys(cx);
    });
    let blocks = (0..30).map(|ix| BlockData::new(BlockKind::Paragraph, [format!("line {ix}")]));
    let page = ScrollHandle::new();
    let (view, cx) = cx.add_window_view({
        let page = page.clone();
        |window, cx| {
            Page(
                cx.new(|cx| BlockEditor::new(blocks.collect(), window, cx)),
                page,
            )
        }
    });
    let editor = view.read_with(cx, |view, _| view.0.clone());
    cx.update(|window, _| window.activate_window());
    still(cx);
    (editor, page, cx)
}

/// Settles past reduced motion's 1 ms glides on the wall clock.
fn still(cx: &mut VisualTestContext) {
    settle(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(5));
    settle(cx);
}

/// The focused caret's box in the window.
fn shown(editor: &Entity<BlockEditor>, cx: &mut VisualTestContext) -> gpui::Bounds<Pixels> {
    cx.update(|window, cx| {
        let (key, ix) = editor
            .read(cx)
            .focused(window, cx)
            .expect("a field has focus");
        let at = editor.read(cx).index(key).expect("the block is there");
        editor.read(cx).blocks[at].fields[ix]
            .read(cx)
            .caret_bounds()
            .expect("laid out")
    })
}

fn inside(caret: gpui::Bounds<Pixels>) -> bool {
    caret.top() >= px(0.0) && caret.bottom() <= TALL
}

#[gpui::test]
fn the_caret_stays_in_view_as_blocks_are_added_at_the_end(cx: &mut TestAppContext) {
    let (editor, page, cx) = open(cx);
    caret(&editor, 29, 7, cx);
    still(cx);
    assert!(inside(shown(&editor, cx)), "{:?}", shown(&editor, cx));
    for _ in 0..4 {
        cx.simulate_keystrokes("enter");
        still(cx);
        assert!(inside(shown(&editor, cx)), "{:?}", shown(&editor, cx));
    }
    assert_eq!(page.offset().y, px(0.0), "the page stayed");
}

#[gpui::test]
fn a_wheel_scrolls_the_editor_and_leaves_its_caret(cx: &mut TestAppContext) {
    let (editor, page, cx) = open(cx);
    caret(&editor, 0, 0, cx);
    still(cx);
    let before = shown(&editor, cx);
    cx.simulate_event(ScrollEvent {
        position: point(px(100.0), px(100.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    still(cx);
    assert_eq!(
        shown(&editor, cx).top(),
        before.top() - px(120.0),
        "the editor scrolled"
    );
    assert_eq!(page.offset().y, px(0.0), "the page stayed");
}
