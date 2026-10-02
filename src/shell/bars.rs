use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window,
    div, prelude::*,
};
use smallvec::SmallVec;

use crate::{
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
};

/// A row of tools, grouped and divided.
#[derive(IntoElement)]
pub struct Toolbar {
    base: Div,
    children: SmallVec<[AnyElement; 4]>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            base: div(),
            children: SmallVec::new(),
        }
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Toolbar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Toolbar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        self.base
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .h(cx.theme().control_height(ControlSize::Lg))
            .px_1()
            .children(self.children)
    }
}

/// Tools that belong together.
#[derive(IntoElement, Default)]
pub struct ToolbarGroup {
    children: SmallVec<[AnyElement; 4]>,
}

impl ToolbarGroup {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ParentElement for ToolbarGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ToolbarGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_0p5()
            .children(self.children)
    }
}

/// Hairline between toolbar groups.
#[derive(IntoElement)]
pub struct ToolbarSeparator;

impl RenderOnce for ToolbarSeparator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .mx_1()
            .h(theme.icon_size(IconSize::Md))
            .border_l_1()
            .border_color(theme.colors.border)
    }
}

/// Thin bar along a window's foot: items left and right.
#[derive(IntoElement)]
pub struct StatusBar {
    base: Div,
    left: SmallVec<[AnyElement; 4]>,
    right: SmallVec<[AnyElement; 4]>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            base: div(),
            left: SmallVec::new(),
            right: SmallVec::new(),
        }
    }

    pub fn left(mut self, item: impl IntoElement) -> Self {
        self.left.push(item.into_any_element());
        self
    }

    pub fn right(mut self, item: impl IntoElement) -> Self {
        self.right.push(item.into_any_element());
        self
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for StatusBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let side = || div().flex().items_center().gap_0p5().h_full();
        self.base
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(theme.control_height(ControlSize::Sm))
            .px_1()
            .bg(theme.colors.bg)
            .border_t_1()
            .border_color(theme.colors.border)
            .text_size(theme.text_size(TextSize::Xs))
            .text_color(theme.colors.fg_muted)
            .child(side().children(self.left))
            .child(side().children(self.right))
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One status: an icon, a label, or both. Clickable when given a handler.
#[derive(IntoElement)]
pub struct StatusBarItem {
    id: ElementId,
    leading: Option<AnyElement>,
    icon: Option<IconName>,
    label: Option<SharedString>,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl StatusBarItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            leading: None,
            icon: None,
            label: None,
            tooltip: None,
            on_click: None,
        }
    }

    /// Drawn before the icon, such as a mark with a count.
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
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

impl RenderOnce for StatusBarItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let hover = theme.colors.hover;
        let name = self
            .label
            .clone()
            .or_else(|| self.tooltip.clone())
            .or_else(|| self.icon.map(|icon| icon.name().into()));
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_1()
            .h_full()
            .px_1p5()
            .rounded(theme.radius(Radius::Sm))
            .children(self.leading)
            .when_some(self.icon, |item, icon| {
                item.child(
                    Icon::new(icon)
                        .size(IconSize::Xs)
                        .color(theme.colors.fg_muted),
                )
            })
            .when_some(self.label, |item, label| item.child(label))
            .when_some(self.tooltip, |item, text| item.tooltip(Tooltip::text(text)))
            .when_some(self.on_click, |item, handler| {
                let item = item.role(Role::Button);
                let item = match name {
                    Some(name) => item.aria_label(name),
                    None => item,
                };
                item.cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .on_click(handler)
            })
    }
}
