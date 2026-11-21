use std::{collections::VecDeque, fmt, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, DispatchPhase, ElementId, InteractiveElement, IntoElement, KeyContext,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    Pixels, Point, RenderOnce, ScrollDelta, ScrollEvent, StatefulInteractiveElement, Styled,
    Window, canvas, div, prelude::FluentBuilder,
};
use smallvec::SmallVec;
use web_time::Instant;

use crate::{
    layout::on_axis,
    theme::{ActiveTheme, TextSize},
    typography::{Ellipsis, tabular},
};

/// Events a logger keeps.
const KEPT: usize = 50;

/// An input event and what it carried; pointer places are in the logger's box.
#[derive(Clone, Debug)]
enum Seen {
    Press(MouseButton, usize, Point<Pixels>),
    Release(MouseButton, Point<Pixels>),
    /// Where the pointer is, and how many moves ran together to get there.
    Move(Point<Pixels>, usize),
    Wheel(ScrollDelta),
    KeyDown(String),
    KeyUp(String),
    Modifiers(Modifiers),
}

fn at(point: Point<Pixels>) -> String {
    format!("{:.0}, {:.0}", f32::from(point.x), f32::from(point.y))
}

fn button(button: MouseButton) -> String {
    format!("{button:?}").to_lowercase()
}

/// The key gpui makes of a modifier pressed and released alone; the log shows it as modifiers.
fn lone_modifier(keystroke: &Keystroke) -> bool {
    let names = ["shift", "control", "alt", "platform", "function"];
    !keystroke.modifiers.modified() && names.contains(&keystroke.key.as_str())
}

impl fmt::Display for Seen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seen::Press(which, 1, place) => write!(f, "press {} at {}", button(*which), at(*place)),
            Seen::Press(which, clicks, place) => {
                write!(f, "press {} ×{clicks} at {}", button(*which), at(*place))
            }
            Seen::Release(which, place) => {
                write!(f, "release {} at {}", button(*which), at(*place))
            }
            Seen::Move(place, 1) => write!(f, "move to {}", at(*place)),
            Seen::Move(place, moves) => write!(f, "move to {} ×{moves}", at(*place)),
            Seen::Wheel(ScrollDelta::Pixels(delta)) => write!(f, "wheel {} px", at(*delta)),
            Seen::Wheel(ScrollDelta::Lines(delta)) => {
                write!(f, "wheel {}, {} lines", delta.x, delta.y)
            }
            Seen::KeyDown(key) => write!(f, "key down {key}"),
            Seen::KeyUp(key) => write!(f, "key up {key}"),
            Seen::Modifiers(held) => {
                let names = [
                    (held.control, "ctrl"),
                    (held.alt, "alt"),
                    (held.shift, "shift"),
                    (held.platform, "cmd"),
                    (held.function, "fn"),
                ]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect::<Vec<_>>();
                match names.is_empty() {
                    true => write!(f, "modifiers none"),
                    false => write!(f, "modifiers {}", names.join(" ")),
                }
            }
        }
    }
}

/// The events seen, oldest first, each timed from the first.
#[derive(Default)]
struct Log {
    start: Option<Instant>,
    entries: VecDeque<(Duration, Seen)>,
}

impl Log {
    /// Adds an event; a move after a move joins it.
    fn push(&mut self, now: Instant, seen: Seen) {
        let start = *self.start.get_or_insert(now);
        let when = now.duration_since(start);
        if let (Seen::Move(place, _), Some((last, Seen::Move(was, moves)))) =
            (&seen, self.entries.back_mut())
        {
            (*last, *was, *moves) = (when, *place, *moves + 1);
            return;
        }
        self.entries.push_back((when, seen));
        if self.entries.len() > KEPT {
            self.entries.pop_front();
        }
    }
}

type Record = Rc<dyn Fn(Seen, &mut App)>;

/// The input events that reach what it holds, newest first below it: presses, releases, moves run together while the pointer travels, wheels, keys and modifiers, each timed from the first. Pointer events count inside its box, even where a child stops them; keys count while focus is inside, a key a binding takes too. It keeps the last fifty.
#[derive(IntoElement)]
pub struct EventLogger {
    id: ElementId,
    children: SmallVec<[AnyElement; 2]>,
}

impl EventLogger {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: SmallVec::new(),
        }
    }
}

impl ParentElement for EventLogger {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for EventLogger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id;
        let log = window.use_keyed_state((id.clone(), "log"), cx, |_, _| Log::default());
        let entries: Vec<(Duration, Seen)> = log.read(cx).entries.iter().rev().cloned().collect();
        let record: Record = Rc::new({
            let id = id.clone();
            move |seen, cx| {
                if !matches!(seen, Seen::Move(..)) {
                    log::debug!("event logger {id}: {seen}");
                }
                let now = cx.background_executor().now();
                log.update(cx, |log, cx| {
                    log.push(now, seen);
                    cx.notify();
                });
            }
        });
        // Bindings run before key listeners, so key downs come through an interceptor.
        let mark = id.to_string();
        window.use_keyed_state((id.clone(), "keys"), cx, {
            let (down, mark) = (record.clone(), mark.clone());
            move |_, cx| {
                cx.intercept_keystrokes(move |event, _, cx| {
                    let inside = event
                        .context_stack
                        .iter()
                        .any(|context| context.get("logger").is_some_and(|logger| *logger == mark));
                    if inside && !lone_modifier(&event.keystroke) {
                        down(
                            Seen::KeyDown(event.keystroke.unparse(cx.platform_info().os)),
                            cx,
                        )
                    }
                })
            }
        });
        let mut context = KeyContext::default();
        context.set("logger", mark);
        let theme = cx.theme();
        let colors = &theme.colors;
        let (up, held) = (record.clone(), record.clone());
        let watched = div()
            .relative()
            .key_context(context)
            .capture_key_up(move |event, _, cx| {
                up(
                    Seen::KeyUp(event.keystroke.unparse(cx.platform_info().os)),
                    cx,
                )
            })
            .on_modifiers_changed(move |event, _, cx| held(Seen::Modifiers(event.modifiers), cx))
            // Laid first, so its capture listeners run before any child's.
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let inside = move |place: Point<Pixels>, phase: DispatchPhase| {
                            (phase == DispatchPhase::Capture && bounds.contains(&place))
                                .then(|| place - bounds.origin)
                        };
                        let pressed = record.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                            if let Some(place) = inside(event.position, phase) {
                                pressed(Seen::Press(event.button, event.click_count, place), cx)
                            }
                        });
                        let released = record.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if let Some(place) = inside(event.position, phase) {
                                released(Seen::Release(event.button, place), cx)
                            }
                        });
                        let moved = record.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if let Some(place) = inside(event.position, phase) {
                                moved(Seen::Move(place, 1), cx)
                            }
                        });
                        let wheeled = record.clone();
                        window.on_mouse_event(move |event: &ScrollEvent, phase, _, cx| {
                            if inside(event.position, phase).is_some() {
                                wheeled(Seen::Wheel(event.delta), cx)
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .children(self.children);
        let rows = entries.into_iter().map(|(when, seen)| {
            let text = seen.to_string();
            div()
                .debug_selector({
                    let text = text.clone();
                    move || format!("event-{text}")
                })
                .flex()
                .items_center()
                .gap_3()
                .child(
                    tabular(div())
                        .flex_none()
                        .text_color(colors.fg_muted)
                        .child(format!("+{:.2} s", when.as_secs_f64())),
                )
                .child(div().flex_1().min_w_0().child(Ellipsis::new(text)))
        });
        let empty = rows.len() == 0;
        div().flex().flex_col().gap_3().child(watched).child(
            on_axis(
                div()
                    .id((id, "log"))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(theme.tooling().log)
                    .overflow_y_scroll()
                    .text_size(theme.text_size(TextSize::Xs)),
            )
            .when(empty, |log| {
                log.child(
                    div()
                        .debug_selector(|| "event-logger-empty".into())
                        .text_color(colors.fg_muted)
                        .child("Events inside the box show here."),
                )
            })
            .children(rows),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gpui::{Modifiers, MouseButton, point, px};
    use web_time::Instant;

    use super::{KEPT, Log, Seen};

    #[test]
    fn moves_run_together_and_the_log_keeps_fifty() {
        let (start, mut log) = (Instant::now(), Log::default());
        log.push(start, Seen::Move(point(px(1.0), px(2.0)), 1));
        log.push(
            start + Duration::from_millis(250),
            Seen::Move(point(px(5.0), px(6.0)), 1),
        );
        let (when, seen) = log.entries.back().expect("an entry");
        assert_eq!(log.entries.len(), 1);
        assert_eq!(*when, Duration::from_millis(250));
        assert_eq!(seen.to_string(), "move to 5, 6 ×2");
        for _ in 0..KEPT {
            log.push(
                start,
                Seen::Press(MouseButton::Left, 2, point(px(0.0), px(0.0))),
            );
        }
        assert_eq!(log.entries.len(), KEPT);
        assert_eq!(
            log.entries.front().expect("an entry").1.to_string(),
            "press left ×2 at 0, 0"
        );
        let held = Modifiers {
            shift: true,
            platform: true,
            ..Modifiers::default()
        };
        assert_eq!(Seen::Modifiers(held).to_string(), "modifiers shift cmd");
        assert_eq!(
            Seen::Modifiers(Modifiers::default()).to_string(),
            "modifiers none"
        );
    }
}
