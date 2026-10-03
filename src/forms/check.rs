use std::rc::Rc;

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled,
    Toggled, Window, div, prelude::*,
};

use super::{
    Choice,
    options::{OnFlag, OnValues},
};
use crate::{
    i18n, motion,
    primitives::{Icon, IconName, tab_stop},
    theme::{ActiveTheme, IconSize, Mix, Radius, TextSize},
};

/// A checkbox's state. `Mixed` is the dash for a parent whose children disagree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Off,
    On,
    Mixed,
}

impl From<bool> for CheckState {
    fn from(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }
}

/// The box alone, shared by checkboxes, cards and lists. `changes` replays its motion.
pub(crate) fn check_mark(
    state: CheckState,
    focused: bool,
    disabled: bool,
    changes: usize,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let on = state != CheckState::Off;
    let border = if focused {
        colors.focus
    } else if on {
        colors.accent
    } else {
        colors.border_strong
    };
    let (from, to) = if on {
        (colors.surface, colors.accent)
    } else {
        (colors.accent, colors.surface)
    };
    let glyph = match state {
        CheckState::On => Some(IconName::Check),
        CheckState::Mixed => Some(IconName::Minus),
        CheckState::Off => None,
    };
    let ink = colors.on_accent;
    let mark = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(theme.check_size())
        .rounded(theme.radius(Radius::Sm))
        .border_1()
        .border_color(border)
        .bg(to)
        .when(disabled, |mark| mark.opacity(0.5))
        .when_some(glyph, |mark, glyph| {
            let icon = Icon::new(glyph).size(IconSize::Xs).color(ink);
            if changes == 0 {
                return mark.child(icon);
            }
            mark.child(
                icon.with_animation(
                    ("check-glyph", changes),
                    Animation::new(motion::duration(motion::FAST, cx))
                        .with_easing(motion::ease_out_cubic),
                    move |icon, t| icon.scale(0.6 + 0.4 * t).color(ink.opacity(t)),
                ),
            )
        });
    if changes == 0 {
        return mark.into_any_element();
    }
    mark.with_animation(
        ("check-fill", changes),
        Animation::new(motion::duration(motion::FAST, cx)),
        move |mark, t| mark.bg(from.mix(&to, t)),
    )
    .into_any_element()
}

/// A box that is on, off, or mixed, with an optional label beside it.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    state: CheckState,
    label: Option<SharedString>,
    disabled: bool,
    on_change: Option<OnFlag>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, state: impl Into<CheckState>) -> Self {
        Self {
            id: id.into(),
            state: state.into(),
            label: None,
            disabled: false,
            on_change: None,
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

    /// Runs with the new value. Mixed and off both turn on.
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = tab_stop(
            (self.id.clone(), "focus").into(),
            !self.disabled,
            window,
            cx,
        );
        let changes = motion::changes((self.id.clone(), "changes"), self.state, window, cx);
        let focused = focus.is_focused(window);
        let next = self.state != CheckState::On;
        let (id, on_change) = (self.id.clone(), self.on_change);
        let theme = cx.theme();
        let fg = if self.disabled {
            theme.colors.fg_disabled
        } else {
            theme.colors.fg
        };
        let toggled = match self.state {
            CheckState::Off => Toggled::False,
            CheckState::On => Toggled::True,
            CheckState::Mixed => Toggled::Mixed,
        };
        div()
            .id(self.id)
            .role(Role::CheckBox)
            .aria_toggled(toggled)
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
                        log::info!("checkbox {id:?}: {}", if next { "on" } else { "off" });
                        if let Some(on_change) = &on_change {
                            on_change(next, window, cx);
                        }
                    })
            })
            .child(check_mark(self.state, focused, self.disabled, changes, cx))
            .children(self.label.map(|label| {
                div()
                    .debug_selector(|| "checkbox-label".into())
                    .flex_1()
                    .min_w_0()
                    .child(label)
            }))
    }
}

/// Checkboxes for a list of choices; any number can be on.
#[derive(IntoElement)]
pub struct CheckboxGroup {
    id: ElementId,
    choices: Vec<Choice>,
    selected: Vec<SharedString>,
    on_change: Option<OnValues>,
}

impl CheckboxGroup {
    pub fn new(id: impl Into<ElementId>, choices: impl IntoIterator<Item = Choice>) -> Self {
        Self {
            id: id.into(),
            choices: choices.into_iter().collect(),
            selected: Vec::new(),
            on_change: None,
        }
    }

    pub fn selected(mut self, values: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected = values.into_iter().map(Into::into).collect();
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

/// `selected` with `value` turned on or off, in choice order.
pub(crate) fn toggled(
    choices: &[Choice],
    selected: &[SharedString],
    value: &SharedString,
    on: bool,
) -> Vec<SharedString> {
    choices
        .iter()
        .map(|choice| &choice.value)
        .filter(|candidate| {
            if *candidate == value {
                on
            } else {
                selected.contains(candidate)
            }
        })
        .cloned()
        .collect()
}

impl RenderOnce for CheckboxGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (choices, selected, on_change) = (
            Rc::new(self.choices),
            Rc::new(self.selected),
            self.on_change,
        );
        let boxes: Vec<_> = choices
            .iter()
            .enumerate()
            .map(|(ix, choice)| {
                let (choices, selected, on_change) =
                    (choices.clone(), selected.clone(), on_change.clone());
                let value = choice.value.clone();
                Checkbox::new(("check", ix), selected.contains(&choice.value))
                    .label(choice.label.clone())
                    .disabled(choice.disabled)
                    .on_change(move |on, window, cx| {
                        let next = toggled(&choices, &selected, &value, on);
                        if let Some(on_change) = &on_change {
                            on_change(&next, window, cx);
                        }
                    })
            })
            .collect();
        div().id(self.id).flex().flex_col().gap_2().children(boxes)
    }
}

#[cfg(test)]
mod tests {
    use gpui::SharedString;

    use super::{Choice, toggled};

    #[test]
    fn toggling_keeps_choice_order() {
        let choices = [
            Choice::new("a", "A"),
            Choice::new("b", "B"),
            Choice::new("c", "C"),
        ];
        let on = |values: &[&str]| {
            values
                .iter()
                .map(|v| SharedString::from(v.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            toggled(&choices, &on(&["c"]), &"a".into(), true),
            on(&["a", "c"])
        );
        assert_eq!(
            toggled(&choices, &on(&["a", "c"]), &"a".into(), false),
            on(&["c"])
        );
    }
}
