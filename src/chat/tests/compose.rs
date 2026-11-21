use gpui::{
    AppContext as _, Context, Entity, ExternalPaths, FileDropEvent, Focusable, IntoElement,
    Modifiers, ParentElement, Render, SharedString, Styled, TestAppContext, VisualTestContext,
    Window, div, point, px,
};

use super::settle;
use crate::{
    chat::PromptInput,
    forms::{self, Choice, TextInput},
    motion,
    theme::{ActiveTheme, Theme},
};

/// A composer over a field, and what it asked: sends, commands and context.
struct Compose {
    field: Entity<TextInput>,
    busy: bool,
    commands: Vec<Choice>,
    sent: usize,
    picked: Vec<SharedString>,
}

impl Render for Compose {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (send, command, mention) = (cx.entity(), cx.entity(), cx.entity());
        let input = PromptInput::new("compose", &self.field, move |_, cx| {
            send.update(cx, |compose, _| compose.sent += 1)
        })
        .commands(self.commands.clone(), move |value, _, cx| {
            command.update(cx, |compose, _| compose.picked.push(value.clone()))
        })
        .context(
            [
                Choice::new("lift", "lift.rs"),
                Choice::new("src", "src/lift.rs"),
            ],
            move |value, _, cx| mention.update(cx, |compose, _| compose.picked.push(value.clone())),
        )
        .on_drop(|_, _, _| {});
        let input = if self.busy {
            input.busy(|_, _| {})
        } else {
            input
        };
        div().size_full().child(input)
    }
}

fn open(
    busy: bool,
    commands: Vec<Choice>,
    cx: &mut TestAppContext,
) -> (Entity<Compose>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
    });
    let (compose, cx) = cx.add_window_view(move |window, cx| Compose {
        field: cx.new(|cx| TextInput::new(window, cx).multi_line(1, 8)),
        busy,
        commands,
        sent: 0,
        picked: Vec::new(),
    });
    cx.update(|window, cx| {
        window.activate_window();
        let field = compose.read(cx).field.focus_handle(cx);
        window.focus(&field, cx);
    });
    settle(cx);
    (compose, cx)
}

fn summarize() -> Vec<Choice> {
    vec![Choice::new("summarize", "/summarize")]
}

/// Commands `c00` to `c19`, some disabled.
fn numbered(disabled: &[usize]) -> Vec<Choice> {
    (0..20)
        .map(|ix| {
            let choice = Choice::new(format!("c{ix:02}"), format!("/c{ix:02}"));
            if disabled.contains(&ix) {
                choice.disabled()
            } else {
                choice
            }
        })
        .collect()
}

/// Frames 2ms apart, past reduced motion's 1ms entrances.
fn frames(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn picked(compose: &Entity<Compose>, cx: &mut VisualTestContext) -> Vec<SharedString> {
    compose.read_with(cx, |compose, _| compose.picked.clone())
}

fn text(compose: &Entity<Compose>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| compose.read(cx).field.read(cx).text().to_string())
}

#[gpui::test]
fn enter_sends_and_shift_enter_breaks_the_line(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, summarize(), cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        compose.read_with(cx, |compose, _| compose.sent),
        0,
        "nothing to send"
    );
    cx.simulate_input("lift");
    cx.simulate_keystrokes("shift-enter");
    settle(cx);
    assert_eq!(text(&compose, cx), "lift\n");
    cx.simulate_input("tone");
    settle(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(compose.read_with(cx, |compose, _| compose.sent), 1);
    assert_eq!(
        text(&compose, cx),
        "lift\ntone",
        "Enter sends without a new line"
    );
}

#[gpui::test]
fn a_slash_picks_a_command_and_an_at_sign_picks_context(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, summarize(), cx);
    cx.simulate_input("/sum");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(text(&compose, cx), "", "the command's words go");
    cx.simulate_input("see @li");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(text(&compose, cx), "see ");
    compose.read_with(cx, |compose, _| {
        assert_eq!(compose.picked, ["summarize", "lift"]);
        assert_eq!(compose.sent, 0, "a pick is not a send");
    });
}

#[gpui::test]
fn a_busy_composer_holds_its_message(cx: &mut TestAppContext) {
    let (compose, cx) = open(true, summarize(), cx);
    cx.simulate_input("wait");
    settle(cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(compose.read_with(cx, |compose, _| compose.sent), 0);
    assert_eq!(text(&compose, cx), "wait");
}

#[gpui::test]
fn a_file_name_keeps_its_suggestions_past_a_dot(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, summarize(), cx);
    cx.simulate_input("see @lift.");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(text(&compose, cx), "see ");
    assert_eq!(picked(&compose, cx), ["lift"]);
    assert_eq!(compose.read_with(cx, |compose, _| compose.sent), 0);
}

#[gpui::test]
fn a_path_keeps_its_suggestions_past_a_slash(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, summarize(), cx);
    cx.simulate_input("see @src/");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(text(&compose, cx), "see ");
    assert_eq!(picked(&compose, cx), ["src"]);
    assert_eq!(compose.read_with(cx, |compose, _| compose.sent), 0);
}

/// Clicks near the foot of the command list that hangs under the slash.
fn click_list_foot(compose: &Entity<Compose>, cx: &mut VisualTestContext) {
    let (anchor, height) = cx.update(|window, cx| {
        let anchor = compose.read(cx).field.read(cx).bounds_for(0);
        let height = cx.theme().list_max_height().to_pixels(window.rem_size());
        (anchor.expect("the slash is laid out"), height)
    });
    let foot = point(
        anchor.left() + px(24.0),
        anchor.bottom() + motion::NUDGE + height - px(8.0),
    );
    cx.simulate_mouse_move(foot, None, Modifiers::none());
    cx.simulate_click(foot, Modifiers::none());
}

#[gpui::test]
fn the_list_opens_on_its_first_enabled_row_in_view(cx: &mut TestAppContext) {
    let disabled: Vec<usize> = (0..17).collect();
    let (compose, cx) = open(false, numbered(&disabled), cx);
    cx.simulate_input("/");
    frames(cx);
    click_list_foot(&compose, cx);
    assert_eq!(
        picked(&compose, cx),
        ["c17"],
        "the list opens scrolled to its cursor"
    );
}

#[gpui::test]
fn arrows_pass_disabled_rows_and_keep_the_cursor_in_view(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, numbered(&[1]), cx);
    cx.simulate_input("/");
    frames(cx);
    for _ in 0..16 {
        cx.simulate_keystrokes("down");
        frames(cx);
    }
    click_list_foot(&compose, cx);
    assert_eq!(
        picked(&compose, cx),
        ["c17"],
        "sixteen steps past one disabled row land on the eighteenth, at the list's foot"
    );
}

#[gpui::test]
fn the_cursor_starts_on_the_first_enabled_row(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, numbered(&[0]), cx);
    cx.simulate_input("/");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(picked(&compose, cx), ["c01"]);
}

#[gpui::test]
fn enter_on_only_disabled_rows_picks_and_sends_nothing(cx: &mut TestAppContext) {
    let (compose, cx) = open(false, vec![Choice::new("c00", "/c00").disabled()], cx);
    cx.simulate_input("/");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(picked(&compose, cx), Vec::<SharedString>::new());
    assert_eq!(compose.read_with(cx, |compose, _| compose.sent), 0);
    assert_eq!(text(&compose, cx), "/");
}

#[gpui::test]
fn files_over_the_composer_raise_the_veil(cx: &mut TestAppContext) {
    let (_, cx) = open(false, summarize(), cx);
    assert_eq!(
        cx.debug_bounds("drag-drop-overlay"),
        None,
        "no veil at rest"
    );
    cx.simulate_event(FileDropEvent::Entered {
        position: point(px(8.0), px(8.0)),
        paths: ExternalPaths::default(),
    });
    settle(cx);
    assert!(
        cx.debug_bounds("drag-drop-overlay").is_some(),
        "files over the composer raise the veil"
    );
}
