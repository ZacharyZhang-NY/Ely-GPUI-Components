use std::time::Duration;

use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Render, ScrollDelta,
    ScrollEvent, ScrollSource, SharedString, Styled, TestAppContext, TouchPhase, VisualTestContext,
    Window, div, point, px,
};
use web_time::Instant;

use super::{picture, press, settle, setup, tab_to};
use crate::media::{
    CameraPreview, Device, DeviceKind, DeviceSelector, ModelViewer, Orbit, Recording,
    ScreenRecorderControls,
};

/// A recorder, where it stands, whether it takes the mic, and what it was told.
struct Taping {
    state: Recording,
    mic: bool,
    heard: Vec<&'static str>,
}

impl Render for Taping {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hear = |what: &'static str, cx: &mut Context<Self>| {
            let owner = cx.entity();
            move |_: &mut Window, cx: &mut gpui::App| {
                owner.update(cx, |view, _| view.heard.push(what))
            }
        };
        let mic = cx.entity();
        div().child(
            ScreenRecorderControls::new("rec", self.state)
                .on_record(hear("record", cx))
                .on_pause(hear("pause", cx))
                .on_resume(hear("resume", cx))
                .on_stop(hear("stop", cx))
                .mic(self.mic, move |on, _, cx| {
                    mic.update(cx, |view, cx| {
                        view.mic = on;
                        cx.notify();
                    })
                }),
        )
    }
}

fn now_at(state: Recording, view: &Entity<Taping>, cx: &mut VisualTestContext) {
    view.update(cx, |view, cx| {
        view.state = state;
        cx.notify();
    });
    settle(cx);
}

#[gpui::test]
fn each_state_offers_its_own_buttons(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Taping {
        state: Recording::Idle,
        mic: true,
        heard: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(1, cx);
    press("space", cx);
    let since = Instant::now();
    now_at(Recording::Running { since }, &view, cx);
    for nth in [1, 2, 3] {
        tab_to(nth, cx);
        press("space", cx);
    }
    let at = since + Duration::from_secs(4);
    now_at(Recording::Paused { since, at }, &view, cx);
    tab_to(1, cx);
    press("space", cx);
    let heard = view.read_with(cx, |view, _| (view.heard.clone(), view.mic));
    assert_eq!(heard, (vec!["record", "pause", "stop", "resume"], false));
}

#[test]
#[should_panic(expected = "a pause before its recording started")]
fn a_pause_comes_after_its_start() {
    let at = Instant::now();
    let since = at + Duration::from_secs(1);
    let _ = ScreenRecorderControls::new("rec", Recording::Paused { since, at });
}

/// A preview this wide, with a frame or not, under a long name.
struct Watching {
    width: f32,
    frame: Option<std::path::PathBuf>,
}

impl Render for Watching {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let preview =
            CameraPreview::new("cam", 1.5).name("The camera over the long desk by the window");
        let preview = match self.frame.clone() {
            Some(path) => preview.frame(path),
            None => preview,
        };
        div().w(px(self.width)).child(preview)
    }
}

#[gpui::test]
fn with_no_frame_the_camera_shows_off(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Watching {
        width: 300.0,
        frame: None,
    });
    settle(cx);
    assert!(cx.debug_bounds("camera-off").is_some());
}

#[gpui::test]
fn a_framed_camera_names_itself_inside_a_narrow_box(cx: &mut TestAppContext) {
    setup(cx);
    let frame = Some(picture("camera", 150, 100));
    let (_, cx) = cx.add_window_view(|_, _| Watching {
        width: 200.0,
        frame,
    });
    settle(cx);
    assert!(cx.debug_bounds("camera-off").is_none(), "a frame shows");
    let (name, preview) = (
        cx.debug_bounds("camera-name").expect("the name draws"),
        cx.debug_bounds("camera-preview")
            .expect("the preview draws"),
    );
    assert!(name.right() <= preview.right(), "{name:?} in {preview:?}");
}

fn device(key: &str) -> Device {
    Device {
        key: SharedString::from(key.to_string()),
        name: SharedString::from(format!("Device {key}")),
    }
}

/// Two microphones, the first in use, and the ones it was told to use.
struct Choosing {
    devices: Vec<Device>,
    heard: Vec<SharedString>,
}

impl Render for Choosing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let selector = DeviceSelector::new("mics", DeviceKind::Microphone, self.devices.clone());
        let selector = match self.devices.first() {
            Some(first) => selector.chosen(first.key.clone()),
            None => selector,
        };
        div().w(px(280.0)).h(px(300.0)).child(
            selector.on_change(move |key, _, cx| {
                owner.update(cx, |view, _| view.heard.push(key.clone()))
            }),
        )
    }
}

#[gpui::test]
fn down_and_enter_choose_the_next_device(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Choosing {
        devices: vec![device("a"), device("b")],
        heard: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(1, cx);
    press("enter", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.heard.clone()), ["b"]);
}

#[gpui::test]
fn with_none_attached_the_selector_takes_no_focus(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Choosing {
        devices: Vec::new(),
        heard: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(1, cx);
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));
}

#[test]
#[should_panic(expected = "device a twice")]
fn a_device_is_listed_once() {
    let _ = DeviceSelector::new("mics", DeviceKind::Speaker, [device("a"), device("a")]);
}

/// A model viewer, its host's orbit, and every orbit it was told.
struct Orbiting {
    orbit: Orbit,
    heard: Vec<Orbit>,
    framed: bool,
}

impl Render for Orbiting {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let viewer = ModelViewer::new("model", 1.5, self.orbit).on_orbit(move |orbit, _, cx| {
            owner.update(cx, |view, cx| {
                view.orbit = orbit;
                view.heard.push(orbit);
                cx.notify();
            })
        });
        let viewer = match self.framed {
            true => viewer.frame(picture("model", 150, 100)),
            false => viewer,
        };
        div().w(px(480.0)).child(viewer)
    }
}

fn orbiting(framed: bool, cx: &mut TestAppContext) -> (Entity<Orbiting>, &mut VisualTestContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(move |_, _| Orbiting {
        orbit: Orbit::START,
        heard: Vec::new(),
        framed,
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (view, cx)
}

fn orbit(view: &Entity<Orbiting>, cx: &mut VisualTestContext) -> Orbit {
    view.read_with(cx, |view, _| view.orbit)
}

#[gpui::test]
fn a_drag_turns_the_view_by_its_whole_length(cx: &mut TestAppContext) {
    let (view, cx) = orbiting(true, cx);
    let middle = cx
        .debug_bounds("model-viewer")
        .expect("the viewer draws")
        .center();
    let none = Modifiers::none();
    cx.simulate_mouse_down(middle, MouseButton::Left, none);
    cx.simulate_mouse_move(middle + point(px(20.0), px(0.0)), MouseButton::Left, none);
    cx.simulate_mouse_move(middle + point(px(100.0), px(0.0)), MouseButton::Left, none);
    cx.simulate_mouse_up(middle + point(px(100.0), px(0.0)), MouseButton::Left, none);
    settle(cx);
    let now = orbit(&view, cx);
    assert!((now.yaw - (Orbit::START.yaw + 1.0)).abs() < 1e-4, "{now:?}");
    assert_eq!((now.pitch, now.distance), (Orbit::START.pitch, 1.0));
}

#[gpui::test]
fn keys_turn_bring_nearer_and_return_but_leave_command_keys(cx: &mut TestAppContext) {
    let (view, cx) = orbiting(true, cx);
    tab_to(1, cx);
    press("right", cx);
    assert!((orbit(&view, cx).yaw - (Orbit::START.yaw + std::f32::consts::PI / 12.0)).abs() < 1e-5);
    press("=", cx);
    assert!((orbit(&view, cx).distance - 0.8).abs() < 1e-5);
    press("cmd-=", cx);
    assert!(
        (orbit(&view, cx).distance - 0.8).abs() < 1e-5,
        "a command key passes"
    );
    press("0", cx);
    assert_eq!(orbit(&view, cx), Orbit::START);
}

#[gpui::test]
fn command_scroll_brings_it_nearer_and_a_plain_scroll_passes(cx: &mut TestAppContext) {
    let (view, cx) = orbiting(true, cx);
    let middle = cx
        .debug_bounds("model-viewer")
        .expect("the viewer draws")
        .center();
    let wheel = |platform: bool| ScrollEvent {
        position: middle,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-50.0))),
        modifiers: Modifiers {
            platform,
            ..Modifiers::none()
        },
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    };
    cx.simulate_event(wheel(false));
    settle(cx);
    assert_eq!(orbit(&view, cx), Orbit::START);
    cx.simulate_event(wheel(true));
    settle(cx);
    assert!((orbit(&view, cx).distance - (-0.5f32).exp()).abs() < 1e-4);
}

#[gpui::test]
fn until_its_first_frame_it_spins(cx: &mut TestAppContext) {
    let (_, cx) = orbiting(false, cx);
    assert!(cx.debug_bounds("model-loading").is_some());
}
