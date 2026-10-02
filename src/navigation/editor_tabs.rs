use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, EntityId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::*,
};

use super::TabOverflowMenu;
use crate::{
    buttons::{ButtonVariant, IconButton},
    forms::Choice,
    i18n,
    primitives::{DragGhost, Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, TextSize},
};

/// One open document: its id, title and state.
#[derive(Clone, Debug)]
pub struct EditorTab {
    id: SharedString,
    title: SharedString,
    icon: Option<IconName>,
    dirty: bool,
    pinned: bool,
    preview: bool,
}

impl EditorTab {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            dirty: false,
            pinned: false,
            preview: false,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Unsaved work: a dot that turns into the close button on hover.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// A pin in place of the close button.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// A tab the next preview replaces, shown in italics.
    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }
}

struct TabDrag {
    owner: EntityId,
    from: usize,
}

type OnTab = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type OnReorder = Rc<dyn Fn(usize, usize, &mut Window, &mut App)>;

/// Document tabs: close, pin, drag to reorder, keep a preview. Every tab is listed at the end.
#[derive(IntoElement)]
pub struct EditorTabs {
    id: ElementId,
    tabs: Vec<EditorTab>,
    selected: Option<SharedString>,
    on_select: Option<OnTab>,
    on_close: Option<OnTab>,
    on_pin: Option<OnTab>,
    on_keep: Option<OnTab>,
    on_reorder: Option<OnReorder>,
}

impl EditorTabs {
    pub fn new(id: impl Into<ElementId>, tabs: impl IntoIterator<Item = EditorTab>) -> Self {
        Self {
            id: id.into(),
            tabs: tabs.into_iter().collect(),
            selected: None,
            on_select: None,
            on_close: None,
            on_pin: None,
            on_keep: None,
            on_reorder: None,
        }
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// The close button, a middle click, or the button over a dirty dot.
    pub fn on_close(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// The pin on a pinned tab, which unpins it.
    pub fn on_pin(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pin = Some(Rc::new(handler));
        self
    }

    /// A double click on a preview tab, which keeps it open.
    pub fn on_keep(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_keep = Some(Rc::new(handler));
        self
    }

    /// Reports `(from, to)` when a tab is dropped on another.
    pub fn on_reorder(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }
}

fn run(handler: &Option<OnTab>, what: &str, id: &SharedString, window: &mut Window, cx: &mut App) {
    log::info!("editor tabs: {what} {id}");
    if let Some(handler) = handler {
        handler(id, window, cx);
    }
}

impl RenderOnce for EditorTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let owner = window
            .use_keyed_state((self.id.clone(), "drag"), cx, |_, _| ())
            .entity_id();
        let scroll = window
            .use_keyed_state((self.id.clone(), "scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let chosen = self
            .selected
            .as_ref()
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == *id));
        let followed = window.use_keyed_state((self.id.clone(), "followed"), cx, |_, _| {
            None::<Option<SharedString>>
        });
        let follow = (followed.read(cx).as_ref() != Some(&self.selected))
            .then(|| (followed, self.selected.clone(), scroll.clone()));
        let theme = cx.theme();
        let colors = &theme.colors;
        let wide = theme.tab_width().1;
        let tabs: Vec<AnyElement> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(ix, tab)| {
                let on = chosen == Some(ix);
                let group = SharedString::from(format!("editor-tab-{}", tab.id));
                let fg = if on { colors.fg } else { colors.fg_muted };
                let (select, keep, close) = (
                    self.on_select.clone(),
                    self.on_keep.clone(),
                    self.on_close.clone(),
                );
                let (id, middle_id, preview) = (tab.id.clone(), tab.id.clone(), tab.preview);
                let slot_id = tab.id.clone();
                let slot: AnyElement = if tab.pinned {
                    let pin = self.on_pin.clone();
                    IconButton::new(("pin", ix), IconName::Pin)
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .tooltip("Unpin")
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            run(&pin, "unpin", &slot_id, window, cx);
                        })
                        .into_any_element()
                } else {
                    let close_button = {
                        let close = self.on_close.clone();
                        IconButton::new(("close", ix), IconName::X)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .tooltip(i18n::text(cx, "dialog.close", &[]))
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                run(&close, "close", &slot_id, window, cx);
                            })
                    };
                    let hidden = tab.dirty || !on;
                    div()
                        .relative()
                        .flex_none()
                        .size(theme.control_height(ControlSize::Sm))
                        .when(tab.dirty, |slot| {
                            slot.child(
                                div()
                                    .absolute()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .group_hover(group.clone(), |style| style.invisible())
                                    .child(div().size(theme.status_dot()).rounded_full().bg(fg)),
                            )
                        })
                        .child(
                            div()
                                .absolute()
                                .size_full()
                                .when(hidden, |cover| {
                                    cover
                                        .invisible()
                                        .group_hover(group.clone(), |style| style.visible())
                                })
                                .child(close_button),
                        )
                        .into_any_element()
                };
                let reorder = self.on_reorder.clone();
                let (title, icon, edge) = (tab.title.clone(), tab.icon, colors.focus);
                div()
                    .id(tab.id.clone())
                    .role(Role::Tab)
                    .aria_selected(on)
                    .aria_label(tab.title.clone())
                    .group(group)
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_1p5()
                    .max_w(wide)
                    .h(theme.control_height(ControlSize::Lg))
                    .pl_3()
                    .pr_1()
                    .border_r_1()
                    .border_color(colors.border)
                    .text_size(theme.text_size(TextSize::Sm))
                    .text_color(fg)
                    .map(|el| {
                        if on {
                            el.bg(colors.bg).child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .h(theme.tab_indicator())
                                    .bg(colors.accent),
                            )
                        } else {
                            el.hover(|style| style.text_color(colors.fg))
                        }
                    })
                    .when(preview, |el| el.italic())
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                        run(&close, "close", &middle_id, window, cx)
                    })
                    .on_click(move |event, window, cx| {
                        if preview && event.click_count() == 2 {
                            run(&keep, "keep", &id, window, cx);
                        } else {
                            run(&select, "select", &id, window, cx);
                        }
                    })
                    .when_some(reorder, |el, reorder| {
                        el.on_drag(TabDrag { owner, from: ix }, move |_, _, _, cx| {
                            DragGhost::new(title.clone(), icon, cx)
                        })
                        .drag_over::<TabDrag>(move |style, drag, _, _| {
                            if drag.owner == owner {
                                style.border_color(edge)
                            } else {
                                style
                            }
                        })
                        .on_drop(move |drag: &TabDrag, window, cx| {
                            if drag.owner == owner && drag.from != ix {
                                log::info!("editor tabs: move {} -> {ix}", drag.from);
                                reorder(drag.from, ix, window, cx);
                            }
                        })
                    })
                    .when_some(tab.icon, |el, icon| {
                        el.child(Icon::new(icon).size(IconSize::Sm).color(fg))
                    })
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(tab.title.clone()),
                    )
                    .child(slot)
                    .into_any_element()
            })
            .collect();
        let listed: Vec<Choice> = self
            .tabs
            .iter()
            .map(|tab| {
                let choice = Choice::new(tab.id.clone(), tab.title.clone());
                match tab.icon {
                    Some(icon) => choice.icon(icon),
                    None => choice,
                }
            })
            .collect();
        let overflow = (!listed.is_empty()).then(|| {
            let select = self.on_select.clone();
            let menu = TabOverflowMenu::new((self.id.clone(), "overflow"), listed)
                .on_select(move |id, window, cx| run(&select, "select", id, window, cx));
            match &self.selected {
                Some(id) => menu.selected(id.clone()),
                None => menu,
            }
        });
        div()
            .id(self.id)
            .flex()
            .items_center()
            .bg(colors.sunken)
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .id("strip")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .children(tabs)
                    .children(follow.map(|(followed, selected, scroll)| {
                        canvas(
                            move |_, window, cx| {
                                if scroll.bounds().size.width <= Pixels::ZERO {
                                    return;
                                }
                                followed.update(cx, |followed, _| *followed = Some(selected));
                                if let Some(ix) = chosen {
                                    scroll.scroll_to_item(ix);
                                    window.request_animation_frame();
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                    })),
            )
            .children(overflow.map(|menu| div().flex_none().px_1().child(menu)))
    }
}
