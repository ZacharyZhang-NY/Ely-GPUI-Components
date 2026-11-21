use std::rc::Rc;

use gpui::{
    Action, App, Bounds, ElementId, Entity, InteractiveElement, IntoElement, MouseButton,
    OperatingSystem, OwnedMenu, OwnedMenuItem, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::*,
};

use super::{
    menu::{Open, Spot, hang, measure_host},
    model::{Menu, MenuItem},
};
use crate::{
    primitives::tab_stop,
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
};

/// The title under the keyboard and where each title sits.
#[derive(Default)]
struct Titles {
    at: usize,
    boxes: Vec<Bounds<Pixels>>,
}

/// gpui's menu model as rows: actions run through dispatch and show the keys bound to them.
fn rows(items: &[OwnedMenuItem], window: &Window, os: OperatingSystem) -> Menu {
    items.iter().fold(Menu::new(), |menu, item| match item {
        OwnedMenuItem::Separator => menu.separator(),
        OwnedMenuItem::Submenu(sub) => menu.item(MenuItem::submenu(
            sub.name.clone(),
            rows(&sub.items, window, os),
        )),
        OwnedMenuItem::SystemMenu(os) => menu.item(MenuItem::new(os.name.clone()).disabled(true)),
        OwnedMenuItem::Action { name, action, .. } => {
            let keys = window
                .highest_precedence_binding_for_action(action.as_ref())
                .map(|binding| {
                    let strokes: Vec<String> = binding
                        .keystrokes()
                        .iter()
                        .map(|stroke| stroke.unparse(os))
                        .collect();
                    strokes.join(" ")
                });
            let action: Rc<Box<dyn Action>> = Rc::new(action.boxed_clone());
            let row = MenuItem::new(name.clone())
                .on_click(move |window, cx| window.dispatch_action(action.boxed_clone(), cx));
            menu.item(match keys {
                Some(keys) => row.keys(keys),
                None => row,
            })
        }
    })
}

/// Opens title `at` under its box, its first row marked when `marked`.
fn open_title(
    state: &Entity<Open>,
    titles: &Entity<Titles>,
    menus: &[Menu],
    at: usize,
    marked: bool,
    cx: &mut App,
) {
    let anchor = titles.read(cx).boxes[at];
    titles.update(cx, |titles, cx| {
        titles.at = at;
        cx.notify();
    });
    Open::set_anchor(state, anchor, cx);
    Open::show(state, &menus[at], Spot::Under, marked, cx);
}

/// The app's menus drawn in the window, for platforms and layouts without a native bar.
#[derive(IntoElement)]
pub struct MenuBar {
    id: ElementId,
    menus: Vec<OwnedMenu>,
}

impl MenuBar {
    /// Pass `cx.get_menus()`: the menus `cx.set_menus` set.
    pub fn new(id: impl Into<ElementId>, menus: Vec<OwnedMenu>) -> Self {
        assert!(!menus.is_empty(), "a menu bar needs menus");
        Self {
            id: id.into(),
            menus,
        }
    }
}

impl RenderOnce for MenuBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.menus.len();
        let focus = tab_stop((self.id.clone(), "focus").into(), true, window, cx);
        let bar_focused = focus.is_focused(window);
        let state = window.use_keyed_state((self.id.clone(), "menu"), cx, |_, _| Open::default());
        let titles =
            window.use_keyed_state((self.id.clone(), "titles"), cx, |_, _| Titles::default());
        if titles.read(cx).boxes.len() != count {
            titles.update(cx, |titles, _| {
                titles.boxes = vec![Bounds::default(); count]
            });
        }
        let os = cx.platform_info().os;
        let menus: Rc<Vec<Menu>> = Rc::new(
            self.menus
                .iter()
                .map(|menu| rows(&menu.items, window, os))
                .collect(),
        );
        let open = state.read(cx).is_open();
        let at = titles.read(cx).at.min(count - 1);
        let theme = cx.theme();
        let colors = &theme.colors;
        let heads = self.menus.iter().enumerate().map(|(ix, menu)| {
            let (press, hover, measure, follow) =
                (state.clone(), state.clone(), titles.clone(), state.clone());
            let (press_titles, hover_titles) = (titles.clone(), titles.clone());
            let (press_menus, hover_menus) = (menus.clone(), menus.clone());
            let here = ix == at;
            div()
                .id(("title", ix))
                .relative()
                .flex()
                .items_center()
                .h(theme.control_height(ControlSize::Sm))
                .px_2()
                .rounded(theme.radius(Radius::Md))
                .border_1()
                .border_color(if bar_focused && here && !open {
                    colors.focus
                } else {
                    gpui::transparent_black()
                })
                .text_size(theme.text_size(TextSize::Sm))
                .when(open && here, |head| head.bg(colors.hover))
                .hover(|style| style.bg(colors.hover))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.prevent_default();
                    if open && here {
                        Open::close(&press, window, cx);
                    } else {
                        open_title(&press, &press_titles, &press_menus, ix, false, cx);
                    }
                })
                .on_hover(move |hovered, _, cx| {
                    if *hovered && hover.read(cx).is_open() && hover_titles.read(cx).at != ix {
                        open_title(&hover, &hover_titles, &hover_menus, ix, false, cx);
                    }
                })
                .child(menu.name.clone())
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            if measure.read(cx).boxes.get(ix) != Some(&bounds) {
                                measure.update(cx, |titles, _| titles.boxes[ix] = bounds);
                            }
                            if here {
                                Open::set_anchor(&follow, bounds, cx);
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
        });
        let (keys, key_titles, key_menus) = (state.clone(), titles.clone(), menus.clone());
        div()
            .id(self.id.clone())
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_0p5()
            .on_key_down(move |event, _, cx| {
                let key = event.keystroke.key.as_str();
                let open = keys.read(cx).is_open();
                let at = key_titles.read(cx).at.min(count - 1);
                let next = |by: isize| (at as isize + by).rem_euclid(count as isize) as usize;
                match key {
                    "left" | "right" => {
                        let to = next(if key == "right" { 1 } else { -1 });
                        if open {
                            open_title(&keys, &key_titles, &key_menus, to, true, cx);
                        } else {
                            key_titles.update(cx, |titles, cx| {
                                titles.at = to;
                                cx.notify();
                            });
                        }
                    }
                    "down" | "enter" | "space" if !open => {
                        open_title(&keys, &key_titles, &key_menus, at, key == "down", cx)
                    }
                    _ => return,
                }
                cx.stop_propagation();
            })
            .relative()
            .children(heads)
            .child(measure_host(state.clone(), false))
            .children(hang(&self.id, &menus[at], &state, None, window, cx))
    }
}
