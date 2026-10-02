use gpui::{
    App, ClickEvent, ElementId, FocusHandle, FontWeight, Hsla, IntoElement, MouseButton,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Window, div, prelude::*,
    transparent_black,
};

use crate::{
    i18n,
    motion::Spinner,
    primitives::{FocusRing, Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, Mix, Palette, Platform, Radius, TextSize},
    typography::keys::{keystroke, keystroke_labels},
};

pub(crate) type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Outline,
    Ghost,
    Subtle,
    /// Confirms a gain, such as a purchase; green.
    Success,
    Danger,
    Link,
}

pub(crate) struct Tone {
    pub bg: Hsla,
    pub hover: Hsla,
    pub pressed: Hsla,
    pub fg: Hsla,
    pub border: Hsla,
}

pub(crate) fn tone(variant: ButtonVariant, colors: &Palette) -> Tone {
    let clear = transparent_black();
    match variant {
        ButtonVariant::Primary => Tone {
            bg: colors.accent,
            hover: colors.accent_hover,
            pressed: colors.accent_hover.mix(&colors.on_accent, 0.12),
            fg: colors.on_accent,
            border: clear,
        },
        ButtonVariant::Secondary => Tone {
            bg: colors.surface,
            hover: colors.hover,
            pressed: colors.active,
            fg: colors.fg,
            border: colors.border,
        },
        ButtonVariant::Outline => Tone {
            bg: clear,
            hover: colors.hover,
            pressed: colors.active,
            fg: colors.fg,
            border: colors.border_strong,
        },
        ButtonVariant::Ghost => Tone {
            bg: clear,
            hover: colors.hover,
            pressed: colors.active,
            fg: colors.fg,
            border: clear,
        },
        ButtonVariant::Subtle => Tone {
            bg: colors.hover,
            hover: colors.active,
            pressed: colors.active.mix(&colors.border_strong, 0.5),
            fg: colors.fg,
            border: clear,
        },
        ButtonVariant::Success => Tone {
            bg: colors.success,
            hover: colors.success.mix(&colors.fg, 0.14),
            pressed: colors.success.mix(&colors.fg, 0.24),
            fg: colors.on_accent,
            border: clear,
        },
        ButtonVariant::Danger => Tone {
            bg: colors.danger,
            hover: colors.danger.mix(&colors.fg, 0.14),
            pressed: colors.danger.mix(&colors.fg, 0.24),
            fg: colors.on_accent,
            border: clear,
        },
        ButtonVariant::Link => Tone {
            bg: clear,
            hover: clear,
            pressed: clear,
            fg: colors.link,
            border: clear,
        },
    }
}

pub(crate) fn label_size(size: ControlSize) -> (TextSize, IconSize) {
    match size {
        ControlSize::Sm => (TextSize::Sm, IconSize::Xs),
        ControlSize::Md => (TextSize::Base, IconSize::Sm),
        ControlSize::Lg => (TextSize::Md, IconSize::Md),
    }
}

/// Where a button sits inside a `ButtonGroup`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    First,
    Middle,
    Last,
}

/// Fails on a keystroke gpui cannot read, when a builder takes it.
pub(crate) fn keystroke_of(source: &str) {
    keystroke(source, Platform::current());
}

/// A keystroke in gpui key syntax as `platform` spells it: `⌘S`, or `Ctrl+S`.
pub(crate) fn shortcut_text(source: &str, platform: Platform) -> String {
    let glue = if platform == Platform::Mac { "" } else { "+" };
    keystroke_labels(&keystroke(source, platform), platform).join(glue)
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    trailing_icon: Option<IconName>,
    shortcut: Option<SharedString>,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    loading: bool,
    full_width: bool,
    focus: Option<FocusHandle>,
    pub(crate) slot: Option<Slot>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            trailing_icon: None,
            shortcut: None,
            variant: ButtonVariant::default(),
            size: ControlSize::default(),
            disabled: false,
            loading: false,
            full_width: false,
            focus: None,
            slot: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn trailing_icon(mut self, icon: IconName) -> Self {
        self.trailing_icon = Some(icon);
        self
    }

    /// Shows the keystroke that also runs it, e.g. `"cmd-s"`. Panics on a bad keystroke.
    pub fn shortcut(mut self, keystroke: &str) -> Self {
        keystroke_of(keystroke);
        self.shortcut = Some(SharedString::from(keystroke.to_string()));
        self
    }

    /// Takes focus through the owner's handle, one that outlives a mode where the button changes; a Tab stop when built with `tab_stop`.
    pub fn focus_handle(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Spins in place of its content, at the same width, and ignores presses. Tab still reaches it.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
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

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let tone = tone(self.variant, &theme.colors);
        let (text, icon_size) = label_size(self.size);
        let link = self.variant == ButtonVariant::Link;
        let icon = |name| Icon::new(name).size(icon_size).color(tone.fg);
        let radius = theme.radius(Radius::Md);
        let platform = cx.theme().platform;
        let hint = self.shortcut.map(|source| shortcut_text(&source, platform));
        let spinner = (self.id.clone(), "spinner");
        let name = self.label.clone();
        let parts = div()
            .flex()
            .items_center()
            .gap_1p5()
            .when_some(self.icon, |el, name| el.child(icon(name)))
            .when(!self.label.is_empty(), |el| el.child(self.label))
            .when_some(self.trailing_icon, |el, name| el.child(icon(name)))
            .when_some(hint, |el, hint| {
                el.child(
                    div()
                        .pl_1()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(tone.fg.opacity(0.55))
                        .child(hint),
                )
            });

        div()
            .id(self.id)
            .role(Role::Button)
            .when(!name.is_empty(), |el| el.aria_label(name))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap_1p5()
            .h(theme.control_height(self.size))
            .when(!link, |el| el.px(theme.control_padding(self.size)))
            .border_1()
            .border_color(tone.border)
            .map(|el| match self.slot {
                None => el.rounded(radius),
                Some(Slot::First) => el.rounded_l(radius),
                Some(Slot::Middle) => el.border_l_0(),
                Some(Slot::Last) => el.rounded_r(radius).border_l_0(),
            })
            .text_size(theme.text_size(text))
            .font_weight(FontWeight::MEDIUM)
            .text_color(tone.fg)
            .bg(tone.bg)
            .when(self.full_width, |el| el.w_full())
            .map(|el| {
                if !self.loading {
                    return el.child(parts);
                }
                el.relative().child(parts.invisible()).child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Spinner::new(spinner).size(icon_size).color(tone.fg)),
                )
            })
            .map(|el| {
                if self.disabled {
                    return el
                        .aria_description(i18n::text(cx, "state.unavailable", &[]))
                        .opacity(0.45)
                        .cursor_not_allowed();
                }
                let el = match &self.focus {
                    Some(handle) => el.track_focus(handle),
                    None => el.tab_index(0),
                };
                if self.loading {
                    return el
                        .focus_ring(cx)
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default());
                }
                el.cursor_pointer()
                    .hover(|style| {
                        let style = style.bg(tone.hover);
                        if link { style.underline() } else { style }
                    })
                    .active(|style| style.bg(tone.pressed))
                    .focus_ring(cx)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .when_some(self.on_click, |el, handler| el.on_click(handler))
            })
    }
}
