use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, Keystroke, ParentElement, RenderOnce, Styled,
    Window, div, prelude::*,
};

use crate::{
    primitives::{FocusRing, Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::KbdCombo,
};

type OnChange = Rc<dyn Fn(Option<Keystroke>, &mut Window, &mut App)>;

/// Records one shortcut, then lets go. Backspace clears; Escape leaves.
#[derive(IntoElement)]
pub struct HotkeyInput {
    id: ElementId,
    value: Option<Keystroke>,
    on_change: Option<OnChange>,
}

impl HotkeyInput {
    pub fn new(id: impl Into<ElementId>, value: Option<Keystroke>) -> Self {
        Self {
            id: id.into(),
            value,
            on_change: None,
        }
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(Option<Keystroke>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for HotkeyInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let recording = focus.is_focused(window);
        let theme = cx.theme();
        let colors = &theme.colors;
        let on_change = self.on_change;
        div()
            .id(self.id)
            .track_focus(&focus)
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .h(theme.control_height(ControlSize::Md))
            .px(theme.control_padding(ControlSize::Md))
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(colors.border_strong)
            .bg(colors.surface)
            .focus_ring(cx)
            .cursor_pointer()
            .text_size(theme.text_size(TextSize::Base))
            .on_key_down(move |event, window, cx| {
                cx.stop_propagation();
                let stroke = &event.keystroke;
                let bare = !stroke.modifiers.modified();
                let next = match stroke.key.as_str() {
                    "escape" if bare => {
                        window.blur(cx);
                        return;
                    }
                    "backspace" | "delete" if bare => None,
                    _ => Some(Keystroke {
                        key_char: None,
                        ..stroke.clone()
                    }),
                };
                log::info!(
                    "hotkey input: {}",
                    next.as_ref().map_or("cleared".into(), |stroke| stroke
                        .unparse(cx.platform_info().os))
                );
                window.blur(cx);
                if let Some(on_change) = &on_change {
                    on_change(next, window, cx);
                }
            })
            .map(|field| match (&self.value, recording) {
                (_, true) => field.child(
                    div()
                        .text_color(colors.fg_subtle)
                        .child("Press a shortcut…"),
                ),
                (Some(stroke), false) => {
                    field.child(KbdCombo::new(&stroke.unparse(cx.platform_info().os)))
                }
                (None, false) => field.child(div().text_color(colors.fg_subtle).child("None")),
            })
            .child(
                Icon::new(IconName::Keyboard)
                    .size(IconSize::Sm)
                    .color(colors.fg_subtle),
            )
    }
}
