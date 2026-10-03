use std::rc::Rc;

use gpui::{
    Animation, AnimationExt, App, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, transparent_black,
};

use super::options::OnFlag;
use crate::{
    i18n, motion,
    primitives::tab_stop,
    theme::{ActiveTheme, Elevation, Mix, TextSize},
};

/// An on-off switch. The thumb slides over with a small overshoot.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    label: Option<SharedString>,
    disabled: bool,
    on_change: Option<OnFlag>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
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

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = tab_stop(
            (self.id.clone(), "focus").into(),
            !self.disabled,
            window,
            cx,
        );
        let changes = motion::changes((self.id.clone(), "changes"), self.on, window, cx);
        let focused = focus.is_focused(window);
        let theme = cx.theme();
        let colors = &theme.colors;
        let track = theme.switch_track();
        let travel = track.width - track.height;
        let (off, on) = (colors.border_strong, colors.accent);
        let (knob_off, knob_on) = (colors.fg_muted, colors.on_accent);
        let (from, to, fill_from, fill_to, knob_from, knob_to) = if self.on {
            (0.0, 1.0, off, on, knob_off, knob_on)
        } else {
            (1.0, 0.0, on, off, knob_on, knob_off)
        };
        let thumb = div()
            .h_full()
            .map(|mut thumb| {
                thumb.style().aspect_ratio = Some(1.0);
                thumb
            })
            .rounded_full()
            .bg(knob_to)
            .shadow(theme.elevation(Elevation::Raised));
        let rail = div()
            .flex()
            .flex_none()
            .items_center()
            .w(track.width)
            .h(track.height)
            .p_0p5()
            .rounded_full()
            .border_1()
            .border_color(if focused {
                colors.focus
            } else {
                transparent_black()
            })
            .bg(fill_to)
            .when(self.disabled, |rail| rail.opacity(0.5));
        let rail = if changes == 0 {
            rail.child(thumb.ml(travel * to)).into_any_element()
        } else {
            let duration = motion::duration(motion::BASE, cx);
            rail.child(thumb.with_animation(
                ("switch-thumb", changes),
                Animation::new(duration),
                move |thumb, t| {
                    thumb
                        .ml(travel * motion::lerp(from, to, motion::spring(t)))
                        .bg(knob_from.mix(&knob_to, t))
                },
            ))
            .with_animation(
                ("switch-fill", changes),
                Animation::new(duration),
                move |rail, t| rail.bg(fill_from.mix(&fill_to, t)),
            )
            .into_any_element()
        };
        let (id, next, on_change) = (self.id.clone(), !self.on, self.on_change);
        div()
            .id(self.id)
            .role(Role::Switch)
            .aria_toggled(self.on.into())
            .when_some(self.label.clone(), |row, label| row.aria_label(label))
            .when(self.disabled, |row| {
                row.aria_description(i18n::text(cx, "state.unavailable", &[]))
            })
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_2()
            .text_size(theme.text_size(TextSize::Base))
            .text_color(if self.disabled {
                colors.fg_disabled
            } else {
                colors.fg
            })
            .when(!self.disabled, |row| {
                row.cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(move |_, window, cx| {
                        log::info!("switch {id:?}: {}", if next { "on" } else { "off" });
                        if let Some(on_change) = &on_change {
                            on_change(next, window, cx);
                        }
                    })
            })
            .child(rail)
            .children(self.label.map(|label| {
                div()
                    .debug_selector(|| "switch-label".into())
                    .flex_1()
                    .min_w_0()
                    .child(label)
            }))
    }
}
