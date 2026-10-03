use std::{ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, DragMoveEvent, ElementId, Entity, EntityId, FocusHandle, InteractiveElement,
    IntoElement, MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::*, relative,
};

use super::{
    DropAt, Nav, OnKey, OnMove, OnPick, OnRow, OnToggle, TreeDrag,
    model::{Row, Shown, TreeNode, can_drop, check_state, find, place_at},
};
use crate::{
    forms::{Editing, Input, check_mark},
    lists::select::Pick,
    motion::{self, Spinner},
    primitives::{Disclosure, DragGhost, Icon},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::Ellipsis,
};

/// What drawing rows needs, shared by every range the list asks for.
pub(super) struct Rows {
    pub id: ElementId,
    pub owner: EntityId,
    pub nodes: Rc<Vec<TreeNode>>,
    pub shown: Rc<Vec<Row>>,
    pub selected: Rc<Vec<SharedString>>,
    pub checked: Option<Rc<Vec<SharedString>>>,
    pub current: Option<usize>,
    pub nav: Entity<Nav>,
    pub editing: Entity<Editing>,
    pub focus: FocusHandle,
    pub pick: OnPick,
    pub toggle: OnToggle,
    pub check: Option<OnKey>,
    pub activate: OnRow,
    pub on_move: Option<OnMove>,
}

pub(super) fn draw(
    rows: &Rows,
    range: Range<usize>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    range.map(|ix| row(rows, ix, window, cx)).collect()
}

/// A faint line down each level a row sits under.
fn guides(name: &str, depth: usize, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .absolute()
        .inset_0()
        .px_2()
        .flex()
        .children((0..depth).map(|level| {
            div()
                .flex_none()
                .w(theme.tree_indent())
                .h_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .debug_selector(|| format!("tree-guide-{name}-{level}"))
                        .h_full()
                        .border_l_1()
                        .border_color(theme.colors.border),
                )
        }))
}

fn row(rows: &Rows, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
    let item = &rows.shown[ix];
    let (key, label, icon, opens, open) = match &item.shown {
        Shown::Node {
            key,
            label,
            icon,
            opens,
            open,
            ..
        } => (key.clone(), label.clone(), *icon, *opens, *open),
        Shown::Loading => return waiting(rows, ix, item.depth, cx),
    };
    let checked = rows.checked.as_ref().map(|checked| {
        let state = check_state(find(&rows.nodes, &key).expect("a shown node"), checked);
        (
            state,
            motion::changes((rows.id.clone(), format!("check-{key}")), state, window, cx),
        )
    });
    let nav = rows.nav.read(cx);
    let renaming = nav.renaming.as_ref() == Some(&key);
    let landing = nav
        .drop
        .as_ref()
        .filter(|(target, _)| *target == key)
        .map(|(_, at)| *at);
    let field = renaming.then(|| rows.editing.read(cx).field()).flatten();
    let theme = cx.theme();
    let colors = &theme.colors;
    let indent = theme.tree_indent();
    let (note, ink) = match &item.shown {
        Shown::Node { note, tone, .. } => (note.clone(), tone.map(|tone| tone.colors(colors).1)),
        Shown::Loading => (None, None),
    };
    let selected = rows.selected.contains(&key);
    let (toggle, pick, activate, focus) = (
        rows.toggle.clone(),
        rows.pick.clone(),
        rows.activate.clone(),
        rows.focus.clone(),
    );
    let (owner, nodes, nav, on_move) = (
        rows.owner,
        rows.nodes.clone(),
        rows.nav.clone(),
        rows.on_move.clone(),
    );
    let (dropped_on, dragged_over, shut) = (key.clone(), key.clone(), key.clone());
    let chevron = div()
        .debug_selector(|| format!("tree-chevron-{key}"))
        .flex_none()
        .w(indent)
        .ml(indent * item.depth as f32)
        .flex()
        .justify_center()
        .when(opens, |slot| {
            slot.child(Disclosure::new(
                (rows.id.clone(), format!("chevron-{key}")),
                open,
            ))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                toggle(&shut, !open, cx);
            })
        });
    let body: AnyElement = match field {
        Some(field) => {
            let editing = rows.editing.clone();
            div()
                .flex_1()
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        editing.update(cx, |editing, cx| editing.finish(true, window, cx));
                    }
                })
                .child(Input::new(&field).size(ControlSize::Sm))
                .into_any_element()
        }
        None => div()
            .flex_1()
            .min_w_0()
            .when_some(ink, |text, ink| text.text_color(ink))
            .child(Ellipsis::new(label.clone()))
            .into_any_element(),
    };
    let line = |at: DropAt| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .when(at == DropAt::Before, |line| line.top_0())
            .when(at == DropAt::After, |line| line.bottom_0())
            .border_t_2()
            .border_color(colors.accent)
    };
    div()
        .id((rows.id.clone(), format!("node-{key}")))
        .relative()
        .w_full()
        .h(theme.control_height(ControlSize::Md))
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .rounded(theme.radius(Radius::Sm))
        .text_size(theme.text_size(TextSize::Sm))
        .text_color(colors.fg)
        .when(selected, |row| row.bg(colors.active))
        .when(!selected, |row| row.hover(|style| style.bg(colors.hover)))
        .when(landing == Some(DropAt::Inside), |row| {
            row.bg(colors.selection)
        })
        .child(guides(&key, item.depth, cx))
        .child(chevron)
        .children(
            checked
                .zip(rows.check.clone())
                .map(|((state, turns), check)| {
                    let key = key.clone();
                    div()
                        .flex_none()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            check(&key, window, cx);
                        })
                        .child(check_mark(state, false, false, turns, cx))
                }),
        )
        .children(icon.map(|icon| Icon::new(icon).size(IconSize::Sm).color(colors.fg_muted)))
        .child(body)
        .children(note.map(|note| {
            div()
                .debug_selector(|| "tree-note".into())
                .flex_none()
                .max_w(relative(0.5))
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(ink.unwrap_or(colors.fg_subtle))
                .child(Ellipsis::new(note))
        }))
        .children(landing.filter(|at| *at != DropAt::Inside).map(line))
        .when(rows.current == Some(ix), |row| {
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(theme.radius(Radius::Sm))
                    .border_1()
                    .border_color(colors.focus),
            )
        })
        .on_click(move |event, window, cx| {
            window.focus(&focus, cx);
            if event.click_count() == 2 {
                activate(ix, window, cx);
                return;
            }
            let held = event.modifiers();
            let how = match (held.shift, held.platform) {
                (true, _) => Pick::Range,
                (false, true) => Pick::Toggle,
                _ => Pick::One,
            };
            pick(ix, how, window, cx);
        })
        .when(!renaming, |row| {
            row.on_drag(
                TreeDrag {
                    owner,
                    key: key.clone(),
                    label: label.clone(),
                },
                move |drag, _, _, cx| DragGhost::new(drag.label.clone(), icon, cx),
            )
        })
        .on_drag_move(move |event: &DragMoveEvent<TreeDrag>, _, cx| {
            let drag = event.drag(cx);
            if drag.owner != owner || !event.bounds.contains(&event.event.position) {
                return;
            }
            let share = (event.event.position.y - event.bounds.top()) / event.bounds.size.height;
            let fits = drag.key != dragged_over && can_drop(&nodes, &drag.key, &dragged_over);
            let next = fits.then(|| (dragged_over.clone(), place_at(share, opens)));
            nav.update(cx, |nav, cx| {
                if nav.drop != next {
                    nav.drop = next;
                    cx.notify();
                }
            });
        })
        .on_drop({
            let nav = rows.nav.clone();
            move |drag: &TreeDrag, window, cx| {
                if drag.owner != owner {
                    return;
                }
                let landed = nav.update(cx, |nav, cx| {
                    cx.notify();
                    nav.drop.take()
                });
                if let (Some((target, at)), Some(on_move)) = (landed, &on_move)
                    && target == dropped_on
                {
                    log::info!("tree: {} dropped {at:?} {target}", drag.key);
                    on_move(&drag.key, &target, at, window, cx);
                }
            }
        })
        .into_any_element()
}

/// The row under a pending node while its children come.
fn waiting(rows: &Rows, ix: usize, depth: usize, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .id((rows.id.clone(), format!("waiting-{ix}")))
        .relative()
        .w_full()
        .h(theme.control_height(ControlSize::Md))
        .flex()
        .items_center()
        .px_2()
        .child(guides(&format!("waiting-{ix}"), depth, cx))
        .child(
            div()
                .flex_none()
                .w(theme.tree_indent() * (depth + 1) as f32),
        )
        .child(Spinner::new((rows.id.clone(), format!("spinner-{ix}"))).size(IconSize::Xs))
        .into_any_element()
}
