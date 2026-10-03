use std::rc::Rc;

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, FocusHandle, InteractiveElement,
    IntoElement, MouseButton, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::*,
};

use super::{
    Choice,
    options::{OnValue, Run, step},
};
use crate::{
    i18n, motion,
    primitives::tab_stop,
    theme::{ActiveTheme, TextSize},
};

/// The circle alone, shared by radios and cards. `changes` replays its motion.
pub(crate) fn radio_mark(
    on: bool,
    focused: bool,
    disabled: bool,
    changes: usize,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let border = if focused {
        colors.focus
    } else if on {
        colors.accent
    } else {
        colors.border_strong
    };
    let (dot, accent) = (theme.radio_dot(), colors.accent);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(theme.check_size())
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(colors.surface)
        .when(disabled, |mark| mark.opacity(0.5))
        .when(on, |mark| {
            let fill = div().size(dot).rounded_full().bg(accent);
            if changes == 0 {
                return mark.child(fill);
            }
            mark.child(fill.with_animation(
                ("radio-dot", changes),
                Animation::new(motion::duration(motion::BASE, cx)),
                move |fill, t| fill.size(dot * motion::spring(t)),
            ))
        })
        .into_any_element()
}

/// One choice of several, with an optional label beside it.
#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    on: bool,
    label: Option<SharedString>,
    disabled: bool,
    focus: Option<FocusHandle>,
    on_select: Option<Run>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            label: None,
            disabled: false,
            focus: None,
            on_select: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Runs when chosen while off.
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Focus owned by a group that moves it with the arrows.
    fn focus(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = match self.focus {
            Some(handle) => handle,
            None => tab_stop(
                (self.id.clone(), "focus").into(),
                !self.disabled,
                window,
                cx,
            ),
        };
        let changes = motion::changes((self.id.clone(), "changes"), self.on, window, cx);
        let focused = focus.is_focused(window);
        let (on, on_select) = (self.on, self.on_select);
        let theme = cx.theme();
        let fg = if self.disabled {
            theme.colors.fg_disabled
        } else {
            theme.colors.fg
        };
        div()
            .id(self.id)
            .role(Role::RadioButton)
            .aria_toggled(on.into())
            .when_some(self.label.clone(), |row, label| row.aria_label(label))
            .when(self.disabled, |row| {
                row.aria_description(i18n::text(cx, "state.unavailable", &[]))
            })
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_2()
            .text_size(theme.text_size(TextSize::Base))
            .text_color(fg)
            .when(!self.disabled, |row| {
                row.cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(move |_, window, cx| {
                        if let (false, Some(on_select)) = (on, &on_select) {
                            on_select(window, cx);
                        }
                    })
            })
            .child(radio_mark(self.on, focused, self.disabled, changes, cx))
            .children(self.label.map(|label| {
                div()
                    .debug_selector(|| "radio-label".into())
                    .flex_1()
                    .min_w_0()
                    .child(label)
            }))
    }
}

/// Radios for a list of choices. One Tab stop; the arrows move the choice.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    choices: Vec<Choice>,
    selected: Option<SharedString>,
    horizontal: bool,
    on_change: Option<OnValue>,
}

impl RadioGroup {
    pub fn new(id: impl Into<ElementId>, choices: impl IntoIterator<Item = Choice>) -> Self {
        Self {
            id: id.into(),
            choices: choices.into_iter().collect(),
            selected: None,
            horizontal: false,
            on_change: None,
        }
    }

    pub fn selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected = Some(value.into());
        self
    }

    /// Lays the radios in a row.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
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

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.choices.len();
        assert!(count > 0, "radio group {:?} has no choices", self.id);
        let handles = window.use_keyed_state((self.id.clone(), "focus"), cx, |_, _| Vec::new());
        if handles.read(cx).len() != count {
            handles.update(cx, |handles: &mut Vec<FocusHandle>, cx| {
                handles.resize_with(count, || cx.focus_handle());
            });
        }
        let choices = Rc::new(self.choices);
        let chosen = self
            .selected
            .as_ref()
            .and_then(|value| choices.iter().position(|choice| choice.value == *value));
        let stop = match chosen {
            Some(ix) if !choices[ix].disabled => ix,
            _ => step(&choices, count - 1, 1),
        };
        let handles: Vec<FocusHandle> = handles
            .read(cx)
            .iter()
            .enumerate()
            .map(|(ix, handle)| handle.clone().tab_stop(ix == stop && !choices[ix].disabled))
            .collect();
        let pick: OnValue = {
            let (id, on_change) = (self.id.clone(), self.on_change);
            Rc::new(move |value, window, cx| {
                log::info!("radio group {id:?}: {value}");
                if let Some(on_change) = &on_change {
                    on_change(value, window, cx);
                }
            })
        };
        let radios: Vec<_> = choices
            .iter()
            .enumerate()
            .map(|(ix, choice)| {
                let (pick, value) = (pick.clone(), choice.value.clone());
                Radio::new(("radio", ix), chosen == Some(ix))
                    .label(choice.label.clone())
                    .disabled(choice.disabled)
                    .focus(handles[ix].clone())
                    .on_select(move |window, cx| pick(&value, window, cx))
            })
            .collect();
        div()
            .id(self.id)
            .role(Role::RadioGroup)
            .flex()
            .when(self.horizontal, |group| group.flex_row().gap_4())
            .when(!self.horizontal, |group| group.flex_col().gap_2())
            .on_key_down(move |event, window, cx| {
                let by = match event.keystroke.key.as_str() {
                    "up" | "left" => -1,
                    "down" | "right" => 1,
                    _ => return,
                };
                let Some(at) = handles.iter().position(|handle| handle.is_focused(window)) else {
                    return;
                };
                cx.stop_propagation();
                let next = step(&choices, at, by);
                window.focus(&handles[next], cx);
                pick(&choices[next].value, window, cx);
            })
            .children(radios)
    }
}
