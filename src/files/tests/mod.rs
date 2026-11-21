use gpui::{
    Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement,
    Render, SharedString, Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};
use jiff::{Timestamp, ToSpan};

use super::{FileGrid, item::detail};
use crate::{lists::DirEntry, primitives::FocusNext, theme::Theme};

mod explorer;
mod find;
mod stale;
mod transfers;

pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

pub(super) fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

pub(super) fn then() -> Timestamp {
    Timestamp::from_second(1_700_000_000).expect("a time")
}

#[test]
fn a_row_says_a_files_size_and_a_folders_kind_with_when_each_changed() {
    let now = then() + 2.hours();
    assert_eq!(
        detail(&DirEntry::file("a.txt", 2_400, then()), now),
        "2.4 KB · 2 hours ago"
    );
    assert_eq!(
        detail(&DirEntry::folder("Docs", then()), now),
        "Folder · 2 hours ago"
    );
}

/// Seven files in a grid three tiles across, and what it picked and opened.
struct Tiled {
    selected: Option<SharedString>,
    picked: Vec<SharedString>,
    opened: Vec<SharedString>,
}

impl Render for Tiled {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (pick, open) = (cx.entity(), cx.entity());
        let names = ["a", "b", "c", "d", "e", "f", "g"];
        let grid = FileGrid::new("grid", names.map(|name| DirEntry::file(name, 10, then())))
            .on_select(move |name, _, cx| pick.update(cx, |view, _| view.picked.push(name.clone())))
            .on_open(move |entry, _, cx| {
                open.update(cx, |view, _| view.opened.push(entry.name().clone()))
            });
        let grid = match self.selected.clone() {
            Some(name) => grid.selected(name),
            None => grid,
        };
        div().w(px(320.0)).child(grid)
    }
}

fn tiled<'a>(
    selected: Option<&'static str>,
    cx: &'a mut TestAppContext,
) -> (Entity<Tiled>, &'a mut VisualTestContext) {
    setup(cx);
    let selected = selected.map(|name| SharedString::from(name.to_string()));
    let (view, cx) = cx.add_window_view(move |_, _| Tiled {
        selected,
        picked: Vec::new(),
        opened: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (view, cx)
}

fn heard(
    view: &Entity<Tiled>,
    cx: &mut VisualTestContext,
) -> (Vec<SharedString>, Vec<SharedString>) {
    view.read_with(cx, |view, _| (view.picked.clone(), view.opened.clone()))
}

#[gpui::test]
fn the_arrows_move_by_tile_and_by_row_and_enter_opens(cx: &mut TestAppContext) {
    let (view, cx) = tiled(None, cx);
    cx.update(|window, cx| window.focus_next(cx));
    for key in ["right", "right", "down", "down", "cmd-left"] {
        press(key, cx);
    }
    press("enter", cx);
    let (picked, opened) = heard(&view, cx);
    assert_eq!(picked, ["a", "b", "e", "g"], "a command key passes");
    assert_eq!(opened, ["g"]);
}

#[gpui::test]
fn an_owner_pick_gone_from_the_grid_replaces_the_local_one(cx: &mut TestAppContext) {
    let (view, cx) = tiled(None, cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("right", cx);
    view.update(cx, |view, cx| {
        view.selected = Some("Ghost.md".into());
        cx.notify();
    });
    settle(cx);
    press("enter", cx);
    assert_eq!(heard(&view, cx).1, Vec::<SharedString>::new());
}

#[gpui::test]
fn the_keys_start_from_the_tile_picked(cx: &mut TestAppContext) {
    let (view, cx) = tiled(Some("c"), cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("down", cx);
    assert_eq!(heard(&view, cx).0, ["f"]);
}

#[gpui::test]
fn a_press_picks_a_tile_and_a_double_press_opens_it(cx: &mut TestAppContext) {
    let (view, cx) = tiled(None, cx);
    let grid = cx.debug_bounds("file-grid").expect("the grid draws");
    let first = grid.origin + point(px(12.0), px(8.0));
    cx.simulate_click(first, Modifiers::none());
    settle(cx);
    assert_eq!(heard(&view, cx), (vec!["a".into()], vec![]));
    cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: first,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: first,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    settle(cx);
    assert_eq!(heard(&view, cx).1, ["a"]);
}

#[test]
#[should_panic(expected = "a twice")]
fn a_name_is_listed_once() {
    let _ = FileGrid::new(
        "grid",
        ["a", "a"].map(|name| DirEntry::file(name, 1, then())),
    );
}

/// One tile whose name runs long.
struct Named;

impl Render for Named {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let name = "A very long file name that keeps going past two whole lines.pdf";
        div()
            .w(px(280.0))
            .child(FileGrid::new("grid", [DirEntry::file(name, 10, then())]))
    }
}

#[gpui::test]
fn a_long_name_takes_one_line_inside_its_tile(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Named);
    settle(cx);
    let (name, grid) = (
        cx.debug_bounds("file-name 0").expect("the name draws"),
        cx.debug_bounds("file-grid").expect("the grid draws"),
    );
    assert!(name.size.height < px(24.0), "one line: {name:?}");
    assert!(
        name.left() >= grid.left() && name.right() <= grid.right(),
        "{name:?} in {grid:?}"
    );
}
