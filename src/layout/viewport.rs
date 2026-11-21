use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, RenderOnce, Styled, Window, canvas, div, point,
};

/// Maps content space to the viewport: `local = content * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub offset: Point<Pixels>,
    pub scale: f32,
}

impl Transform {
    pub fn apply(&self, content: Point<Pixels>) -> Point<Pixels> {
        point(
            content.x * self.scale + self.offset.x,
            content.y * self.scale + self.offset.y,
        )
    }

    pub fn invert(&self, local: Point<Pixels>) -> Point<Pixels> {
        point(
            (local.x - self.offset.x) / self.scale,
            (local.y - self.offset.y) / self.scale,
        )
    }

    /// Scales by `factor`, clamped, keeping `anchor` fixed on screen.
    fn zoom_at(self, anchor: Point<Pixels>, factor: f32, range: (f32, f32)) -> Self {
        let scale = (self.scale * factor).clamp(range.0, range.1);
        let ratio = scale / self.scale;
        Self {
            offset: point(
                anchor.x - (anchor.x - self.offset.x) * ratio,
                anchor.y - (anchor.y - self.offset.y) * ratio,
            ),
            scale,
        }
    }
}

struct View {
    transform: Transform,
    bounds: Bounds<Pixels>,
    grab: Option<(Point<Pixels>, Point<Pixels>)>,
}

type Content = Rc<dyn Fn(Transform, &mut Window, &mut App) -> AnyElement>;

/// Pan with a drag or the wheel; zoom with ⌘/Ctrl and the wheel.
#[derive(IntoElement)]
pub struct Viewport {
    id: ElementId,
    range: (f32, f32),
    content: Content,
}

impl Viewport {
    /// `content` places itself with the transform it receives.
    pub fn new(
        id: impl Into<ElementId>,
        content: impl Fn(Transform, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            range: (0.1, 8.0),
            content: Rc::new(content),
        }
    }

    pub fn zoom_range(mut self, min: f32, max: f32) -> Self {
        assert!(0.0 < min && min <= max, "zoom range {min}..{max} is empty");
        self.range = (min, max);
        self
    }
}

impl RenderOnce for Viewport {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| View {
            transform: Transform {
                offset: Point::default(),
                scale: 1.0,
            },
            bounds: Bounds::default(),
            grab: None,
        });
        let transform = state.read(cx).transform;
        let range = self.range;
        let (wheel, down, drag, up, out, measure) = (
            state.clone(),
            state.clone(),
            state.clone(),
            state.clone(),
            state.clone(),
            state,
        );
        div()
            .id(self.id)
            .relative()
            .size_full()
            .overflow_hidden()
            .on_scroll(move |event, window, cx| {
                let delta = event.delta.pixel_delta(window.line_height());
                wheel.update(cx, |view, cx| {
                    view.transform = if event.modifiers.secondary(cx) || event.modifiers.control {
                        let anchor = event.position - view.bounds.origin;
                        let factor = (-f32::from(delta.y) * 0.01).exp();
                        view.transform.zoom_at(anchor, factor, range)
                    } else {
                        Transform {
                            offset: view.transform.offset + delta,
                            ..view.transform
                        }
                    };
                    cx.notify();
                });
                cx.stop_propagation();
            })
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                down.update(cx, |view, _| {
                    view.grab = Some((event.position, view.transform.offset))
                })
            })
            .on_mouse_move(move |event, _, cx| {
                drag.update(cx, |view, cx| {
                    if let Some((anchor, start)) = view.grab.filter(|_| event.dragging()) {
                        view.transform.offset = start + (event.position - anchor);
                        cx.notify();
                    }
                })
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |view, _| view.grab = None)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                out.update(cx, |view, _| view.grab = None)
            })
            .child(
                canvas(
                    move |bounds, _, cx| {
                        if measure.read(cx).bounds != bounds {
                            measure.update(cx, |view, _| view.bounds = bounds);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child((self.content)(transform, window, cx))
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::Transform;

    #[test]
    fn apply_and_invert_are_inverse() {
        let transform = Transform {
            offset: point(px(30.0), px(-20.0)),
            scale: 2.5,
        };
        let content = point(px(12.0), px(8.0));
        let back = transform.invert(transform.apply(content));
        assert!((f32::from(back.x) - 12.0).abs() < 1e-4 && (f32::from(back.y) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn zoom_keeps_the_anchor_still_and_clamps() {
        let start = Transform {
            offset: point(px(10.0), px(10.0)),
            scale: 1.0,
        };
        let anchor = point(px(100.0), px(50.0));
        let zoomed = start.zoom_at(anchor, 2.0, (0.1, 8.0));
        let before = start.invert(anchor);
        let after = zoomed.invert(anchor);
        assert!((f32::from(before.x) - f32::from(after.x)).abs() < 1e-4);
        assert_eq!(start.zoom_at(anchor, 100.0, (0.1, 8.0)).scale, 8.0);
    }
}
