use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, DragMoveEvent, ElementId, EmptyView, EntityId, InteractiveElement,
    IntoElement, MouseButton, Orientation, ParentElement, Pixels, Point, RenderOnce, Role,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::*, relative,
};

use super::options::OnNumber;
use crate::{
    i18n,
    primitives::tab_stop,
    theme::{ActiveTheme, Elevation},
};

/// `value` as a fraction of `min..=max`.
pub(crate) fn fraction(value: f64, min: f64, max: f64) -> f32 {
    ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
}

/// The value at `fraction` of `min..=max`, on the `step` grid.
pub(crate) fn value_at(fraction: f32, min: f64, max: f64, step: f64) -> f64 {
    let raw = min + f64::from(fraction.clamp(0.0, 1.0)) * (max - min);
    (min + ((raw - min) / step).round() * step).clamp(min, max)
}

/// Where a key moves `value`: arrows step, Page keys jump ten, Home and End go to the ends.
pub(crate) fn keyed(key: &str, value: f64, min: f64, max: f64, step: f64) -> Option<f64> {
    let next = match key {
        "left" | "down" => value - step,
        "right" | "up" => value + step,
        "pagedown" => value - step * 10.0,
        "pageup" => value + step * 10.0,
        "home" => min,
        "end" => max,
        _ => return None,
    };
    Some(next.clamp(min, max))
}

/// How far along a track `at` is, from its start: left, or bottom when vertical.
fn along(bounds: Bounds<Pixels>, at: Point<Pixels>, vertical: bool) -> f32 {
    if vertical {
        (bounds.bottom() - at.y) / bounds.size.height
    } else {
        (at.x - bounds.left()) / bounds.size.width
    }
}

/// A drag of thumb `index`, or of the thumb the last track press grabbed.
struct Thumb {
    owner: EntityId,
    index: Option<usize>,
}

type Commit = Rc<dyn Fn(usize, f64, &mut Window, &mut App)>;
type OnSpan = Rc<dyn Fn((f64, f64), &mut Window, &mut App)>;

/// A track with one or two thumbs. `commit` gets the thumb and its new value.
struct Track {
    id: ElementId,
    values: Vec<f64>,
    range: (f64, f64, f64),
    vertical: bool,
    disabled: bool,
    commit: Commit,
}

impl Track {
    fn render(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let (min, max, step) = self.range;
        let vertical = self.vertical;
        let owner = window
            .use_keyed_state((self.id.clone(), "drag"), cx, |_, _| ())
            .entity_id();
        let grabbed = window.use_keyed_state((self.id.clone(), "grab"), cx, |_, _| 0usize);
        let measured = window.use_keyed_state((self.id.clone(), "bounds"), cx, |_, _| {
            Bounds::<Pixels>::default()
        });
        let handles: Vec<_> = (0..self.values.len())
            .map(|k| {
                tab_stop(
                    (self.id.clone(), ["low", "high"][k]).into(),
                    !self.disabled,
                    window,
                    cx,
                )
            })
            .collect();
        let unavailable = self
            .disabled
            .then(|| i18n::text(cx, "state.unavailable", &[]));
        let orientation = match vertical {
            true => Orientation::Vertical,
            false => Orientation::Horizontal,
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let (thumb, line) = (theme.slider_thumb(), theme.slider_track());
        let fractions: Vec<f32> = self
            .values
            .iter()
            .map(|value| fraction(*value, min, max))
            .collect();
        let (start, end) = if fractions.len() == 2 {
            (fractions[0], fractions[1])
        } else {
            (0.0, fractions[0])
        };
        let rail = div()
            .absolute()
            .rounded_full()
            .bg(colors.border)
            .map(|rail| {
                if vertical {
                    rail.top_0()
                        .bottom_0()
                        .left(relative(0.5))
                        .ml(line * -0.5)
                        .w(line)
                } else {
                    rail.left_0()
                        .right_0()
                        .top(relative(0.5))
                        .mt(line * -0.5)
                        .h(line)
                }
            });
        let fill = div()
            .absolute()
            .rounded_full()
            .bg(colors.accent)
            .map(|fill| {
                if vertical {
                    fill.bottom(relative(start))
                        .h(relative(end - start))
                        .left(relative(0.5))
                        .ml(line * -0.5)
                        .w(line)
                } else {
                    fill.left(relative(start))
                        .w(relative(end - start))
                        .top(relative(0.5))
                        .mt(line * -0.5)
                        .h(line)
                }
            });
        let thumbs: Vec<_> = fractions
            .iter()
            .enumerate()
            .map(|(k, at)| {
                let focused = handles[k].is_focused(window);
                let (commit, value) = (self.commit.clone(), self.values[k]);
                let (floor, ceiling) = match (self.values.as_slice(), k) {
                    ([_, high], 0) => (min, *high),
                    ([low, _], _) => (*low, max),
                    _ => (min, max),
                };
                div()
                    .id(("thumb", k))
                    .role(Role::Slider)
                    .aria_numeric_value(value)
                    .aria_min_numeric_value(floor)
                    .aria_max_numeric_value(ceiling)
                    .aria_numeric_value_step(step)
                    .aria_orientation(orientation)
                    .when_some(unavailable.clone(), |thumb, text| {
                        thumb.aria_description(text)
                    })
                    .track_focus(&handles[k])
                    .absolute()
                    .size(thumb)
                    .rounded_full()
                    .bg(colors.surface)
                    .border_1()
                    .border_color(if focused {
                        colors.focus
                    } else {
                        colors.border_strong
                    })
                    .shadow(theme.elevation(Elevation::Raised))
                    .map(|thumb_box| {
                        if vertical {
                            thumb_box.left_0().bottom(relative(*at)).mb(thumb * -0.5)
                        } else {
                            thumb_box.top_0().left(relative(*at)).ml(thumb * -0.5)
                        }
                    })
                    .when(!self.disabled, |thumb_box| {
                        thumb_box
                            .cursor_pointer()
                            .on_drag(
                                Thumb {
                                    owner,
                                    index: Some(k),
                                },
                                |_, _, _, cx| cx.new(|_| EmptyView),
                            )
                            .on_key_down(move |event, window, cx| {
                                if let Some(next) =
                                    keyed(&event.keystroke.key, value, min, max, step)
                                {
                                    cx.stop_propagation();
                                    commit(k, next, window, cx);
                                }
                            })
                    })
            })
            .collect();
        let (drag, jump) = (self.commit.clone(), self.commit);
        let (values, focus) = (self.values, handles);
        let bounds_state = measured.clone();
        let inner = div()
            .id((self.id.clone(), "inner"))
            .relative()
            .size_full()
            .child(rail)
            .child(fill)
            .children(thumbs)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        if *bounds_state.read(cx) != bounds {
                            bounds_state.update(cx, |state, _| *state = bounds);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .when(!self.disabled, |inner| {
                let grab = grabbed.clone();
                inner
                    .on_drag(Thumb { owner, index: None }, |_, _, _, cx| {
                        cx.new(|_| EmptyView)
                    })
                    .on_drag_move(move |event: &DragMoveEvent<Thumb>, window, cx| {
                        let thumb = event.drag(cx);
                        if thumb.owner != owner {
                            return;
                        }
                        let index = thumb.index.unwrap_or(*grabbed.read(cx));
                        let next = value_at(
                            along(event.bounds, event.event.position, vertical),
                            min,
                            max,
                            step,
                        );
                        drag(index, next, window, cx);
                    })
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        let bounds = *measured.read(cx);
                        let next =
                            value_at(along(bounds, event.position, vertical), min, max, step);
                        let index = match values.as_slice() {
                            [low, high] if (next - high).abs() < (next - low).abs() => 1,
                            _ => 0,
                        };
                        grab.update(cx, |grab, _| *grab = index);
                        window.focus(&focus[index], cx);
                        jump(index, next, window, cx);
                    })
            });
        div()
            .debug_selector(|| "slider-root".into())
            .id(self.id)
            .flex_none()
            .map(|outer| {
                if vertical {
                    outer.h_full().w(thumb).py(thumb * 0.5)
                } else {
                    outer.h(thumb).px(thumb * 0.5)
                }
            })
            .when(self.disabled, |outer| outer.opacity(0.5))
            .child(inner)
            .into_any_element()
    }
}

/// A value along a track. Drag the thumb, click the track, or use the arrow keys.
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f64,
    range: (f64, f64, f64),
    vertical: bool,
    disabled: bool,
    on_change: Option<OnNumber>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f64) -> Self {
        Self {
            id: id.into(),
            value,
            range: (0.0, 100.0, 1.0),
            vertical: false,
            disabled: false,
            on_change: None,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        assert!(min < max, "slider range {min}..{max} is empty");
        self.range.0 = min;
        self.range.1 = max;
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        assert!(step > 0.0, "slider step {step} must be positive");
        self.range.2 = step;
        self
    }

    /// Runs bottom to top; the parent gives it a height.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (id, value, on_change) = (self.id.clone(), self.value, self.on_change);
        let commit: Commit = Rc::new(move |_, next, window, cx| {
            if next == value {
                return;
            }
            log::debug!("slider {id:?}: {next}");
            if let Some(on_change) = &on_change {
                on_change(next, window, cx);
            }
        });
        Track {
            id: self.id,
            values: vec![value],
            range: self.range,
            vertical: self.vertical,
            disabled: self.disabled,
            commit,
        }
        .render(window, cx)
    }
}

/// Two thumbs that bound a span. Neither passes the other.
#[derive(IntoElement)]
pub struct RangeSlider {
    id: ElementId,
    value: (f64, f64),
    range: (f64, f64, f64),
    vertical: bool,
    disabled: bool,
    on_change: Option<OnSpan>,
}

impl RangeSlider {
    pub fn new(id: impl Into<ElementId>, low: f64, high: f64) -> Self {
        assert!(low <= high, "range {low}..{high} runs backwards");
        Self {
            id: id.into(),
            value: (low, high),
            range: (0.0, 100.0, 1.0),
            vertical: false,
            disabled: false,
            on_change: None,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        assert!(min < max, "slider range {min}..{max} is empty");
        self.range.0 = min;
        self.range.1 = max;
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        assert!(step > 0.0, "slider step {step} must be positive");
        self.range.2 = step;
        self
    }

    /// Runs bottom to top; the parent gives it a height.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn((f64, f64), &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RangeSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (id, (low, high), on_change) = (self.id.clone(), self.value, self.on_change);
        let commit: Commit = Rc::new(move |index, next, window, cx| {
            let span = if index == 0 {
                (next.min(high), high)
            } else {
                (low, next.max(low))
            };
            if span == (low, high) {
                return;
            }
            log::debug!("range slider {id:?}: {span:?}");
            if let Some(on_change) = &on_change {
                on_change(span, window, cx);
            }
        });
        Track {
            id: self.id,
            values: vec![low, high],
            range: self.range,
            vertical: self.vertical,
            disabled: self.disabled,
            commit,
        }
        .render(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{fraction, keyed, value_at};

    #[test]
    fn values_snap_to_the_step_and_stay_in_range() {
        assert_eq!(fraction(25.0, 0.0, 100.0), 0.25);
        assert_eq!(value_at(0.26, 0.0, 100.0, 5.0), 25.0);
        assert_eq!(value_at(1.4, 0.0, 100.0, 5.0), 100.0);
        assert_eq!(value_at(0.5, -1.0, 1.0, 0.5), 0.0);
    }

    #[test]
    fn keys_step_jump_and_reach_the_ends() {
        assert_eq!(keyed("right", 10.0, 0.0, 100.0, 1.0), Some(11.0));
        assert_eq!(keyed("pagedown", 5.0, 0.0, 100.0, 1.0), Some(0.0));
        assert_eq!(keyed("end", 5.0, 0.0, 100.0, 1.0), Some(100.0));
        assert_eq!(keyed("a", 5.0, 0.0, 100.0, 1.0), None);
    }
}
