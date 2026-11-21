use gpui::{
    Context, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Render, TestAppContext,
    VisualTestContext, Window,
};

use super::setup;
use crate::{
    forms::{Upload, UploadList, UploadState},
    primitives::FocusNext,
};

/// Three uploads, one in each state, recording which buttons ran.
struct Shelf {
    log: Vec<String>,
}

impl Render for Shelf {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let note = |what: &'static str, cx: &mut Context<Self>| {
            let view = cx.entity();
            move |ix: usize, _: &mut Window, cx: &mut gpui::App| {
                view.update(cx, |shelf, _| shelf.log.push(format!("{what} {ix}")))
            }
        };
        let file = |name: &str, state| Upload {
            name: name.to_string().into(),
            bytes: 1_000,
            state,
        };
        UploadList::new(
            "uploads",
            [
                file("a", UploadState::Uploading(0.5)),
                file("b", UploadState::Failed("Too big".into())),
                file("c", UploadState::Done),
            ],
        )
        .on_cancel(note("cancel", cx))
        .on_retry(note("retry", cx))
        .on_remove(note("remove", cx))
    }
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    cx.run_until_parked();
}

#[gpui::test]
fn each_row_offers_what_its_state_allows(cx: &mut TestAppContext) {
    setup(cx);
    cx.update(|cx| cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]));
    let (view, cx) = cx.add_window_view(|_, _| Shelf { log: Vec::new() });
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    let log = view.read_with(cx, |shelf, _| shelf.log.clone());
    assert_eq!(log, ["cancel 0", "retry 1", "remove 2"]);
}

#[test]
#[should_panic(expected = "not 0..=1")]
fn an_upload_past_whole_is_refused() {
    let _ = UploadList::new(
        "uploads",
        [Upload {
            name: "a".into(),
            bytes: 1,
            state: UploadState::Uploading(1.5),
        }],
    );
}
