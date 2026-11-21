use std::rc::Rc;

use gpui::{
    App, DragMoveEvent, ElementId, EmptyView, EntityId, ImageSource, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div, img, prelude::*, transparent_black,
};

use crate::{
    generative::lens::{DRAG, START, STEP, turned},
    layout::seeded::use_seeded,
    motion::Spinner,
    overlays::media_button,
    primitives::{FocusRing, IconName, checked_ratio, framed, tab_stop},
    theme::{ActiveTheme, ControlSize, Radius},
};

type OnOrbit = Rc<dyn Fn(Orbit, &mut Window, &mut App)>;

/// Nearest and farthest the camera comes, as shares of the distance that fits the model.
const NEAR: f32 = 0.4;
const FAR: f32 = 4.0;
/// What a key or a button moves the camera nearer or farther by.
const ZOOM: f32 = 1.25;

/// Where the camera sits around a model: turned by yaw and pitch in radians, at a distance that is a share of the one that fits the model in view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Orbit {
    /// The view a model opens at: a little turned, fitted.
    pub const START: Self = Self {
        yaw: START.0,
        pitch: START.1,
        distance: 1.0,
    };

    /// Turned by `by`, the pitch short of either pole.
    pub(crate) fn turned(self, by: (f32, f32)) -> Self {
        let (yaw, pitch) = turned((self.yaw, self.pitch), by);
        Self { yaw, pitch, ..self }
    }

    /// Nearer below 1, farther above, within reach.
    pub(crate) fn scaled(self, by: f32) -> Self {
        Self {
            distance: (self.distance * by).clamp(NEAR, FAR),
            ..self
        }
    }
}

/// An orbiting drag, marked with its viewer.
struct Orbiting(EntityId);

/// A 3D model the host renders, a frame for each view, in the view's shape. A drag or the arrows turn the camera around it; Command-scroll, + and - bring it nearer or farther; 0 or the reset button returns to the start. Until its first frame, it shows a spinner.
#[derive(IntoElement)]
pub struct ModelViewer {
    id: ElementId,
    ratio: f32,
    frame: Option<ImageSource>,
    orbit: Orbit,
    on_orbit: Option<OnOrbit>,
}

impl ModelViewer {
    /// `ratio` is the view's width over its height.
    pub fn new(id: impl Into<ElementId>, ratio: f32, orbit: Orbit) -> Self {
        assert!(
            (NEAR..=FAR).contains(&orbit.distance),
            "a distance of {}",
            orbit.distance
        );
        Self {
            id: id.into(),
            ratio: checked_ratio(ratio),
            frame: None,
            orbit,
            on_orbit: None,
        }
    }

    /// The frame for the orbit now.
    pub fn frame(mut self, source: impl Into<ImageSource>) -> Self {
        self.frame = Some(source.into());
        self
    }

    /// Gets the orbit to draw next.
    pub fn on_orbit(mut self, handler: impl Fn(Orbit, &mut Window, &mut App) + 'static) -> Self {
        self.on_orbit = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ModelViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let orbit = use_seeded((self.id.clone(), "orbit"), self.orbit, window, cx);
        let from = window.use_keyed_state((self.id.clone(), "from"), cx, |_, _| {
            Point::<Pixels>::default()
        });
        let focus = tab_stop(
            (self.id.clone(), "focus").into(),
            self.on_orbit.is_some(),
            window,
            cx,
        );
        let owner = orbit.entity_id();
        let theme = cx.theme();
        let colors = &theme.colors;
        let round = theme.radius(Radius::Lg);
        let body = match self.frame {
            Some(frame) => img(frame)
                .id((self.id.clone(), "frame"))
                .size_full()
                .rounded(round)
                .into_any_element(),
            None => div()
                .debug_selector(|| "model-loading".into())
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new((self.id.clone(), "loading")))
                .into_any_element(),
        };
        let stage = framed(self.ratio, cx)
            .id(self.id.clone())
            .debug_selector(|| "model-viewer".into())
            .rounded(round)
            .border_1()
            .border_color(transparent_black())
            .child(body);
        let Some(send) = self.on_orbit else {
            return stage;
        };
        let move_to = {
            let (orbit, send) = (orbit.clone(), send.clone());
            Rc::new(move |next: Orbit, window: &mut Window, cx: &mut App| {
                orbit.update(cx, |orbit, _| orbit.value = next);
                send(next, window, cx)
            })
        };
        let now = orbit.read(cx).value;
        let button = |key: &'static str, icon: IconName, next: Orbit| {
            let move_to = move_to.clone();
            media_button(
                (self.id.clone(), key),
                icon,
                ControlSize::Sm,
                next != now,
                move |window, cx| {
                    log::info!("model viewer: {key}");
                    move_to(next, window, cx)
                },
                cx,
            )
        };
        let buttons = div()
            .absolute()
            .right_2()
            .bottom_2()
            .flex()
            .items_center()
            .rounded_full()
            .bg(colors.media_backdrop.alpha(0.6))
            .child(button("farther", IconName::Minus, now.scaled(ZOOM)))
            .child(button("nearer", IconName::Plus, now.scaled(1.0 / ZOOM)))
            .child(button("reset", IconName::RotateCcw, Orbit::START));
        let (pressed, dragged, wheeled, keyed) =
            (from.clone(), move_to.clone(), move_to.clone(), move_to);
        let (spun, zoomed, turned_by_key) = (orbit.clone(), orbit.clone(), orbit);
        stage
            .track_focus(&focus)
            .focus_ring(cx)
            .cursor_grab()
            .child(buttons)
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                pressed.update(cx, |from, _| *from = event.position)
            })
            .on_drag(Orbiting(owner), |_, _, _, cx| {
                log::info!("model viewer: a drag turns the view");
                cx.new(|_| EmptyView)
            })
            .on_drag_move(move |event: &DragMoveEvent<Orbiting>, window, cx| {
                if event.drag(cx).0 != owner {
                    return;
                }
                let delta = event.event.position - *from.read(cx);
                from.update(cx, |from, _| *from = event.event.position);
                let by = (f32::from(delta.x) * DRAG, f32::from(delta.y) * DRAG);
                let next = spun.read(cx).value.turned(by);
                dragged(next, window, cx)
            })
            .on_scroll(move |event, window, cx| {
                if !(event.modifiers.platform || event.modifiers.control) {
                    return;
                }
                cx.stop_propagation();
                let delta = event.delta.pixel_delta(window.line_height());
                let next = zoomed
                    .read(cx)
                    .value
                    .scaled((f32::from(delta.y) * 0.01).exp());
                wheeled(next, window, cx)
            })
            .on_key_down(move |event, window, cx| {
                let held = &event.keystroke.modifiers;
                if held.platform || held.control {
                    return;
                }
                let now = turned_by_key.read(cx).value;
                let next = match event.keystroke.key.as_str() {
                    "left" => now.turned((-STEP, 0.0)),
                    "right" => now.turned((STEP, 0.0)),
                    "up" => now.turned((0.0, -STEP)),
                    "down" => now.turned((0.0, STEP)),
                    "+" | "=" => now.scaled(1.0 / ZOOM),
                    "-" => now.scaled(ZOOM),
                    "0" => Orbit::START,
                    _ => return,
                };
                cx.stop_propagation();
                log::info!(
                    "model viewer: turned to {:.2}, {:.2} at {:.2}",
                    next.yaw,
                    next.pitch,
                    next.distance
                );
                keyed(next, window, cx)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{FAR, NEAR, Orbit};

    #[test]
    fn distance_stays_within_reach_and_a_turn_keeps_it() {
        assert_eq!(Orbit::START.scaled(100.0).distance, FAR);
        assert_eq!(Orbit::START.scaled(0.01).distance, NEAR);
        let turned = Orbit::START.scaled(2.0).turned((0.5, 0.0));
        assert_eq!(turned.distance, 2.0);
        assert!((turned.yaw - (Orbit::START.yaw + 0.5)).abs() < 1e-6);
    }
}
