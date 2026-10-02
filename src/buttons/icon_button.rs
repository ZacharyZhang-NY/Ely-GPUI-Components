use gpui::{
    App, ClickEvent, ElementId, IntoElement, MouseButton, RenderOnce, Role, SharedString, Window,
    div, prelude::*,
};

use super::button::{ButtonVariant, ClickHandler, label_size, tone};
use crate::{
    i18n,
    primitives::{FocusRing, Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, Radius},
};

/// Square button holding one icon.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            icon,
            variant: ButtonVariant::Ghost,
            size: ControlSize::default(),
            disabled: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
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

    /// Names what the icon does, on hover.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let tone = tone(self.variant, &theme.colors);
        let (_, icon_size) = label_size(self.size);
        let side = theme.control_height(self.size);
        let name: SharedString = self
            .tooltip
            .clone()
            .unwrap_or_else(|| self.icon.name().into());

        div()
            .id(self.id)
            .role(Role::Button)
            .aria_label(name)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(side)
            .rounded(theme.radius(Radius::Md))
            .bg(tone.bg)
            .border_1()
            .border_color(tone.border)
            .child(Icon::new(self.icon).size(icon_size).color(tone.fg))
            .when_some(self.tooltip, |el, text| el.tooltip(Tooltip::text(text)))
            .map(|el| {
                if self.disabled {
                    return el
                        .aria_description(i18n::text(cx, "state.unavailable", &[]))
                        .opacity(0.45)
                        .cursor_not_allowed();
                }
                el.cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(tone.hover))
                    .active(|style| style.bg(tone.pressed))
                    .focus_ring(cx)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .when_some(self.on_click, |el, handler| el.on_click(handler))
            })
    }
}
