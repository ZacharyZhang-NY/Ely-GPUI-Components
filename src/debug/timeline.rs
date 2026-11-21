use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, ScrollEvent, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::*, relative,
};

use super::OnIndex;
use crate::{
    charts::{measure, nice_step},
    primitives::FocusRing,
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
    typography::{Ellipsis, LEADING, format::decimals},
};

/// A stretch of work on a track, in milliseconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub track: usize,
    pub name: SharedString,
    pub start: f64,
    pub end: f64,
}

/// Wheel pixels that zoom by a factor of e.
const ZOOM: f64 = 400.0;

/// A tick's time, in as many places as its step needs.
fn tick_label(time: f64, step: f64) -> String {
    format!("{time:.*} ms", decimals(step))
}

/// Ticks along the ruler, at most.
const TICKS: usize = 6;

/// The ruler's step and ticks: as many as leave `room` for each label across `width`, each with room before the end.
pub(crate) fn ruler(range: (f64, f64), width: f32, room: f32) -> (f64, Vec<f64>) {
    let length = range.1 - range.0;
    let across = move |span: f64| (span / length) as f32 * width;
    let step = (2..=TICKS)
        .rev()
        .map(|count| nice_step(length, count))
        .find(|step| across(*step) >= room)
        .unwrap_or_else(|| nice_step(length, 1));
    let first = (range.0 / step).ceil() as i64;
    let ticks = (first..)
        .map(|n| n as f64 * step)
        .take_while(|time| across(range.1 - time) >= room)
        .collect();
    (step, ticks)
}

/// `range` scaled by `factor` around the point at `at` across it, from 0 to 1; above 1 widens, below narrows.
pub fn zoomed(range: (f64, f64), at: f64, factor: f64) -> (f64, f64) {
    assert!(
        factor > 0.0 && range.1 > range.0,
        "a range zooms by a positive factor"
    );
    let pivot = range.0 + (range.1 - range.0) * at.clamp(0.0, 1.0);
    (
        pivot - (pivot - range.0) * factor,
        pivot + (range.1 - pivot) * factor,
    )
}

/// A steady color for a span's name.
fn tint(name: &str, palette: &[Hsla; 8]) -> Hsla {
    let hash = name.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });
    palette[hash as usize % palette.len()]
}

type OnRange = Rc<dyn Fn((f64, f64), &mut Window, &mut App)>;

/// Work over time on tracks, such as threads: each span a bar from its start to its end under a time axis. The wheel zooms around the pointer and a sideways wheel pans; a press picks a span.
#[derive(IntoElement)]
pub struct TimelineProfiler {
    id: ElementId,
    tracks: Vec<SharedString>,
    spans: Rc<Vec<Span>>,
    range: (f64, f64),
    selected: Option<usize>,
    on_range: Option<OnRange>,
    on_select: Option<OnIndex>,
}

impl TimelineProfiler {
    /// `range` is the stretch of time in view, in milliseconds.
    pub fn new(
        id: impl Into<ElementId>,
        tracks: impl IntoIterator<Item = impl Into<SharedString>>,
        spans: impl Into<Rc<Vec<Span>>>,
        range: (f64, f64),
    ) -> Self {
        Self {
            id: id.into(),
            tracks: tracks.into_iter().map(Into::into).collect(),
            spans: spans.into(),
            range,
            selected: None,
            on_range: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, span: usize) -> Self {
        self.selected = Some(span);
        self
    }

    pub fn on_range(
        mut self,
        handler: impl Fn((f64, f64), &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_range = Some(Rc::new(handler));
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TimelineProfiler {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let lane: Entity<Bounds<Pixels>> =
            window.use_keyed_state((self.id.clone(), "lane"), cx, |_, _| Bounds::default());
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let line = theme
            .control_height(ControlSize::Md)
            .to_pixels(window.rem_size());
        let (from, to) = self.range;
        let length = to - from;
        let at = move |time: f64| ((time - from) / length) as f32;
        let room = theme.chart().label.to_pixels(window.rem_size());
        let width = lane.read(cx).size.width;
        let (step, ticks) = ruler(self.range, f32::from(width), f32::from(room));
        let wheel_lane = lane.clone();
        let on_range = self.on_range.clone();
        let label = theme.label_width() * 0.6;
        let row = theme.control_height(ControlSize::Md);
        let selected = self.selected.and_then(|ix| self.spans.get(ix).cloned());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(theme.text_size(TextSize::Xs))
            .child(
                div().flex().child(div().flex_none().w(label)).child(
                    div()
                        .relative()
                        .flex_1()
                        .h(theme.text_size(TextSize::Xs) * LEADING)
                        .line_height(relative(LEADING))
                        .overflow_hidden()
                        .border_b_1()
                        .border_color(colors.border)
                        .children(ticks.iter().map(|time| {
                            div()
                                .absolute()
                                .left(relative(at(*time)))
                                .top_0()
                                .pl_1()
                                .border_l_1()
                                .border_color(colors.border)
                                .text_color(colors.fg_subtle)
                                .child(tick_label(*time, step))
                        })),
                ),
            )
            .child(
                div()
                    .id(self.id.clone())
                    .flex()
                    .flex_col()
                    .gap_px()
                    .on_scroll(move |event: &ScrollEvent, window, cx| {
                        let Some(on_range) = &on_range else {
                            return;
                        };
                        let bounds = *wheel_lane.read(cx);
                        let delta = event.delta.pixel_delta(line);
                        let (x, y) = (f64::from(f32::from(delta.x)), f64::from(f32::from(delta.y)));
                        cx.stop_propagation();
                        let width = f64::from(f32::from(bounds.size.width)).max(1.0);
                        let next = if x.abs() > y.abs() {
                            let shift = -x / width * length;
                            Some((from + shift, to + shift))
                        } else {
                            let pointer =
                                f64::from(f32::from(event.position.x - bounds.origin.x)) / width;
                            let factor = (-y / ZOOM).exp();
                            (factor > 0.0 && factor.is_finite())
                                .then(|| zoomed((from, to), pointer, factor))
                        };
                        let fits = |next: &(f64, f64)| {
                            next.0.is_finite() && next.1.is_finite() && next.1 > next.0
                        };
                        if let Some(next) = next.filter(fits) {
                            on_range(next, window, cx);
                        }
                    })
                    .children(self.tracks.iter().enumerate().map(|(track, name)| {
                        let bars = self
                            .spans
                            .iter()
                            .enumerate()
                            .filter(|(_, span)| {
                                span.track == track && span.end > from && span.start < to
                            })
                            .map(|(ix, span)| {
                                let (left, right) =
                                    (at(span.start).max(0.0), at(span.end).min(1.0));
                                let lit = self.selected == Some(ix);
                                let pick = self.on_select.clone();
                                let color = tint(&span.name, &colors.chart);
                                div()
                                    .id((self.id.clone(), format!("span-{ix}")))
                                    .absolute()
                                    .top_0p5()
                                    .bottom_0p5()
                                    .left(relative(left))
                                    .w(relative(right - left))
                                    .px_1()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .rounded(theme.radius(Radius::Sm))
                                    .bg(if lit { color } else { color.opacity(0.55) })
                                    .border_1()
                                    .border_color(if lit {
                                        colors.accent
                                    } else {
                                        color.opacity(0.0)
                                    })
                                    .text_color(colors.fg)
                                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                                        window.prevent_default()
                                    })
                                    .when_some(pick, |bar, pick| {
                                        bar.tab_index(0)
                                            .focus_ring(cx)
                                            .cursor_pointer()
                                            .on_click(move |_, window, cx| pick(ix, window, cx))
                                    })
                                    .child(Ellipsis::new(span.name.clone()))
                            });
                        div()
                            .flex()
                            .h(row)
                            .child(
                                div()
                                    .flex_none()
                                    .w(label)
                                    .pr_2()
                                    .text_color(colors.fg_muted)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .h_full()
                                    .overflow_hidden()
                                    .bg(colors.hover.opacity(0.5))
                                    .when(track == 0, |row| {
                                        row.child(measure(lane.clone(), |bounds| bounds))
                                    })
                                    .children(bars),
                            )
                    })),
            )
            .children(selected.map(|span| {
                div().text_color(colors.fg_muted).child(format!(
                    "{} · {:.1} ms · from {:.1} to {:.1} ms",
                    span.name,
                    span.end - span.start,
                    span.start,
                    span.end
                ))
            }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ticks_keep_the_places_their_step_needs() {
        assert_eq!(super::tick_label(0.2, 0.2), "0.2 ms");
        assert_eq!(super::tick_label(15.0, 5.0), "15 ms");
    }

    use super::*;

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        assert_eq!(zoomed((0.0, 100.0), 0.5, 0.5), (25.0, 75.0));
        assert_eq!(
            zoomed((0.0, 100.0), 0.0, 0.5),
            (0.0, 50.0),
            "zooming at the start keeps the start"
        );
        assert_eq!(zoomed((10.0, 20.0), 1.0, 2.0), (0.0, 20.0));
    }

    #[test]
    fn the_ruler_keeps_a_label_of_room_between_ticks() {
        let narrow = ruler((0.0, 4_400.0), 184.0, 64.0);
        assert_eq!(
            narrow,
            (2_000.0, vec![0.0, 2_000.0]),
            "4000 has no room before the end"
        );
        let wide = ruler((0.0, 4_600.0), 664.0, 64.0).1;
        assert_eq!(wide, [0.0, 1_000.0, 2_000.0, 3_000.0, 4_000.0]);
        assert!(
            ruler((0.0, 4_400.0), 0.0, 64.0).1.is_empty(),
            "an unmeasured lane holds none"
        );
    }
}
