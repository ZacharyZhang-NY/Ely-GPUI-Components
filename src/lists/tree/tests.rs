use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, Pixels, Point, Render, SharedString,
    Styled, TestAppContext, VisualTestContext, Window, point, px,
};

use super::{DropAt, Tree, TreeNode};
use crate::theme::Theme;

/// A small project tree that keeps what its tree reports.
#[derive(Default)]
struct Explorer {
    docs: Vec<TreeNode>,
    selected: Vec<SharedString>,
    checked: Option<Vec<SharedString>>,
    heard: Vec<String>,
}

impl Render for Explorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let heard = |view: &Entity<Explorer>| {
            let view = view.clone();
            move |line: String, cx: &mut gpui::App| {
                view.update(cx, |explorer, _| explorer.heard.push(line))
            }
        };
        let docs = match self.docs.is_empty() {
            true => TreeNode::new("docs", "docs").pending(),
            false => TreeNode::new("docs", "docs").children(self.docs.clone()),
        };
        let all = [
            TreeNode::new("src", "src").children([
                TreeNode::new("lib", "lib.rs"),
                TreeNode::new("ui", "ui").child(TreeNode::new("button", "button.rs")),
            ]),
            docs,
            TreeNode::new("readme", "README.md")
                .note("a note long enough to take the whole row and more"),
        ];
        let (select, check, load, rename, moved) = (
            view.clone(),
            view,
            heard(&cx.entity()),
            heard(&cx.entity()),
            heard(&cx.entity()),
        );
        let tree = Tree::new("tree", all)
            .open(["src"])
            .selected(self.selected.clone())
            .on_select(move |keys, _, cx| {
                select.update(cx, |explorer, _| explorer.selected = keys.to_vec())
            })
            .on_load(move |key, _, cx| load(format!("load {key}"), cx))
            .on_rename(move |key, name, _, cx| rename(format!("rename {key} {name}"), cx))
            .on_move(move |key, target, at, _, cx| moved(format!("move {key} {at:?} {target}"), cx))
            .w(px(300.0))
            .h(px(400.0));
        match self.checked.clone() {
            Some(checked) => tree.checked(checked).on_check(move |keys, _, cx| {
                check.update(cx, |explorer, _| explorer.checked = Some(keys.to_vec()))
            }),
            None => tree,
        }
    }
}

fn explorer(checked: bool, cx: &mut TestAppContext) -> (Entity<Explorer>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        crate::forms::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(move |_, _| Explorer {
        checked: checked.then(Vec::new),
        ..Explorer::default()
    });
    settle(cx);
    (view, cx)
}

/// Frames 2ms apart, past reduced motion's 1ms glides.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// Row `ix`'s left padding: rows are 28 tall.
fn row(ix: usize) -> Point<Pixels> {
    point(px(4.0), px(14.0 + 28.0 * ix as f32))
}

fn read<T>(view: &Entity<Explorer>, cx: &mut VisualTestContext, f: impl Fn(&Explorer) -> T) -> T {
    view.read_with(cx, |explorer, _| f(explorer))
}

#[gpui::test]
fn a_long_note_leaves_the_label_half_the_row(cx: &mut TestAppContext) {
    let (_, cx) = explorer(false, cx);
    let note = cx.debug_bounds("tree-note").expect("the readme's note");
    assert!(
        note.size.width <= px(150.0),
        "the note takes at most half the row: {note:?}"
    );
    assert!(
        note.size.width > px(100.0),
        "the note keeps its half: {note:?}"
    );
}

#[gpui::test]
fn keys_walk_open_climb_and_close(cx: &mut TestAppContext) {
    let (view, cx) = explorer(false, cx);
    cx.simulate_click(row(0), Modifiers::none());
    settle(cx);
    let mut seen = Vec::new();
    for keys in ["right", "down", "right", "right", "left", "left"] {
        cx.simulate_keystrokes(keys);
        settle(cx);
        seen.push(read(&view, cx, |explorer| explorer.selected.join(",")));
    }
    assert_eq!(
        seen,
        ["lib", "ui", "ui", "button", "ui", "ui"],
        "right enters, left climbs, then closes"
    );
    cx.simulate_keystrokes("down");
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.selected.join(",")),
        "docs",
        "ui closed again"
    );
}

#[gpui::test]
fn a_pending_node_asks_for_its_children_once(cx: &mut TestAppContext) {
    let (view, cx) = explorer(false, cx);
    cx.simulate_click(row(3), Modifiers::none());
    settle(cx);
    cx.simulate_keystrokes("right");
    settle(cx);
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.heard.clone()),
        ["load docs"],
        "asked once while it waits"
    );
    view.update(cx, |explorer, cx| {
        explorer.docs = vec![TreeNode::new("guide", "guide.md")];
        cx.notify();
    });
    settle(cx);
    cx.simulate_keystrokes("left right down");
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.heard.clone()),
        ["load docs"]
    );
    assert_eq!(
        read(&view, cx, |explorer| explorer.selected.join(",")),
        "guide",
        "the children came"
    );
}

#[gpui::test]
fn space_checks_a_whole_branch_and_again_clears_it(cx: &mut TestAppContext) {
    let (view, cx) = explorer(true, cx);
    cx.simulate_click(row(0), Modifiers::none());
    settle(cx);
    cx.simulate_keystrokes("space");
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.checked.clone()),
        Some(vec!["lib".into(), "button".into()])
    );
    cx.simulate_keystrokes("space");
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.checked.clone()),
        Some(Vec::new())
    );
}

#[gpui::test]
fn f2_renames_a_node(cx: &mut TestAppContext) {
    let (view, cx) = explorer(false, cx);
    cx.simulate_click(row(1), Modifiers::none());
    settle(cx);
    cx.simulate_keystrokes("f2");
    settle(cx);
    cx.simulate_input("main.rs");
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.heard.clone()),
        ["rename lib main.rs"]
    );
}

#[gpui::test]
fn a_drop_in_a_folders_middle_moves_the_node_inside(cx: &mut TestAppContext) {
    let (view, cx) = explorer(false, cx);
    cx.simulate_mouse_down(row(4), MouseButton::Left, Modifiers::none());
    for y in [120.0, 100.0, 80.0, 70.0] {
        cx.simulate_mouse_move(point(px(40.0), px(y)), MouseButton::Left, Modifiers::none());
        settle(cx);
    }
    cx.simulate_mouse_up(
        point(px(40.0), px(70.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    settle(cx);
    assert_eq!(
        read(&view, cx, |explorer| explorer.heard.clone()),
        [format!("move readme {:?} ui", DropAt::Inside)]
    );
}

#[gpui::test]
fn guides_run_unbroken_under_the_parents_chevron(cx: &mut TestAppContext) {
    let (_, cx) = explorer(false, cx);
    let bounds = |name: &'static str, cx: &mut VisualTestContext| {
        cx.debug_bounds(name).unwrap_or_else(|| panic!("no {name}"))
    };
    let chevron = bounds("tree-chevron-src", cx);
    let (lib, ui) = (
        bounds("tree-guide-lib-0", cx),
        bounds("tree-guide-ui-0", cx),
    );
    assert_eq!(
        lib.center().x,
        chevron.center().x,
        "the guide falls from the chevron's middle"
    );
    assert_eq!(lib.bottom(), ui.top(), "one row's guide meets the next");
    assert_eq!(lib.size.height, px(28.0), "a guide takes its row's height");
}
