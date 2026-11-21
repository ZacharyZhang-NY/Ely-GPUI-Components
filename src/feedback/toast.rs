use std::{rc::Rc, time::Duration};

use gpui::{
    Anchor, Animation, AnimationExt, AnyElement, App, ClickEvent, Context, ElementId, Entity,
    FontWeight, HoverListenerMode, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Task, Window, anchored,
    canvas, div, point, prelude::*, relative,
};
use web_time::Instant;

use crate::{
    buttons::{Button, ButtonVariant, IconButton},
    forms::Run,
    motion,
    primitives::{Icon, IconName, Severity, raise},
    theme::{ActiveTheme, ControlSize, Elevation, IconSize, Radius, TextSize},
};

/// How long a toast stays unless told otherwise.
const STAY: Duration = Duration::from_secs(5);
/// A toast past this many sends the oldest away.
const MOST: usize = 4;

/// A short message for a `ToastViewport`. With an action it is a snackbar; with `undo` it counts down.
#[derive(Clone)]
pub struct Toast {
    title: SharedString,
    body: Option<SharedString>,
    severity: Option<Severity>,
    action: Option<(SharedString, Run)>,
    undo: Option<Run>,
    stay: Option<Duration>,
}

impl Toast {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: None,
            severity: None,
            action: None,
            undo: None,
            stay: Some(STAY),
        }
    }

    pub fn body(mut self, text: impl Into<SharedString>) -> Self {
        self.body = Some(text.into());
        self
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// A button that runs `handler`, then sends the toast away.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// An Undo button, and a line that runs out with the time left to press it.
    pub fn undo(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.undo = Some(Rc::new(handler));
        self
    }

    /// How long it stays; `None` keeps it until closed.
    pub fn stay(mut self, stay: Option<Duration>) -> Self {
        assert!(stay != Some(Duration::ZERO), "a toast must stay a while");
        self.stay = stay;
        self
    }
}

/// A toast on screen, and the time it has left.
pub(super) struct Live {
    pub(super) id: u64,
    toast: Toast,
    left: Duration,
    since: Option<Instant>,
    runs: usize,
    pub(super) leaving: bool,
    _timer: Option<Task<()>>,
}

/// The toasts a `ToastViewport` shows. Keep one per window and push to it.
#[derive(Default)]
pub struct Toaster {
    pub(super) live: Vec<Live>,
    next: u64,
    held: bool,
}

impl Toaster {
    /// Shows `toast`; the id sends it away early.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> u64 {
        let id = self.next;
        self.next += 1;
        // Titles may quote user content: never logged.
        log::info!("toast {id}: {:?}", toast.severity);
        let mut live = Live {
            id,
            left: toast.stay.unwrap_or_default(),
            toast,
            since: None,
            runs: 0,
            leaving: false,
            _timer: None,
        };
        if !self.held {
            run(&mut live, cx);
        }
        self.live.push(live);
        let staying: Vec<u64> = self.staying().map(|live| live.id).collect();
        if staying.len() > MOST {
            self.dismiss(staying[0], cx);
        }
        cx.notify();
        id
    }

    /// The titles and severities on show, oldest first.
    pub fn shown(&self) -> Vec<(SharedString, Option<Severity>)> {
        let staying = self.live.iter().filter(|live| !live.leaving);
        staying
            .map(|live| (live.toast.title.clone(), live.toast.severity))
            .collect()
    }

    /// Sends a toast away: it fades and folds, then leaves the stack.
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(live) = self
            .live
            .iter_mut()
            .find(|live| live.id == id && !live.leaving)
        else {
            log::debug!("toast {id}: already gone");
            return;
        };
        log::info!("toast {id}: dismissed");
        live.leaving = true;
        let exit = motion::duration(motion::BASE, cx);
        live._timer = Some(cx.spawn(async move |toaster, cx| {
            cx.background_executor().timer(exit).await;
            let gone = toaster.update(cx, |toaster, cx| {
                toaster.live.retain(|live| live.id != id);
                if toaster.live.is_empty() {
                    toaster.held = false;
                }
                cx.notify();
            });
            if let Err(error) = gone {
                log::error!("toast {id}: the toaster went first: {error:#}");
            }
        }));
        cx.notify();
    }

    fn staying(&self) -> impl Iterator<Item = &Live> {
        self.live.iter().filter(|live| !live.leaving)
    }

    /// Clocks wait while the pointer is on the stack.
    fn hold(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.held == held {
            return;
        }
        self.held = held;
        log::debug!("toasts: {}", if held { "held" } else { "running" });
        let now = cx.background_executor().now();
        for live in self.live.iter_mut().filter(|live| !live.leaving) {
            if !held {
                run(live, cx);
            } else if let Some(since) = live.since.take() {
                live.left = live.left.saturating_sub(now - since);
                live._timer = None;
            }
        }
        cx.notify();
    }
}

/// Starts a toast's clock; at zero the toast is dismissed.
fn run(live: &mut Live, cx: &mut Context<Toaster>) {
    if live.toast.stay.is_none() {
        return;
    }
    let (id, left) = (live.id, live.left);
    live.since = Some(cx.background_executor().now());
    live.runs += 1;
    live._timer = Some(cx.spawn(async move |toaster, cx| {
        cx.background_executor().timer(left).await;
        if let Err(error) = toaster.update(cx, |toaster, cx| toaster.dismiss(id, cx)) {
            log::error!("toast {id}: the toaster went first: {error:#}");
        }
    }));
}

/// Where a window's toasts show: stacked in its bottom-right corner, the newest nearest it.
#[derive(IntoElement)]
pub struct ToastViewport {
    id: ElementId,
    toaster: Entity<Toaster>,
}

impl ToastViewport {
    pub fn new(id: impl Into<ElementId>, toaster: &Entity<Toaster>) -> Self {
        Self {
            id: id.into(),
            toaster: toaster.clone(),
        }
    }
}

impl RenderOnce for ToastViewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.toaster.read(cx).live.is_empty() {
            return div().id(self.id).into_any_element();
        }
        let theme = cx.theme();
        let (width, margin) = (theme.toast_width(), theme.window_margin());
        let viewport = window.viewport_size();
        let hold = self.toaster.clone();
        let mut cards = Vec::new();
        for ix in 0..self.toaster.read(cx).live.len() {
            cards.push(card(&self.id, ix, &self.toaster, window, cx));
        }
        let stack = div()
            .id(self.id.clone())
            .occlude()
            .flex()
            .flex_col()
            .w(width)
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(move |hovered, _, cx| {
                hold.update(cx, |toaster, cx| toaster.hold(*hovered, cx))
            })
            .children(cards);
        raise(
            (self.id, "raised"),
            anchored()
                .position(point(viewport.width - margin, viewport.height - margin))
                .anchor(Anchor::BottomRight)
                .child(stack),
        )
        .with_priority(1)
        .into_any_element()
    }
}

/// One toast: it rises in as its room opens, and folds away under the newer ones.
fn card(
    owner: &ElementId,
    ix: usize,
    toaster: &Entity<Toaster>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let live = &toaster.read(cx).live[ix];
    let (id, leaving, toast) = (live.id, live.leaving, live.toast.clone());
    let key = ElementId::from((owner.clone(), SharedString::from(format!("toast-{id}"))));
    let countdown = countdown(&key, live, cx);
    let height = window.use_keyed_state((key.clone(), "height"), cx, |_, _| Pixels::ZERO);
    let room = *height.read(cx);
    let send = |run: Option<Run>| {
        let toaster = toaster.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            if !toaster.read(cx).staying().any(|live| live.id == id) {
                log::debug!("toast {id}: pressed while leaving");
                return;
            }
            if let Some(run) = &run {
                run(window, cx);
            }
            toaster.update(cx, |toaster, cx| toaster.dismiss(id, cx));
        }
    };
    let theme = cx.theme();
    let colors = &theme.colors;
    // Read aloud: alerts interrupt, other notes wait their turn.
    let role = match toast.severity {
        Some(Severity::Danger | Severity::Warning) => Role::Alert,
        _ => Role::Status,
    };
    let card = div()
        .id(key.clone())
        .role(role)
        .aria_label(toast.title.clone())
        .when_some(toast.body.clone(), |card, body| card.aria_description(body))
        .relative()
        .flex()
        .items_center()
        .gap_3()
        .p_3()
        .rounded(theme.radius(Radius::Lg))
        .bg(colors.surface)
        .border_1()
        .border_color(colors.border)
        .shadow(theme.elevation(Elevation::Floating))
        .text_size(theme.text_size(TextSize::Sm))
        .when_some(toast.severity, |card, severity| {
            card.child(
                Icon::new(severity.icon())
                    .size(IconSize::Sm)
                    .color(severity.color(colors)),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.fg)
                        .child(toast.title),
                )
                .when_some(toast.body, |text, body| {
                    text.child(div().text_color(colors.fg_muted).child(body))
                }),
        )
        .when_some(toast.undo, |card, undo| {
            card.child(
                Button::new((key.clone(), "undo"), "Undo")
                    .size(ControlSize::Sm)
                    .on_click(send(Some(undo))),
            )
        })
        .when_some(toast.action, |card, (label, run)| {
            card.child(
                Button::new((key.clone(), "action"), label)
                    .size(ControlSize::Sm)
                    .on_click(send(Some(run))),
            )
        })
        .child(
            IconButton::new((key.clone(), "close"), IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .on_click(send(None)),
        )
        .children(countdown);
    let placed = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .pt_2()
        .child(card)
        .child(
            canvas(
                move |bounds, window, cx| {
                    if *height.read(cx) != bounds.size.height {
                        height.update(cx, |height, cx| {
                            *height = bounds.size.height;
                            cx.notify();
                            window.request_animation_frame();
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    let frame = div().relative().w_full().child(placed);
    if leaving {
        frame
            .with_animation(
                (key, "out"),
                Animation::new(motion::duration(motion::BASE, cx))
                    .with_easing(motion::ease_in_out_cubic),
                move |frame, t| frame.h(room * (1.0 - t)).opacity(1.0 - t),
            )
            .into_any_element()
    } else {
        frame
            .with_animation(
                (key, "in"),
                Animation::new(motion::duration(motion::SLOW, cx))
                    .with_easing(motion::ease_out_cubic),
                move |frame, t| frame.h(room * t).opacity(t),
            )
            .into_any_element()
    }
}

/// The line under an undo toast that runs out with its time.
fn countdown(key: &ElementId, live: &Live, cx: &App) -> Option<impl IntoElement + use<>> {
    let total = live.toast.stay.filter(|_| live.toast.undo.is_some())?;
    let from = live.left.as_secs_f32() / total.as_secs_f32();
    let theme = cx.theme();
    let line = div().h_full().bg(theme.colors.border_strong);
    let line = match live.since {
        Some(_) => line
            .with_animation(
                (key.clone(), format!("left-{}", live.runs)),
                Animation::new(live.left),
                move |line, t| line.w(relative(from * (1.0 - t))),
            )
            .into_any_element(),
        None => line.w(relative(from)).into_any_element(),
    };
    Some(
        div()
            .absolute()
            .left_3()
            .right_3()
            .bottom_1()
            .h(theme.progress_thickness())
            .child(line),
    )
}
