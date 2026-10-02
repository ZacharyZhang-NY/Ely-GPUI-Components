use std::rc::Rc;

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled,
    Window, anchored, div, point, prelude::*,
};
use smallvec::SmallVec;

use crate::{
    buttons::{ButtonVariant, IconButton},
    forms::{Enter, Run},
    motion,
    primitives::{Backdrop, FocusScope, IconName, give_back, raise, take_focus},
    theme::{ActiveTheme, Elevation, Radius, TextSize},
};

/// A dialog's way out: it hands focus back, then runs the owner's close.
pub type Close = Rc<dyn Fn(&mut Window, &mut App)>;

type Action = Box<dyn FnOnce(Close) -> AnyElement>;
type OnEnter = Box<dyn FnOnce(Close) -> Run>;

/// A card over a scrim: a title, a line of detail, the owner's content and a row of actions. Escape closes it, Tab stays inside, and focus returns on every close.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    detail: Option<SharedString>,
    body: SmallVec<[AnyElement; 2]>,
    actions: SmallVec<[Action; 2]>,
    enter: Option<OnEnter>,
    scrim_closes: bool,
    fullscreen: bool,
    first: Option<FocusHandle>,
    on_close: Run,
}

impl Dialog {
    /// Render it while open; `on_close` runs on Escape, a press on the scrim, or the close button.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            body: SmallVec::new(),
            actions: SmallVec::new(),
            enter: None,
            scrim_closes: true,
            fullscreen: false,
            first: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn detail(mut self, text: impl Into<SharedString>) -> Self {
        self.detail = Some(text.into());
        self
    }

    /// A button in the row at the bottom, right-aligned in the order given. `build` gets the dialog's close; a button that closes calls it.
    pub fn action<E: IntoElement>(mut self, build: impl FnOnce(Close) -> E + 'static) -> Self {
        self.actions
            .push(Box::new(move |close| build(close).into_any_element()));
        self
    }

    /// Runs on an unmodified Enter while the `focus_first` field holds focus; `build` gets the dialog's close. The field's key binding fires on key-down, so an Enter an IME takes never reaches it.
    pub(crate) fn on_enter(mut self, build: impl FnOnce(Close) -> Run + 'static) -> Self {
        self.enter = Some(Box::new(build));
        self
    }

    /// Fills the window below its title bar, under a bar with the title and a close button.
    pub fn fullscreen(mut self) -> Self {
        self.fullscreen = true;
        self
    }

    /// Only its actions and Escape close it; a press on the scrim does not.
    pub(crate) fn held(mut self) -> Self {
        self.scrim_closes = false;
        self
    }

    /// Where focus goes when it opens.
    pub(crate) fn focus_first(mut self, handle: FocusHandle) -> Self {
        self.first = Some(handle);
        self
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let takeover = take_focus((self.id.clone(), "takeover"), window, cx);
        let focus = takeover.read(cx).focus.clone();
        if let Some(first) = &self.first
            && focus.is_focused(window)
        {
            window.focus(first, cx);
        }
        let close: Run = {
            let (id, on_close) = (self.id.clone(), self.on_close);
            Rc::new(move |window, cx| {
                log::info!("dialog {id:?}: closed");
                give_back(&takeover, window, cx);
                on_close(window, cx)
            })
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let heading = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(theme.text_size(TextSize::Lg))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.fg)
                    .child(self.title),
            )
            .when_some(self.detail, |heading, detail| {
                heading.child(
                    div()
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(colors.fg_muted)
                        .child(detail),
                )
            });
        let buttons: Vec<AnyElement> = self
            .actions
            .into_iter()
            .map(|build| build(close.clone()))
            .collect();
        let actions = (!buttons.is_empty()).then(|| {
            div()
                .flex_none()
                .flex()
                .justify_end()
                .gap_2()
                .children(buttons)
        });
        let enter = self.enter.map(|build| {
            let field = self
                .first
                .clone()
                .expect("a dialog that takes Enter names its field with focus_first");
            (field, build(close.clone()))
        });
        let (escape, dismiss, button) = (close.clone(), close.clone(), close);
        let fullscreen = self.fullscreen;
        let card = if fullscreen {
            div()
                .id(self.id.clone())
                .flex()
                .flex_col()
                .size_full()
                .bg(colors.bg)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .px_6()
                        .py_4()
                        .border_b_1()
                        .border_color(colors.border)
                        .child(heading)
                        .child(
                            IconButton::new((self.id.clone(), "close"), IconName::X)
                                .variant(ButtonVariant::Ghost)
                                .tooltip("Close")
                                .on_click(move |_, window, cx| button(window, cx)),
                        ),
                )
                .child(
                    div()
                        .id((self.id.clone(), "body"))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_6()
                        .children(self.body),
                )
                .children(actions.map(|row| {
                    div()
                        .px_6()
                        .py_4()
                        .border_t_1()
                        .border_color(colors.border)
                        .child(row)
                }))
        } else {
            let margin = theme.titlebar_height().to_pixels(window.rem_size());
            let body = (!self.body.is_empty()).then(|| {
                div()
                    .id((self.id.clone(), "body"))
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().flex().flex_col().gap_5().children(self.body))
            });
            div()
                .id(self.id.clone())
                .flex()
                .flex_col()
                .gap_5()
                .w(theme.dialog_width())
                .max_h(window.viewport_size().height - margin * 2.0)
                .p_6()
                .rounded(theme.radius(Radius::Xl))
                .bg(colors.overlay)
                .border_1()
                .border_color(colors.border)
                .shadow(theme.elevation(Elevation::Modal))
                .child(heading)
                .children(body)
                .children(actions)
        };
        let arrive =
            Animation::new(motion::duration(motion::BASE, cx)).with_easing(motion::ease_out_cubic);
        let card = if fullscreen {
            card.with_animation((self.id.clone(), "in"), arrive, |card, t| {
                card.opacity(t).top(motion::NUDGE * 4.0 * (1.0 - t))
            })
            .into_any_element()
        } else {
            // Padding grows the box the scrim centers.
            div()
                .child(card)
                .with_animation((self.id.clone(), "in"), arrive, |card, t| {
                    card.opacity(t).pt(motion::NUDGE * (1.0 - t))
                })
                .into_any_element()
        };
        let layer = div()
            .when(fullscreen, |layer| layer.size_full())
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    escape(window, cx);
                }
            })
            .when_some(enter, |layer, (field, enter)| {
                layer.capture_action(move |_: &Enter, window, cx| {
                    cx.stop_propagation();
                    if field.is_focused(window) {
                        enter(window, cx);
                    }
                })
            })
            .child(
                FocusScope::new(&focus)
                    .trap()
                    .when(fullscreen, |scope| scope.size_full())
                    .child(card),
            );
        if fullscreen {
            let viewport = window.viewport_size();
            let top = cx.theme().titlebar_height().to_pixels(window.rem_size());
            return raise(
                (self.id.clone(), "raised"),
                anchored().position(point(Pixels::ZERO, top)).child(
                    div()
                        .w(viewport.width)
                        .h(viewport.height - top)
                        .occlude()
                        .child(layer),
                ),
            )
            .with_priority(1)
            .into_any_element();
        }
        let scrim_closes = self.scrim_closes;
        Backdrop::new((self.id, "scrim"))
            .on_dismiss(move |window, cx| {
                if scrim_closes {
                    dismiss(window, cx);
                }
            })
            .child(layer)
            .into_any_element()
    }
}
