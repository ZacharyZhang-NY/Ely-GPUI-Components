use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, WindowHandle, div, prelude::*,
};

use super::{Hosted, open_hosted};
use crate::{
    buttons::{Button, ButtonVariant, IconButton},
    i18n::text,
    layout::ScrollArea,
    motion::ProgressBar,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::{Caption, Label, Title},
};

/// Where an update stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdateState {
    Available,
    /// 0 to 1.
    Downloading(f32),
    Ready,
}

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// Handlers for the update flow. Unset ones hide their button.
#[derive(Clone, Default)]
struct Actions {
    download: Option<Action>,
    restart: Option<Action>,
    later: Option<Action>,
}

fn progress_line(state: UpdateState) -> Option<AnyElement> {
    match state {
        UpdateState::Downloading(value) => {
            Some(ProgressBar::new("update-progress", value).into_any_element())
        }
        _ => None,
    }
}

fn buttons(state: UpdateState, actions: &Actions, size: ControlSize, cx: &App) -> Vec<Button> {
    let wire = |button: Button, action: &Option<Action>| {
        action.clone().map(|action| {
            button
                .size(size)
                .on_click(move |_, window, cx| action(window, cx))
        })
    };
    let later = wire(
        Button::new("update-later", text(cx, "update.later", &[])).variant(ButtonVariant::Ghost),
        &actions.later,
    );
    let main = match state {
        UpdateState::Available => wire(
            Button::new("update-download", text(cx, "update.download", &[])).primary(),
            &actions.download,
        ),
        UpdateState::Downloading(_) => None,
        UpdateState::Ready => wire(
            Button::new("update-restart", text(cx, "update.restart", &[])).primary(),
            &actions.restart,
        ),
    };
    later.into_iter().chain(main).collect()
}

macro_rules! update_actions {
    () => {
        /// Starts the download while `Available`.
        pub fn on_download(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
            self.actions.download = Some(Rc::new(handler));
            self
        }

        /// Relaunches once `Ready`.
        pub fn on_restart(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
            self.actions.restart = Some(Rc::new(handler));
            self
        }

        pub fn on_later(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
            self.actions.later = Some(Rc::new(handler));
            self
        }
    };
}

/// A slim bar that offers a new version, shows the download, then asks to restart.
#[derive(IntoElement, Clone)]
pub struct UpdateBanner {
    id: ElementId,
    version: SharedString,
    state: UpdateState,
    actions: Actions,
}

impl UpdateBanner {
    pub fn new(
        id: impl Into<ElementId>,
        version: impl Into<SharedString>,
        state: UpdateState,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            state,
            actions: Actions::default(),
        }
    }

    update_actions!();
}

impl RenderOnce for UpdateBanner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let version = &self.version;
        let message = match self.state {
            UpdateState::Available => text(cx, "update.available", &[("version", version)]),
            UpdateState::Downloading(value) => {
                let percent = (value * 100.0).round().to_string();
                text(
                    cx,
                    "update.downloading",
                    &[("version", version), ("percent", &percent)],
                )
            }
            UpdateState::Ready => text(cx, "update.ready", &[("version", version)]),
        };
        let theme = cx.theme();
        let dismiss = self.actions.later.clone();
        div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .gap_3()
            .py_2()
            .pl_4()
            .pr_2()
            .bg(theme.colors.surface)
            .border_b_1()
            .border_color(theme.colors.border)
            .text_size(theme.text_size(TextSize::Sm))
            .child(
                Icon::new(IconName::Download)
                    .size(IconSize::Sm)
                    .color(theme.colors.fg_muted),
            )
            .child(div().flex_1().child(message))
            .children(buttons(
                self.state,
                &Actions {
                    later: None,
                    ..self.actions.clone()
                },
                ControlSize::Sm,
                cx,
            ))
            .when_some(dismiss, |banner, dismiss| {
                banner.child(
                    IconButton::new("update-dismiss", IconName::X)
                        .size(ControlSize::Sm)
                        .on_click(move |_, window, cx| dismiss(window, cx)),
                )
            })
            .when_some(progress_line(self.state), |banner, line| {
                banner.child(div().absolute().left_0().right_0().bottom_0().child(line))
            })
    }
}

/// The update window: what changed, and what to do.
#[derive(IntoElement, Clone)]
pub struct UpdateDialog {
    name: SharedString,
    current: SharedString,
    version: SharedString,
    notes: Vec<SharedString>,
    state: UpdateState,
    actions: Actions,
}

impl UpdateDialog {
    pub fn new(
        name: impl Into<SharedString>,
        current: impl Into<SharedString>,
        version: impl Into<SharedString>,
        state: UpdateState,
    ) -> Self {
        Self {
            name: name.into(),
            current: current.into(),
            version: version.into(),
            notes: Vec::new(),
            state,
            actions: Actions::default(),
        }
    }

    /// One line of release notes.
    pub fn note(mut self, line: impl Into<SharedString>) -> Self {
        self.notes.push(line.into());
        self
    }

    pub fn state(mut self, state: UpdateState) -> Self {
        self.state = state;
        self
    }

    update_actions!();

    pub fn open(self, cx: &mut App) -> anyhow::Result<WindowHandle<Hosted<UpdateDialog>>> {
        let size = cx.theme().dialog_window();
        open_hosted(text(cx, "update.title", &[]), size, self, cx)
    }
}

impl RenderOnce for UpdateDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let args = [
            ("name", self.name.as_ref()),
            ("version", self.version.as_ref()),
        ];
        let headline = match self.state {
            UpdateState::Ready => text(cx, "update.headline.ready", &args),
            _ => text(cx, "update.headline.available", &args),
        };
        let current = text(cx, "update.current", &[("current", &self.current)]);
        let theme = cx.theme();
        let colors = &theme.colors;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_4()
            .px_8()
            .pb_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Title::new(headline))
                    .child(Caption::new(current)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .rounded(theme.radius(Radius::Lg))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.sunken)
                    .child(
                        ScrollArea::new("update-notes").size_full().child(
                            div().flex().flex_col().gap_2().p_4().children(
                                self.notes.into_iter().map(|line| {
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(Label::new("·"))
                                        .child(Label::new(line))
                                }),
                            ),
                        ),
                    ),
            )
            .when_some(progress_line(self.state), |dialog, line| dialog.child(line))
            .child(div().flex().justify_end().gap_2().children(buttons(
                self.state,
                &self.actions,
                ControlSize::Md,
                cx,
            )))
    }
}
