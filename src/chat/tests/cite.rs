use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, canvas, div, px,
};

use super::{Measured, settle};
use crate::{
    chat::{CitationBadge, Source, SourceCard, SourceList, WebResultCard},
    forms,
    theme::Theme,
};

fn source(title: &str) -> Source {
    Source {
        site: "example.com".into(),
        title: title.to_string().into(),
        url: "https://example.com".into(),
        snippet: None,
    }
}

/// Sources folded under their count, the list's box measured, and how often one opened.
struct Cited {
    frame: Measured,
    opened: usize,
}

impl Render for Cited {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (frame, me) = (self.frame.clone(), cx.entity());
        div()
            .size_full()
            .child(
                CitationBadge::new("cite", 1, source("Gamma"))
                    .on_open(move |_, cx| me.update(cx, |cited, _| cited.opened += 1)),
            )
            .child(
                div()
                    .w(px(320.0))
                    .relative()
                    .child(SourceList::new(
                        "sources",
                        [source("Gamma"), source("Lift")],
                    ))
                    .child(
                        canvas(
                            move |bounds, _, _| *frame.borrow_mut() = Some(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
    }
}

fn release(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("the key parses"),
    });
    settle(cx);
}

#[gpui::test]
fn citations_and_sources_open_from_the_keyboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let frame = Measured::default();
    let seen = frame.clone();
    let (cited, cx) = cx.add_window_view(move |_, _| Cited {
        frame: seen,
        opened: 0,
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    release("enter", cx);
    assert_eq!(
        cited.read_with(cx, |cited, _| cited.opened),
        1,
        "the badge opens its source"
    );
    let folded = frame.borrow().expect("the list is laid out").size.height;
    cx.update(|window, cx| window.focus_next(cx));
    release("space", cx);
    let open = frame.borrow().expect("the list is laid out").size.height;
    assert!(
        open > folded,
        "Tab and Space open the sources: {folded:?} to {open:?}"
    );
}

/// A source card and a web result, each to open, and which opened.
struct Cards {
    opened: Vec<&'static str>,
}

impl Render for Cards {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (card, result) = (cx.entity(), cx.entity());
        div()
            .size_full()
            .child(
                SourceCard::new("card", source("Gamma"))
                    .on_open(move |_, cx| card.update(cx, |cards, _| cards.opened.push("card"))),
            )
            .child(
                WebResultCard::new("result", source("Lift")).on_open(move |_, cx| {
                    result.update(cx, |cards, _| cards.opened.push("result"))
                }),
            )
    }
}

#[gpui::test]
fn cards_open_from_the_keyboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (cards, cx) = cx.add_window_view(|_, _| Cards { opened: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    release("enter", cx);
    cx.update(|window, cx| window.focus_next(cx));
    release("enter", cx);
    assert_eq!(
        cards.read_with(cx, |cards, _| cards.opened.clone()),
        ["card", "result"]
    );
}
