use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, ScrollDelta, SharedString, StatefulInteractiveElement, Styled, TouchPhase,
    Window, div, prelude::*,
};
use web_time::Instant;

use crate::{
    data_display::Tone,
    forms::Run,
    motion,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, TextSize},
};

/// One action behind a swiped row.
pub struct SwipeAction {
    label: SharedString,
    icon: IconName,
    tone: Tone,
    run: Run,
}

impl SwipeAction {
    pub fn new(
        label: impl Into<SharedString>,
        icon: IconName,
        run: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            icon,
            tone: Tone::Neutral,
            run: Rc::new(run),
        }
    }

    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }
}

/// Where the row rests or follows the fingers, whether they are down and on which axis, and the glide to rest.
#[derive(Default)]
struct Swipe {
    offset: Pixels,
    fingers: bool,
    across: Option<bool>,
    from: Pixels,
    since: Option<Instant>,
}

impl Swipe {
    fn now(&self, length: std::time::Duration) -> Pixels {
        let Some(since) = self.since else {
            return self.offset;
        };
        let share = (since.elapsed().as_secs_f32() / length.as_secs_f32()).min(1.0);
        self.from + (self.offset - self.from) * motion::ease_out_cubic(share)
    }

    fn settle(&mut self, to: Pixels, length: std::time::Duration) {
        self.from = self.now(length);
        self.offset = to;
        self.since = Some(Instant::now());
    }
}

/// Where a released row rests: open past half its reach, shut before.
fn rest(offset: Pixels, reach: Pixels) -> Pixels {
    if offset < reach * -0.5 {
        -reach
    } else {
        Pixels::ZERO
    }
}

/// A row that slides left under a two-finger swipe to show the actions behind it, and snaps open or shut when the fingers lift. A press on the open row, or on an action, shuts it.
#[derive(IntoElement)]
pub struct SwipeableListItem {
    id: ElementId,
    item: AnyElement,
    actions: Vec<SwipeAction>,
}

impl SwipeableListItem {
    pub fn new(id: impl Into<ElementId>, item: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            item: item.into_any_element(),
            actions: Vec::new(),
        }
    }

    pub fn action(mut self, action: SwipeAction) -> Self {
        self.actions.push(action);
        self
    }
}

impl RenderOnce for SwipeableListItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let swipe = window.use_keyed_state((self.id.clone(), "swipe"), cx, |_, _| Swipe::default());
        let length = motion::duration(motion::BASE, cx);
        let theme = cx.theme();
        let colors = &theme.colors;
        let wide = theme.swipe_action();
        let reach = wide.to_pixels(window.rem_size()) * self.actions.len() as f32;
        let (at, settling) = {
            let swipe = swipe.read(cx);
            let settling = swipe.since.is_some_and(|since| since.elapsed() < length);
            (swipe.now(length), settling)
        };
        if settling {
            window.request_animation_frame();
        }
        let shut = {
            let (swipe, id) = (swipe.clone(), self.id.clone());
            move |cx: &mut App| {
                swipe.update(cx, |swipe, cx| {
                    log::info!("swipeable row {id:?}: shut");
                    swipe.settle(Pixels::ZERO, length);
                    cx.notify();
                })
            }
        };
        let actions: Vec<_> = self
            .actions
            .into_iter()
            .enumerate()
            .map(|(ix, action)| {
                let (fill, ink) = action.tone.colors(colors);
                let (run, shut) = (action.run, shut.clone());
                div()
                    .id((self.id.clone(), format!("action-{ix}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_0p5()
                    .w(wide)
                    .h_full()
                    .bg(fill)
                    .text_color(ink)
                    .text_size(theme.text_size(TextSize::Xs))
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        shut(cx);
                        run(window, cx);
                    })
                    .child(Icon::new(action.icon).size(IconSize::Sm).color(ink))
                    .child(action.label)
            })
            .collect();
        let (fingers, pressed) = (swipe.clone(), shut);
        let id = self.id.clone();
        div()
            .id(self.id.clone())
            .relative()
            .overflow_hidden()
            .on_scroll(move |event, _, cx| {
                let ScrollDelta::Pixels(delta) = event.delta else {
                    return;
                };
                fingers.update(cx, |swipe, cx| {
                    match event.touch_phase {
                        TouchPhase::Started => {
                            swipe.offset = swipe.now(length);
                            swipe.since = None;
                            swipe.fingers = true;
                            swipe.across = None;
                        }
                        TouchPhase::Moved if swipe.fingers => {
                            if !*swipe.across.get_or_insert(delta.x.abs() > delta.y.abs()) {
                                return;
                            }
                            swipe.offset = (swipe.offset + delta.x).clamp(-reach, Pixels::ZERO);
                            cx.stop_propagation();
                        }
                        TouchPhase::Ended if swipe.fingers => {
                            swipe.fingers = false;
                            if swipe.across != Some(true) {
                                return;
                            }
                            let to = rest(swipe.offset, reach);
                            log::info!(
                                "swipeable row {id:?}: {}",
                                if to < Pixels::ZERO { "open" } else { "shut" }
                            );
                            swipe.settle(to, length);
                        }
                        _ => return,
                    }
                    cx.notify();
                })
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .flex()
                    .children(actions),
            )
            .child(
                div()
                    .id((self.id, "row"))
                    .relative()
                    .left(at)
                    .bg(colors.bg)
                    .block_mouse_except_scroll()
                    .when(at < Pixels::ZERO, |row| {
                        row.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            pressed(cx);
                        })
                    })
                    .child(self.item),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Pixels, px};

    use super::rest;

    #[test]
    fn a_released_row_rests_open_past_half_its_reach() {
        assert_eq!(rest(px(-50.0), px(144.0)), Pixels::ZERO);
        assert_eq!(rest(px(-80.0), px(144.0)), px(-144.0));
        assert_eq!(rest(Pixels::ZERO, px(144.0)), Pixels::ZERO);
    }
}
