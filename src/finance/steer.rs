use std::rc::Rc;

use gpui::{
    App, Bounds, Div, Entity, HoverListenerMode, InteractiveElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, Stateful, StatefulInteractiveElement,
    Styled, Subscription, Window,
};

use super::{
    market::ChartSync,
    stage::Visible,
    tools::{Drawing, Tool},
};
use crate::charts::{Linear, Rect};

/// A drawing in progress: its tool, and where the drag began and is now, each a candle and a price.
pub(crate) type Sketch = (Tool, (f64, f64), (f64, f64));

/// A market chart's own state: its box, what it shows and where the pointer is when no sync holds them, how many candles it last saw, and a drag in progress.
#[derive(Default)]
pub(crate) struct Stage {
    pub bounds: Bounds<Pixels>,
    pub visible: Option<Visible>,
    pub hover: Option<i64>,
    pub pointer: Option<(f32, f32)>,
    pub seen: usize,
    pub drag: Option<(f32, Visible)>,
    pub sketch: Option<Sketch>,
    pub _synced: Option<Subscription>,
}

pub(crate) type OnDraw = Rc<dyn Fn(Drawing, &mut Window, &mut App)>;

/// What the pointer moves on a market chart: where it keeps what shows, the candles in view out of `total`, the price pane and its scale, and the drawing tool.
pub(crate) struct Steering {
    pub stage: Entity<Stage>,
    pub sync: Option<Entity<ChartSync>>,
    pub visible: Visible,
    pub total: usize,
    pub main: Rect,
    pub price: Linear,
    pub tool: Option<Tool>,
    pub on_draw: Option<OnDraw>,
}

/// Keeps what shows and the candle under the pointer in the sync when one is shared, or in the chart's own stage.
fn hold(
    stage: &Entity<Stage>,
    sync: &Option<Entity<ChartSync>>,
    cx: &mut App,
    visible: Option<Visible>,
    hover: Option<i64>,
) {
    match sync {
        Some(sync) => sync.update(cx, |sync, cx| {
            (sync.visible, sync.hover) = (visible, hover);
            cx.notify();
        }),
        None => stage.update(cx, |stage, cx| {
            (stage.visible, stage.hover) = (visible, hover);
            cx.notify();
        }),
    }
}

/// A window position in the chart box's own pixels.
fn local(position: Point<Pixels>, bounds: Bounds<Pixels>) -> (f32, f32) {
    let offset = position - bounds.origin;
    (f32::from(offset.x), f32::from(offset.y))
}

impl Steering {
    /// Drag or scroll sideways to pan, hold Cmd and scroll to zoom, double-press for the newest, and drag with a tool to draw.
    pub(crate) fn attach(self, root: Stateful<Div>) -> Stateful<Div> {
        let Steering {
            stage,
            sync,
            visible,
            total,
            main,
            price,
            tool,
            on_draw,
        } = self;
        let (moved, pressed, released, wheeled, left) = (
            stage.clone(),
            stage.clone(),
            stage.clone(),
            stage.clone(),
            stage,
        );
        let (sync_move, sync_press, sync_wheel, sync_leave) =
            (sync.clone(), sync.clone(), sync.clone(), sync);
        let across = (main.x, main.w);
        let pinned = move |(x, y): (f32, f32)| {
            (
                visible.start + f64::from((x - across.0) / across.1) * visible.count - 0.5,
                price.value(y),
            )
        };
        root.cursor_crosshair()
            .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                let place = local(event.position, moved.read(cx).bounds);
                let pressed_left = event.pressed_button == Some(MouseButton::Left);
                let drag = moved.read(cx).drag.filter(|_| pressed_left);
                moved.update(cx, |stage, _| {
                    stage.pointer = Some(place);
                    if let Some(sketch) = stage.sketch.as_mut().filter(|_| pressed_left) {
                        sketch.2 = pinned(place);
                    }
                });
                let (visible, hover) = match drag {
                    Some((from, start)) => (
                        start.pan(f64::from((from - place.0) / start.slot(across.1)), total),
                        None,
                    ),
                    None => (
                        visible,
                        (place.0 >= across.0 && place.0 <= across.0 + across.1)
                            .then(|| visible.at(place.0, across)),
                    ),
                };
                hold(&moved, &sync_move, cx, Some(visible), hover);
            })
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                let place = local(event.position, pressed.read(cx).bounds);
                if event.click_count == 2 {
                    log::info!("market chart: back to the newest");
                    hold(&pressed, &sync_press, cx, None, None);
                    return;
                }
                pressed.update(cx, |stage, cx| match tool {
                    Some(tool) if main.contains(place) => {
                        stage.sketch = Some((tool, pinned(place), pinned(place)));
                        cx.notify();
                    }
                    Some(_) => {}
                    None => stage.drag = Some((place.0, visible)),
                });
            })
            .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, window, cx| {
                let sketch = released.update(cx, |stage, cx| {
                    stage.drag = None;
                    cx.notify();
                    stage.sketch.take()
                });
                if let (Some((tool, from, to)), Some(on_draw)) = (sketch, &on_draw) {
                    let drawn = Drawing::of(tool, from, to);
                    log::info!("market chart: drew {drawn:?}");
                    on_draw(drawn, window, cx);
                }
            })
            .on_scroll(move |event: &ScrollEvent, _, cx| {
                let delta = event
                    .delta
                    .pixel_delta(Pixels::from(across.1 / visible.count as f32));
                let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                let place = local(event.position, wheeled.read(cx).bounds);
                let next = if event.modifiers.platform && dy.abs() > dx.abs() {
                    let share = f64::from(((place.0 - across.0) / across.1).clamp(0.0, 1.0));
                    visible.zoom((-f64::from(dy) * 0.004).exp(), share, total)
                } else if dx.abs() > dy.abs() {
                    visible.pan(-f64::from(dx / visible.slot(across.1)), total)
                } else {
                    return;
                };
                cx.stop_propagation();
                hold(&wheeled, &sync_wheel, cx, Some(next), None);
            })
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(move |inside, _, cx| {
                if !*inside {
                    left.update(cx, |stage, _| {
                        (stage.pointer, stage.drag, stage.sketch) = (None, None, None)
                    });
                    hold(&left, &sync_leave, cx, Some(visible), None);
                }
            })
    }
}
