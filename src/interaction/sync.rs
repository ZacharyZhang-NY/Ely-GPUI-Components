use gpui::{
    AnyElement, App, Axis, Div, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    Point, RenderOnce, ScrollHandle, StatefulInteractiveElement, StyleRefinement, Styled, Window,
    canvas, div, point, prelude::*,
};

use crate::layout::on_axis;

/// `to` with its place along `axis` taken from `from`.
fn along(to: Point<Pixels>, from: Point<Pixels>, axis: Axis) -> Point<Pixels> {
    match axis {
        Axis::Vertical => point(to.x, from.y),
        Axis::Horizontal => point(from.x, to.y),
    }
}

/// Boxes that scroll together along an axis: a wheel on one takes the others to its place before they draw. Side by side for a vertical axis, stacked for a horizontal one; the owner sizes them through the frame's style, and each fills its share.
#[derive(IntoElement)]
pub struct ScrollSync {
    id: ElementId,
    axis: Axis,
    base: Div,
    panes: Vec<AnyElement>,
}

impl ScrollSync {
    pub fn new(id: impl Into<ElementId>, axis: Axis) -> Self {
        Self {
            id: id.into(),
            axis,
            base: div(),
            panes: Vec::new(),
        }
    }

    pub fn pane(mut self, content: impl IntoElement) -> Self {
        self.panes.push(content.into_any_element());
        self
    }
}

impl Styled for ScrollSync {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for ScrollSync {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (id, axis) = (self.id, self.axis);
        assert!(self.panes.len() >= 2, "scroll sync {id:?} needs two panes");
        let handles: Vec<ScrollHandle> = (0..self.panes.len())
            .map(|ix| {
                window
                    .use_keyed_state((id.clone(), format!("pane-{ix}")), cx, |_, _| {
                        ScrollHandle::new()
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        let leader = window.use_keyed_state((id.clone(), "leader"), cx, |_, _| None::<usize>);
        let (lead, synced) = (*leader.read(cx), handles.clone());
        let sync = canvas(
            move |_, _, _| {
                let Some(lead) = lead else { return };
                let Some(place) = synced.get(lead).map(|handle| handle.offset()) else {
                    log::error!("scroll sync: leader pane {lead} of {} left", synced.len());
                    return;
                };
                for (ix, handle) in synced.iter().enumerate() {
                    let now = handle.offset();
                    let next = along(now, place, axis);
                    if ix != lead && next != now {
                        handle.set_offset(next);
                    }
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_0();
        let panes =
            self.panes
                .into_iter()
                .zip(handles)
                .enumerate()
                .map(|(ix, (content, handle))| {
                    let wheel = leader.clone();
                    let pane = div()
                        .id((id.clone(), format!("pane-{ix}")))
                        .flex_1()
                        .track_scroll(&handle)
                        .on_scroll(move |_, _, cx| {
                            if *wheel.read(cx) != Some(ix) {
                                log::info!("scroll sync: pane {ix} leads");
                                wheel.update(cx, |lead, _| *lead = Some(ix));
                            }
                        })
                        .child(content);
                    on_axis(match axis {
                        Axis::Vertical => pane.min_w_0().overflow_y_scroll(),
                        Axis::Horizontal => pane.min_h_0().overflow_x_scroll(),
                    })
                });
        self.base
            .id(id.clone())
            .relative()
            .flex()
            .when(axis == Axis::Horizontal, |frame| frame.flex_col())
            .child(sync)
            .children(panes)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Axis, point, px};

    use super::along;

    #[test]
    fn a_place_follows_only_along_the_axis() {
        let (mine, theirs) = (point(px(5.0), px(-20.0)), point(px(-40.0), px(-90.0)));
        assert_eq!(
            along(mine, theirs, Axis::Vertical),
            point(px(5.0), px(-90.0))
        );
        assert_eq!(
            along(mine, theirs, Axis::Horizontal),
            point(px(-40.0), px(-20.0))
        );
    }
}
