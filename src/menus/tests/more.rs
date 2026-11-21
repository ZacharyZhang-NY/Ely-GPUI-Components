use super::{press, settle};
use crate::{
    menus::{MenuBar, PieItem, PieMenu},
    primitives::{FocusScope, IconName},
    theme::Theme,
};
use gpui::{
    Action, App, Context, FocusHandle, InteractiveElement, IntoElement, KeyBinding,
    KeyBindingContextPredicate, Modifiers, MouseButton, OwnedMenu, OwnedMenuItem, ParentElement,
    Render, Styled, TestAppContext, VisualTestContext, Window, actions, div, point, px,
};
use std::rc::Rc;

actions!(menu_bar_test, [OpenFile, Save, Undo, Deep]);

fn binding<A: Action>(cx: &App, strokes: &str, action: A, context: Option<&str>) -> KeyBinding {
    let predicate =
        context.map(|c| Rc::new(KeyBindingContextPredicate::parse(c).expect("context")));
    KeyBinding::load(strokes, Box::new(action), predicate, false, None, cx)
        .unwrap_or_else(|error| panic!("bad keystroke {strokes:?}: {error}"))
}

/// A bar over two menus, the second with a submenu, recording what they dispatch.
struct Bar {
    root: FocusHandle,
    ran: Vec<&'static str>,
}

fn action(name: &str, action: impl gpui::Action) -> OwnedMenuItem {
    OwnedMenuItem::Action {
        name: name.into(),
        action: Box::new(action),
        os_action: None,
        checked: false,
        disabled: false,
    }
}

impl Render for Bar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menus = vec![
            OwnedMenu {
                name: "File".into(),
                items: vec![action("Open", OpenFile), action("Save", Save)],
                disabled: false,
            },
            OwnedMenu {
                name: "Edit".into(),
                items: vec![
                    action("Undo", Undo),
                    OwnedMenuItem::Separator,
                    OwnedMenuItem::Submenu(OwnedMenu {
                        name: "More".into(),
                        items: vec![action("Deep", Deep)],
                        disabled: false,
                    }),
                ],
                disabled: false,
            },
        ];
        div()
            .size_full()
            .on_action(cx.listener(|bar, _: &OpenFile, _, _| bar.ran.push("open")))
            .on_action(cx.listener(|bar, _: &Save, _, _| bar.ran.push("save")))
            .on_action(cx.listener(|bar, _: &Undo, _, _| bar.ran.push("undo")))
            .on_action(cx.listener(|bar, _: &Deep, _, _| bar.ran.push("deep")))
            .child(
                FocusScope::new(&self.root)
                    .size_full()
                    .child(MenuBar::new("bar", menus)),
            )
    }
}

fn bar(cx: &mut TestAppContext) -> (gpui::Entity<Bar>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        cx.bind_keys([binding(cx, "secondary-s", Save, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Bar {
        root: cx.focus_handle(),
        ran: Vec::new(),
    });
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    (view, cx)
}

#[gpui::test]
fn the_bar_walks_menus_and_dispatches_their_actions(cx: &mut TestAppContext) {
    let (view, cx) = bar(cx);
    press("down", cx);
    press("enter", cx);
    press("down", cx);
    press("right", cx);
    press("up", cx);
    press("right", cx);
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |bar, _| bar.ran.clone()),
        ["open", "deep"]
    );
}

#[gpui::test]
fn arrows_move_between_closed_titles(cx: &mut TestAppContext) {
    let (view, cx) = bar(cx);
    press("right", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |bar, _| bar.ran.clone()), ["undo"]);
}

/// Four slices around a big area: Copy on top, then Cut, Paste and Share clockwise.
struct Wheel {
    ran: Vec<&'static str>,
    count: usize,
    root: FocusHandle,
    before: FocusHandle,
    enters_above: usize,
}

impl Render for Wheel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let slice = |icon, name: &'static str| {
            let view = cx.entity();
            PieItem::new(icon, name)
                .on_click(move |_, cx| view.update(cx, |wheel, _| wheel.ran.push(name)))
        };
        let mut items = vec![
            slice(IconName::Copy, "copy"),
            slice(IconName::Scissors, "cut"),
            slice(IconName::Clipboard, "paste"),
            slice(IconName::Share2, "share"),
        ];
        items.truncate(self.count);
        let above = cx.entity();
        div()
            .size_full()
            .on_key_down(move |event, _, cx| {
                let stroke = &event.keystroke;
                if stroke.key == "enter" && !stroke.modifiers.modified() {
                    above.update(cx, |wheel, _| wheel.enters_above += 1);
                }
            })
            .child(
                FocusScope::new(&self.root)
                    .size_full()
                    .child(div().id("before").track_focus(&self.before))
                    .child(PieMenu::new("pie", items).child(div().w(px(600.0)).h(px(600.0)))),
            )
    }
}

fn wheel(cx: &mut TestAppContext) -> (gpui::Entity<Wheel>, &mut VisualTestContext) {
    cx.update(Theme::init);
    let (view, cx) = cx.add_window_view(|_, cx| Wheel {
        ran: Vec::new(),
        count: 4,
        root: cx.focus_handle(),
        before: cx.focus_handle(),
        enters_above: 0,
    });
    settle(cx);
    cx.update(|window, cx| window.focus(&view.read(cx).before.clone(), cx));
    let at = point(px(300.0), px(300.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    (view, cx)
}

#[gpui::test]
fn the_pointer_direction_picks_a_slice(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    let right = point(px(400.0), px(300.0));
    cx.simulate_mouse_move(right, None, Modifiers::none());
    cx.simulate_click(right, Modifiers::none());
    settle(cx);
    assert_eq!(view.read_with(cx, |wheel, _| wheel.ran.clone()), ["cut"]);
}

#[gpui::test]
fn arrows_turn_the_pie_and_enter_runs(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    press("right", cx);
    press("right", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |wheel, _| wheel.ran.clone()), ["cut"]);
}

#[gpui::test]
fn a_press_on_the_hub_cancels(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    let hub = point(px(300.0), px(300.0));
    cx.simulate_mouse_move(hub, None, Modifiers::none());
    cx.simulate_click(hub, Modifiers::none());
    settle(cx);
    press("right", cx);
    press("enter", cx);
    assert!(view.read_with(cx, |wheel, _| wheel.ran.is_empty()));
}

#[gpui::test]
fn right_moves_a_menu_opened_by_the_mouse(cx: &mut TestAppContext) {
    let (view, cx) = bar(cx);
    let title = point(px(10.0), px(10.0));
    cx.simulate_mouse_move(title, None, Modifiers::none());
    cx.simulate_click(title, Modifiers::none());
    settle(cx);
    press("right", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |bar, _| bar.ran.clone()), ["undo"]);
}

#[gpui::test]
fn a_pie_that_shrinks_forgets_a_mark_past_its_end(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    for _ in 0..4 {
        press("right", cx);
    }
    view.update(cx, |wheel, cx| {
        wheel.count = 2;
        cx.notify();
    });
    settle(cx);
    press("enter", cx);
    assert!(view.read_with(cx, |wheel, _| wheel.ran.is_empty()));
}

#[gpui::test]
fn cancelling_on_the_hub_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    let hub = point(px(300.0), px(300.0));
    cx.simulate_mouse_move(hub, None, Modifiers::none());
    cx.simulate_click(hub, Modifiers::none());
    settle(cx);
    let before = view.read_with(cx, |wheel, _| wheel.before.clone());
    assert!(cx.update(|window, _| before.is_focused(window)));
}

#[gpui::test]
fn pie_confirm_keys_stay_inside_and_ignore_modifiers(cx: &mut TestAppContext) {
    let (view, cx) = wheel(cx);
    press("right", cx);
    press("ctrl-enter", cx);
    assert!(
        view.read_with(cx, |wheel, _| wheel.ran.is_empty()),
        "a modified key ran"
    );
    press("enter", cx);
    assert_eq!(view.read_with(cx, |wheel, _| wheel.ran.clone()), ["copy"]);
    assert_eq!(view.read_with(cx, |wheel, _| wheel.enters_above), 0);
}

/// A sized context menu over a child that fills it.
struct Filled {
    seen: std::rc::Rc<std::cell::Cell<f32>>,
}

impl Render for Filled {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        let fill = gpui::canvas(
            move |bounds, _, _| seen.set(f32::from(bounds.size.height)),
            |_, _, _, _| {},
        )
        .size_full();
        div().h(px(200.)).w(px(100.)).child(
            crate::menus::ContextMenu::new("filled", crate::menus::Menu::new())
                .size_full()
                .child(fill),
        )
    }
}

#[gpui::test]
fn a_sized_context_menu_lets_its_child_fill_it(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0.0));
    let (_, cx) = cx.add_window_view({
        let seen = seen.clone();
        move |_, _| Filled { seen }
    });
    settle(cx);
    assert_eq!(seen.get(), 200.0);
}
