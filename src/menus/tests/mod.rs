use gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyUpEvent, Keystroke, Modifiers,
    MouseButton, ParentElement, Render, SharedString, Styled, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};

mod more;

use super::{ContextMenu, DropdownMenu, Menu, MenuItem, SearchableMenu};
use crate::{primitives::FocusScope, theme::Theme};

/// What the rows did, and the state the check and radio rows show.
struct Desk {
    root: FocusHandle,
    ran: Vec<SharedString>,
    wrap: bool,
    theme: SharedString,
    view_rows: bool,
    enters_above: usize,
    shift: bool,
    asked: Option<(u64, gpui::Point<gpui::Pixels>)>,
}

impl Desk {
    fn menu(&self, cx: &mut Context<Self>) -> Menu {
        let run = |name: &'static str| {
            let view = cx.entity();
            move |_: &mut Window, cx: &mut App| {
                view.update(cx, |desk, cx| {
                    desk.ran.push(name.into());
                    cx.notify();
                })
            }
        };
        let (wrap, light, dark) = (cx.entity(), cx.entity(), cx.entity());
        let theme = Menu::new()
            .item(
                MenuItem::radio("Light", self.theme == "light")
                    .on_click(move |_, cx| light.update(cx, |desk, _| desk.theme = "light".into())),
            )
            .item(
                MenuItem::radio("Dark", self.theme == "dark")
                    .on_click(move |_, cx| dark.update(cx, |desk, _| desk.theme = "dark".into())),
            );
        let edit = Menu::new()
            .item(
                MenuItem::new("Cut")
                    .keys("secondary-x")
                    .on_click(run("cut")),
            )
            .item(MenuItem::new("Copy").disabled(true).on_click(run("copy")))
            .item(MenuItem::new("Paste").on_click(run("paste")));
        if !self.view_rows {
            return edit;
        }
        edit.group(
            "View",
            [
                MenuItem::check("Word wrap", self.wrap)
                    .on_click(move |_, cx| wrap.update(cx, |desk, _| desk.wrap = !desk.wrap)),
                MenuItem::submenu("Theme", theme),
            ],
        )
    }
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.menu(cx);
        let above = cx.entity();
        div()
            .size_full()
            .on_key_down(move |event, _, cx| {
                if event.keystroke.key == "enter" {
                    above.update(cx, |desk, _| desk.enters_above += 1);
                }
            })
            .child(
                FocusScope::new(&self.root)
                    .size_full()
                    .when(self.shift, |scope| scope.child(div().h(px(80.0))))
                    .child(DropdownMenu::new("edit", "Edit", menu.clone()))
                    .child(
                        ContextMenu::new("area", menu.clone())
                            .child(div().w(px(200.0)).h(px(120.0))),
                    )
                    .child(SearchableMenu::new("find", "Find", menu.clone()))
                    .child(
                        ContextMenu::new("late", menu).manual(self.asked).child(
                            div()
                                .debug_selector(|| "late".into())
                                .w(px(200.0))
                                .h(px(40.0)),
                        ),
                    ),
            )
    }
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Presses and releases `key`, with a frame in between as on a real keyboard.
pub(super) fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn desk(cx: &mut TestAppContext) -> (gpui::Entity<Desk>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Desk {
        root: cx.focus_handle(),
        ran: Vec::new(),
        wrap: false,
        theme: "light".into(),
        view_rows: true,
        enters_above: 0,
        shift: false,
        asked: None,
    });
    settle(cx);
    (view, cx)
}

fn ran(view: &gpui::Entity<Desk>, cx: &mut VisualTestContext) -> Vec<SharedString> {
    view.read_with(cx, |desk, _| desk.ran.clone())
}

fn click(x: f32, y: f32, cx: &mut VisualTestContext) {
    let at = point(px(x), px(y));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
}

#[gpui::test]
fn the_keyboard_opens_walks_past_disabled_rows_and_runs(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["paste"]);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["paste"], "the release left the menu shut");
}

#[gpui::test]
fn submenus_open_right_and_close_left(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("up", cx);
    press("right", cx);
    press("down", cx);
    press("left", cx);
    press("right", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |desk, _| desk.theme.clone()), "dark");
}

#[gpui::test]
fn a_check_row_reports_to_its_owner(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("up", cx);
    press("up", cx);
    press("enter", cx);
    assert!(view.read_with(cx, |desk, _| desk.wrap));
}

#[gpui::test]
fn escape_backs_out_one_level_then_closes(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("up", cx);
    press("right", cx);
    press("escape", cx);
    press("up", cx);
    press("enter", cx);
    assert!(
        view.read_with(cx, |desk, _| desk.wrap),
        "one escape left the root open"
    );
    press("enter", cx);
    press("escape", cx);
    press("down", cx);
    press("enter", cx);
    assert!(
        ran(&view, cx).is_empty(),
        "the second escape closed the menu"
    );
}

#[gpui::test]
fn a_right_click_opens_the_context_menu_where_it_lands(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let at = point(px(60.0), px(80.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
}

#[gpui::test]
fn a_manual_context_menu_opens_only_from_its_owner(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let at = cx.debug_bounds("late").expect("the host").center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    press("down", cx);
    press("enter", cx);
    assert!(ran(&view, cx).is_empty(), "a right click opens nothing");
    view.update(cx, |desk, cx| {
        desk.asked = Some((1, point(px(60.0), px(80.0))));
        cx.notify();
    });
    settle(cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
}

#[gpui::test]
fn the_trigger_toggles_and_a_press_outside_closes(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    click(12.0, 12.0, cx);
    click(12.0, 12.0, cx);
    press("down", cx);
    press("enter", cx);
    assert!(
        ran(&view, cx).is_empty(),
        "the second press on the trigger closed it"
    );
    click(12.0, 12.0, cx);
    click(600.0, 400.0, cx);
    press("down", cx);
    press("enter", cx);
    assert!(ran(&view, cx).is_empty(), "the press outside closed it");
}

#[gpui::test]
fn rows_the_owner_removes_leave_the_menu_usable(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("up", cx);
    view.update(cx, |desk, cx| {
        desk.view_rows = false;
        cx.notify();
    });
    settle(cx);
    press("enter", cx);
    assert!(ran(&view, cx).is_empty(), "the removed row ran");
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
}

#[gpui::test]
fn closing_from_the_trigger_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    click(12.0, 12.0, cx);
    press("enter", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"], "focus came back to the trigger");
}

#[gpui::test]
fn confirm_keys_stay_inside_the_menu(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    click(12.0, 12.0, cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
    assert_eq!(view.read_with(cx, |desk, _| desk.enters_above), 0);
}

#[gpui::test]
fn modified_confirm_keys_do_not_pick(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("ctrl-enter", cx);
    press("shift-space", cx);
    assert!(ran(&view, cx).is_empty(), "a modified key picked");
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
}

#[gpui::test]
fn a_second_right_click_moves_the_context_menu(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|window, cx| window.focus_next(cx));
    for at in [point(px(150.0), px(120.0)), point(px(20.0), px(40.0))] {
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        settle(cx);
    }
    press("down", cx);
    press("enter", cx);
    assert_eq!(
        ran(&view, cx),
        ["cut"],
        "the menu stayed open at the second click"
    );
}

#[gpui::test]
fn a_left_press_in_the_region_closes_the_context_menu(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let at = point(px(150.0), px(120.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    click(20.0, 40.0, cx);
    press("down", cx);
    press("enter", cx);
    assert!(ran(&view, cx).is_empty(), "the left press closed the menu");
}

fn open_find(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.focus_next(cx);
        window.focus_next(cx);
    });
    press("enter", cx);
}

#[gpui::test]
fn the_filter_runs_the_best_fit(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    open_find(cx);
    cx.simulate_input("pa");
    settle(cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["paste"]);
}

#[gpui::test]
fn space_types_into_the_filter(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    open_find(cx);
    cx.simulate_input("pa");
    settle(cx);
    press("space", cx);
    assert!(ran(&view, cx).is_empty(), "space picked a row");
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["paste"]);
}

#[gpui::test]
fn arrows_walk_rows_from_the_filter(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    open_find(cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["paste"]);
}

#[gpui::test]
fn an_open_dropdown_follows_its_host(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    click(12.0, 12.0, cx);
    view.update(cx, |desk, cx| {
        desk.shift = true;
        cx.notify();
    });
    settle(cx);
    settle(cx);
    cx.simulate_mouse_move(point(px(40.0), px(131.0)), None, Modifiers::none());
    settle(cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"], "the panel moved with its button");
}

#[gpui::test]
fn clearing_the_filter_marks_the_first_row_again(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    open_find(cx);
    cx.simulate_input("t");
    settle(cx);
    press("down", cx);
    press("backspace", cx);
    press("enter", cx);
    assert_eq!(ran(&view, cx), ["cut"]);
}
