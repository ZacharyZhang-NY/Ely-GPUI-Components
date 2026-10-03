use std::rc::Rc;

use gpui::{
    App, Bounds, Div, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    Stateful, Styled, Window, canvas, div, prelude::*,
};

use super::{
    Choice,
    input::text_size,
    options::{OnValue, Pick, Popup, Run, reveal, step},
};
use crate::{
    data_display::Avatar,
    i18n,
    primitives::{Icon, IconName, tab_stop},
    theme::{ActiveTheme, AvatarSize, ControlSize, IconSize, Radius},
    typography::Ellipsis,
};

/// An open list's place and cursor, kept between frames.
#[derive(Default)]
pub(crate) struct Picker {
    pub open: bool,
    pub highlighted: usize,
    pub anchor: Bounds<Pixels>,
    pub scroll: ScrollHandle,
    pub revealed: Option<usize>,
}

impl Picker {
    pub fn show(state: &Entity<Picker>, open: bool, highlighted: usize, cx: &mut App) {
        state.update(cx, |picker, cx| {
            picker.open = open;
            picker.highlighted = highlighted;
            cx.notify();
        });
    }

    /// The row to scroll into view while the list shows `highlighted`.
    pub fn reveal(
        state: &Entity<Picker>,
        shown: bool,
        highlighted: usize,
        cx: &mut App,
    ) -> Option<(usize, Run)> {
        reveal(state, |picker| &mut picker.revealed, shown, highlighted, cx)
    }
}

/// Sizes an invisible layer to its parent and records the bounds in `state`.
pub(crate) fn measure_anchor(state: Entity<Picker>) -> impl IntoElement {
    canvas(
        move |bounds, _, cx| {
            if state.read(cx).anchor != bounds {
                state.update(cx, |picker, _| picker.anchor = bounds);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Up, Down, Home and End over `rows` from `at`; `None` for other keys.
pub(crate) fn moved(event: &KeyDownEvent, rows: &[Choice], at: usize) -> Option<usize> {
    let last = rows.len() - 1;
    match event.keystroke.key.as_str() {
        "down" => Some(step(rows, at, 1)),
        "up" => Some(step(rows, at, -1)),
        "home" => Some(step(rows, last, 1)),
        "end" => Some(step(rows, 0, -1)),
        _ => None,
    }
}

/// The bordered box of a field that opens something: a select, a picker.
pub(crate) fn field_button(
    id: impl Into<ElementId>,
    focus: &FocusHandle,
    size: ControlSize,
    disabled: bool,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let colors = &theme.colors;
    div()
        .debug_selector(|| "field-root".into())
        .id(id)
        .track_focus(focus)
        .relative()
        .flex()
        .items_center()
        .gap_2()
        .h(theme.control_height(size))
        .px(theme.control_padding(size))
        .rounded(theme.radius(Radius::Md))
        .border_1()
        .border_color(if focus.is_focused(window) {
            colors.focus
        } else {
            colors.border_strong
        })
        .bg(if disabled {
            colors.sunken
        } else {
            colors.surface
        })
        .text_size(theme.text_size(text_size(size)))
        .when(!disabled, |field| field.cursor_pointer())
}

/// A field's value, or its placeholder in a quieter tone; long text ends in an ellipsis.
pub(crate) fn field_text(
    text: Option<SharedString>,
    placeholder: SharedString,
    disabled: bool,
    cx: &App,
) -> Div {
    let colors = &cx.theme().colors;
    let color = match (&text, disabled) {
        (_, true) => colors.fg_disabled,
        (Some(_), false) => colors.fg,
        (None, false) => colors.fg_subtle,
    };
    div()
        .flex_1()
        .min_w_0()
        .text_color(color)
        .child(Ellipsis::new(text.unwrap_or(placeholder)))
}

/// The list a trigger opens: its rows, the one chosen, and whether the trigger holds focus.
pub(crate) struct Listing<'a> {
    pub id: &'a ElementId,
    pub rows: Rc<Vec<Choice>>,
    pub selected: Option<&'a SharedString>,
    pub focused: bool,
}

/// Opens `list` from `trigger`: a press or Down toggles it, arrows walk it, Enter picks, Escape or blur closes.
pub(crate) fn listing(
    list: Listing,
    trigger: Stateful<Div>,
    on_pick: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let Listing {
        id,
        rows,
        selected,
        focused,
    } = list;
    let picker = window.use_keyed_state((id.clone(), "picker"), cx, |_, _| Picker::default());
    if picker.read(cx).open && !focused {
        log::info!("list {id:?}: closed on blur");
        picker.update(cx, |picker, _| picker.open = false);
    }
    let current = selected.and_then(|value| rows.iter().position(|row| row.value == *value));
    let start = current.unwrap_or_else(|| step(&rows, rows.len() - 1, 1));
    if picker.read(cx).highlighted >= rows.len() {
        picker.update(cx, |picker, _| picker.highlighted = start);
    }
    let (open, highlighted, anchor, scroll) = {
        let picker = picker.read(cx);
        (
            picker.open,
            picker.highlighted,
            picker.anchor,
            picker.scroll.clone(),
        )
    };
    let reveal = Picker::reveal(&picker, open, highlighted, cx);
    let pick: Pick = {
        let (rows, picker) = (rows.clone(), picker.clone());
        Rc::new(move |ix, window, cx| {
            if rows[ix].disabled {
                return;
            }
            Picker::show(&picker, false, ix, cx);
            on_pick(&rows[ix].value, window, cx);
        })
    };
    let chosen: Vec<SharedString> = current
        .map(|ix| rows[ix].value.clone())
        .into_iter()
        .collect();
    let (toggle, keys, close, keyed_rows, enter) = (
        picker.clone(),
        picker.clone(),
        picker.clone(),
        rows.clone(),
        pick.clone(),
    );
    trigger
        .aria_expanded(open)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            Picker::show(&toggle, !open, start, cx)
        })
        .on_key_down(move |event, window, cx| {
            let at = keys.read(cx).highlighted;
            let key = event.keystroke.key.as_str();
            if !open {
                if matches!(key, "down" | "up" | "enter" | "space") {
                    cx.stop_propagation();
                    Picker::show(&keys, true, start, cx);
                }
                return;
            }
            if let Some(to) = moved(event, &keyed_rows, at) {
                cx.stop_propagation();
                Picker::show(&keys, true, to, cx);
            } else if matches!(key, "enter" | "space") {
                cx.stop_propagation();
                enter(at, window, cx);
            } else if key == "escape" {
                cx.stop_propagation();
                Picker::show(&keys, false, at, cx);
            }
        })
        .child(measure_anchor(picker))
        .when(open, |trigger| {
            trigger.child(
                Popup {
                    id: (id.clone(), "list").into(),
                    anchor,
                    rows: &rows,
                    highlighted: Some(highlighted),
                    checked: Some(&chosen),
                    pick,
                    dismiss: Some(Rc::new(move |_, cx| {
                        let at = close.read(cx).highlighted;
                        Picker::show(&close, false, at, cx)
                    })),
                    scroll: Some(&scroll),
                    reveal,
                }
                .render(window, cx),
            )
        })
}

/// A button that opens a list and shows the one chosen.
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    choices: Vec<Choice>,
    selected: Option<SharedString>,
    placeholder: SharedString,
    size: ControlSize,
    disabled: bool,
    on_change: Option<OnValue>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>, choices: impl IntoIterator<Item = Choice>) -> Self {
        Self {
            id: id.into(),
            choices: choices.into_iter().collect(),
            selected: None,
            placeholder: SharedString::from("Choose…"),
            size: ControlSize::default(),
            disabled: false,
            on_change: None,
        }
    }

    pub fn selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected = Some(value.into());
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        assert!(
            self.disabled || !self.choices.is_empty(),
            "select {:?} has no choices",
            self.id
        );
        let focus = tab_stop(
            (self.id.clone(), "focus").into(),
            !self.disabled,
            window,
            cx,
        );
        let choices = Rc::new(self.choices);
        let shown = self
            .selected
            .as_ref()
            .and_then(|value| choices.iter().find(|choice| choice.value == *value))
            .cloned();
        let colors = &cx.theme().colors;
        let trigger = field_button(
            self.id.clone(),
            &focus,
            self.size,
            self.disabled,
            window,
            cx,
        )
        .role(Role::ComboBox)
        .aria_placeholder(self.placeholder.clone())
        .when_some(shown.as_ref(), |trigger, choice| {
            trigger.aria_value(choice.label.clone())
        })
        .when(self.disabled, |trigger| {
            trigger.aria_description(i18n::text(cx, "state.unavailable", &[]))
        })
        .when_some(
            shown.as_ref().and_then(|choice| choice.icon),
            |trigger, icon| {
                trigger.child(Icon::new(icon).size(IconSize::Sm).color(colors.fg_muted))
            },
        )
        .when_some(
            shown.as_ref().and_then(|choice| choice.avatar.clone()),
            |trigger, name| {
                let id = (self.id.clone(), "avatar");
                trigger.child(Avatar::new(id, name).size(AvatarSize::Xs))
            },
        )
        .child(field_text(
            shown.map(|choice| choice.label),
            self.placeholder,
            self.disabled,
            cx,
        ))
        .child(
            Icon::new(IconName::ChevronsUpDown)
                .size(IconSize::Xs)
                .color(colors.fg_subtle),
        );
        if self.disabled {
            return trigger;
        }
        let (id, on_change) = (self.id.clone(), self.on_change);
        let list = Listing {
            id: &self.id,
            rows: choices,
            selected: self.selected.as_ref(),
            focused: focus.is_focused(window),
        };
        listing(
            list,
            trigger,
            move |value, window, cx| {
                log::info!("select {id:?}: {value}");
                if let Some(on_change) = &on_change {
                    on_change(value, window, cx);
                }
            },
            window,
            cx,
        )
    }
}
