use std::{path::PathBuf, time::Duration};

use anyhow::{Context as _, Result};
use futures::future::{Either, select};
use gpui::{AsyncApp, Bounds, Keystroke, Pixels, Point, WindowHandle, point, px};

use crate::{
    capture::{number, popup_number, snapshot},
    probe::{Opened, Probes},
    shell::Gallery,
    step::Step,
    swipe::{BEGAN, CHANGED, ENDED, swipe},
};

const FRAME: Duration = Duration::from_millis(120);
/// How long a shot waits for each frame.
const SHOWN: Duration = Duration::from_secs(5);
/// How long an out-of-process file panel may take to open.
const PANEL: Duration = Duration::from_secs(10);
const DRAG_STEPS: u32 = 10;

#[derive(Clone, Copy)]
enum Mouse {
    Move,
    Down,
    Drag,
    Up,
    RightDown,
    RightUp,
}

/// Parks the pointer on empty sidebar space.
pub fn park(window: WindowHandle<Gallery>, cx: &mut AsyncApp) -> Result<()> {
    send(window, Mouse::Move, point(px(24.0), px(720.0)), cx)
}

fn opened(key: &str, cx: &mut AsyncApp) -> Result<gpui::AnyWindowHandle> {
    cx.update(|cx| Opened::get(key, cx))
        .with_context(|| format!("no window opened as {key}"))
}

pub async fn play(
    window: WindowHandle<Gallery>,
    steps: &[Step],
    path: impl Fn(&str) -> PathBuf,
    cx: &mut AsyncApp,
) -> Result<()> {
    let mut last = point(px(0.0), px(0.0));
    let os = cx.update(|app| app.platform_info().os);
    for step in steps {
        match *step {
            Step::Rest => park(window, cx)?,
            Step::Hover(key) => {
                let at = target(window, key, cx).await?;
                send(window, Mouse::Move, at, cx)?;
            }
            Step::HoverAt(key, x, y) => {
                last = target_bounds(window, key, cx).await?.origin + point(px(x), px(y));
                send(window, Mouse::Move, last, cx)?;
            }
            Step::Down(key) => {
                let at = target(window, key, cx).await?;
                send(window, Mouse::Move, at, cx)?;
                send(window, Mouse::Down, at, cx)?;
            }
            Step::Up(key) => {
                let at = target(window, key, cx).await?;
                send(window, Mouse::Up, at, cx)?;
            }
            Step::Click(key) => {
                let at = target(window, key, cx).await?;
                send(window, Mouse::Move, at, cx)?;
                send(window, Mouse::Down, at, cx)?;
                send(window, Mouse::Up, at, cx)?;
            }
            Step::ClickEnd(key) => {
                let bounds = target_bounds(window, key, cx).await?;
                let at = point(bounds.right() - px(12.0), bounds.center().y);
                send(window, Mouse::Move, at, cx)?;
                send(window, Mouse::Down, at, cx)?;
                send(window, Mouse::Up, at, cx)?;
            }
            Step::DownAt(key, x, y) => {
                last = target_bounds(window, key, cx).await?.origin + point(px(x), px(y));
                send(window, Mouse::Move, last, cx)?;
                send(window, Mouse::Down, last, cx)?;
            }
            Step::RightAt(key, x, y) => {
                let at = target_bounds(window, key, cx).await?.origin + point(px(x), px(y));
                send(window, Mouse::Move, at, cx)?;
                send(window, Mouse::RightDown, at, cx)?;
                send(window, Mouse::RightUp, at, cx)?;
            }
            Step::Swipe(key, across) => {
                let at = target_bounds(window, key, cx).await?.center();
                send(window, Mouse::Move, at, cx)?;
                swipe(window, at, 0.0, BEGAN, cx)?;
                for _ in 0..DRAG_STEPS {
                    swipe(window, at, across / DRAG_STEPS as f32, CHANGED, cx)?;
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                }
                swipe(window, at, 0.0, ENDED, cx)?;
            }
            Step::DragTo(key, x, y) => {
                let goal = target_bounds(window, key, cx).await?.origin + point(px(x), px(y));
                for step in 1..=DRAG_STEPS {
                    let t = step as f32 / DRAG_STEPS as f32;
                    let at = point(
                        last.x + (goal.x - last.x) * t,
                        last.y + (goal.y - last.y) * t,
                    );
                    // Each move twice: gpui_macos replays no posted drag.
                    for _ in 0..2 {
                        send(window, Mouse::Drag, at, cx)?;
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                    }
                }
                last = goal;
            }
            Step::UpAt(key, x, y) => {
                last = target_bounds(window, key, cx).await?.origin + point(px(x), px(y));
                send(window, Mouse::Up, last, cx)?;
            }
            Step::Key(stroke) => {
                let stroke =
                    Keystroke::parse(stroke).with_context(|| format!("bad keystroke {stroke}"))?;
                press(window.into(), stroke, cx)?;
            }
            Step::Type(text) => {
                for ch in text.chars() {
                    let key = match ch {
                        ' ' => "space".to_string(),
                        ch if ch.is_ascii_uppercase() => {
                            format!("shift-{}", ch.to_ascii_lowercase())
                        }
                        ch => ch.to_string(),
                    };
                    let stroke = Keystroke::parse_with_os(&key, os)
                        .with_context(|| format!("bad key {key}"))?;
                    press(window.into(), stroke, cx)?;
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                }
            }
            Step::Wait(ms) => {
                cx.background_executor()
                    .timer(Duration::from_millis(ms))
                    .await
            }
            Step::Shot(name) => {
                let file = path(name);
                shown(window.into(), cx).await?;
                snapshot(window.update(cx, |_, window, _| number(window))??, &file)?;
                log::info!("script: wrote {}", file.display());
            }
            Step::ShotWindow(key, name) => {
                let file = path(name);
                let handle = opened(key, cx)?;
                shown(handle, cx).await?;
                snapshot(handle.update(cx, |_, window, _| number(window))??, &file)?;
                log::info!("script: wrote {}", file.display());
            }
            Step::KeyWindow(key, stroke) => {
                let stroke =
                    Keystroke::parse(stroke).with_context(|| format!("bad keystroke {stroke}"))?;
                press(opened(key, cx)?, stroke, cx)?;
            }
            Step::CloseNative(key) => {
                let number = opened(key, cx)?.update(cx, |_, window, _| number(window))??;
                perform_close(number)?;
            }
            Step::ShotPopup(name) => {
                let file = path(name);
                snapshot(popup_number()?, &file)?;
                log::info!("script: wrote {}", file.display());
            }
            Step::NativeKey(key) => {
                let (text, code) = match key {
                    "escape" => ("\u{1b}", 53),
                    "5" => ("5", 23),
                    other => anyhow::bail!("native key {other} is not known"),
                };
                let number = window.update(cx, |_, window, _| number(window))??;
                post_key(number, true, text, code)?;
            }
            Step::CancelPanel => {
                let mut waited = Duration::ZERO;
                while !cancel_panel()? {
                    anyhow::ensure!(waited < PANEL, "no file panel became key in {PANEL:?}");
                    cx.background_executor().timer(FRAME).await;
                    waited += FRAME;
                }
                log::info!("script: file panel cancelled after {waited:?}");
            }
            Step::ExpectClosed(key) => {
                let handle = opened(key, cx)?;
                let open = cx.update(|cx| cx.windows().contains(&handle));
                anyhow::ensure!(!open, "window {key} is still open");
                cx.update(|cx| Opened::take(key, cx));
                log::info!("script: {key} closed");
            }
            Step::CloseWindow(key) => {
                let handle = opened(key, cx)?;
                handle.update(cx, |_, window, _| window.remove_window())?;
                cx.update(|cx| Opened::take(key, cx));
            }
        }
        cx.background_executor().timer(FRAME).await;
    }
    Ok(())
}

/// Waits three frames, until AppKit shows what the steps left.
pub async fn shown(window: gpui::AnyWindowHandle, cx: &mut AsyncApp) -> Result<()> {
    for _ in 0..3 {
        let (drawn, frame) = futures::channel::oneshot::channel();
        window.update(cx, |_, window, _| {
            window.on_next_frame(move |_, _| drawn.send(()).unwrap_or_default())
        })?;
        match select(frame, cx.background_executor().timer(SHOWN)).await {
            Either::Left((Ok(()), _)) => {}
            Either::Left((Err(_), _)) => anyhow::bail!("the window dropped its frame callback"),
            Either::Right(_) => {
                anyhow::bail!("the window drew no frame in {SHOWN:?}; is it hidden?")
            }
        }
    }
    Ok(())
}

/// Presses a key without holding the root view; Enter and Space also release through AppKit, since components pick on release.
fn press(window: gpui::AnyWindowHandle, stroke: Keystroke, cx: &mut AsyncApp) -> Result<()> {
    let release = match stroke.key.as_str() {
        _ if stroke.modifiers.modified() => None,
        "enter" => Some(("\r", 36)),
        "space" => Some((" ", 49)),
        _ => None,
    };
    let number = window.update(cx, |_, window, cx| {
        window.dispatch_keystroke(stroke, cx);
        number(window)
    })??;
    match release {
        Some((text, code)) => post_key(number, false, text, code),
        None => Ok(()),
    }
}

fn send(
    window: WindowHandle<Gallery>,
    kind: Mouse,
    at: Point<Pixels>,
    cx: &mut AsyncApp,
) -> Result<()> {
    let (height, number) = window.update(cx, |_, window, _| {
        (window.viewport_size().height, number(window))
    })?;
    post(kind, f64::from(at.x), f64::from(height - at.y), number?)
}

/// Queues a real mouse event on this app. No permission needed.
#[cfg(target_os = "macos")]
fn post(kind: Mouse, x: f64, y: f64, number: u32) -> Result<()> {
    use cocoa::{
        appkit::{NSApp, NSApplication, NSEvent, NSEventModifierFlags, NSEventType},
        base::{NO, id, nil},
        foundation::NSPoint,
    };
    let kind = match kind {
        Mouse::Move => NSEventType::NSMouseMoved,
        Mouse::Down => NSEventType::NSLeftMouseDown,
        Mouse::Drag => NSEventType::NSLeftMouseDragged,
        Mouse::Up => NSEventType::NSLeftMouseUp,
        Mouse::RightDown => NSEventType::NSRightMouseDown,
        Mouse::RightUp => NSEventType::NSRightMouseUp,
    };
    unsafe {
        let event = <id as NSEvent>::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure_(
            nil,
            kind,
            NSPoint::new(x, y),
            NSEventModifierFlags::empty(),
            0.0,
            i64::from(number),
            nil,
            0,
            1,
            1.0,
        );
        anyhow::ensure!(event != nil, "AppKit refused a synthetic mouse event");
        let right = matches!(
            kind,
            NSEventType::NSRightMouseDown | NSEventType::NSRightMouseUp
        );
        let event = if right { right_button(event) } else { event };
        anyhow::ensure!(event != nil, "AppKit refused a right-button event");
        NSApp().postEvent_atStart_(event, NO);
    }
    Ok(())
}

/// A built event reports button zero, and gpui reads the number, so a right click sets it.
#[cfg(target_os = "macos")]
unsafe fn right_button(event: cocoa::base::id) -> cocoa::base::id {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe extern "C" {
        fn CGEventSetIntegerValueField(event: *mut std::ffi::c_void, field: u32, value: i64);
    }
    const BUTTON_NUMBER: u32 = 3;
    unsafe {
        let cg: *mut std::ffi::c_void = msg_send![event, CGEvent];
        CGEventSetIntegerValueField(cg, BUTTON_NUMBER, 1);
        msg_send![class!(NSEvent), eventWithCGEvent: cg]
    }
}

/// Queues a real key event on a window of this app; AppKit popovers take Escape this way.
#[cfg(target_os = "macos")]
fn post_key(number: u32, down: bool, text: &str, code: u16) -> Result<()> {
    use cocoa::{
        appkit::{NSApp, NSApplication, NSEvent, NSEventModifierFlags, NSEventType},
        base::{NO, id, nil},
        foundation::{NSPoint, NSString},
    };
    let kind = if down {
        NSEventType::NSKeyDown
    } else {
        NSEventType::NSKeyUp
    };
    unsafe {
        let text = NSString::alloc(nil).init_str(text);
        let event = <id as NSEvent>::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode_(
            nil,
            kind,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            0.0,
            i64::from(number),
            nil,
            text,
            text,
            NO,
            code,
        );
        anyhow::ensure!(event != nil, "AppKit refused a synthetic key event");
        NSApp().postEvent_atStart_(event, NO);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn post_key(_: u32, _: bool, _: &str, _: u16) -> Result<()> {
    anyhow::bail!("native keys need macOS")
}

#[cfg(target_os = "macos")]
/// Cancels a key file panel; false while none is key.
fn cancel_panel() -> Result<bool> {
    use cocoa::{
        appkit::NSApp,
        base::{BOOL, NO, id, nil},
    };
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let key: id = msg_send![NSApp(), keyWindow];
        if key == nil {
            return Ok(false);
        }
        let panel: BOOL = msg_send![key, isKindOfClass: class!(NSSavePanel)];
        if panel == NO {
            return Ok(false);
        }
        let _: () = msg_send![key, cancel: nil];
    }
    Ok(true)
}

#[cfg(not(target_os = "macos"))]
fn cancel_panel() -> Result<bool> {
    anyhow::bail!("file panels need macOS")
}

/// Runs `performClose:` on a window, which asks before closing.
#[cfg(target_os = "macos")]
fn perform_close(number: u32) -> Result<()> {
    use cocoa::{
        appkit::NSApp,
        base::{id, nil},
    };
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let window: id = msg_send![NSApp(), windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(window != nil, "no AppKit window numbered {number}");
        let _: () = msg_send![window, performClose: nil];
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn perform_close(_: u32) -> Result<()> {
    anyhow::bail!("native close needs macOS")
}

#[cfg(not(target_os = "macos"))]
fn post(_: Mouse, _: f64, _: f64, _: u32) -> Result<()> {
    anyhow::bail!("scripted mouse input needs macOS")
}

async fn target(
    window: WindowHandle<Gallery>,
    key: &str,
    cx: &mut AsyncApp,
) -> Result<Point<Pixels>> {
    Ok(target_bounds(window, key, cx).await?.center())
}

async fn target_bounds(
    window: WindowHandle<Gallery>,
    key: &str,
    cx: &mut AsyncApp,
) -> Result<Bounds<Pixels>> {
    let bounds = cx
        .update(|cx| Probes::get(key, cx))
        .with_context(|| format!("probe {key} never rendered"))?;
    if window.update(cx, |gallery, _, cx| gallery.reveal(bounds, cx))? {
        cx.background_executor().timer(FRAME).await;
    }
    let bounds = cx
        .update(|cx| Probes::get(key, cx))
        .with_context(|| format!("probe {key} vanished after scrolling"))?;
    log::info!("script: probe {key} at {bounds:?}");
    Ok(bounds)
}
