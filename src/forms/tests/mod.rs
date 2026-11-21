mod choices;
mod colors;
mod dates;
mod other;
mod popup;
mod stale;
mod structure;
mod tags;
mod upload;

use gpui::{
    AppContext as _, ClipboardItem, Context, Entity, EntityInputHandler, IntoElement, KeyUpEvent,
    Keystroke, OperatingSystem, ParentElement, PlatformInfo, PlatformTarget, Render,
    TestAppContext, VisualTestContext, Window, div, px,
};

use super::{Input, MaskedInput, NumberInput, PinInput, TextInput, bind_keys};
use crate::theme::Theme;

struct Fields {
    fields: Vec<Entity<TextInput>>,
    clearable: bool,
}

impl Render for Fields {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().children(self.fields.iter().map(|field| {
            let input = Input::new(field);
            if self.clearable {
                input.clearable()
            } else {
                input
            }
        }))
    }
}

pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        bind_keys(cx);
    });
}

fn open(
    count: usize,
    clearable: bool,
    cx: &mut TestAppContext,
) -> (Vec<Entity<TextInput>>, &mut VisualTestContext) {
    open_with(count, clearable, |input| input, cx)
}

fn open_with(
    count: usize,
    clearable: bool,
    build: impl Fn(TextInput) -> TextInput + 'static,
    cx: &mut TestAppContext,
) -> (Vec<Entity<TextInput>>, &mut VisualTestContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|window, cx| Fields {
        fields: (0..count)
            .map(|_| cx.new(|cx| build(TextInput::new(window, cx))))
            .collect(),
        clearable,
    });
    let fields = view.read_with(cx, |view, _| view.fields.clone());
    (fields, cx)
}

fn text(field: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
    field.read_with(cx, |input, _| input.text().to_string())
}

fn focus(field: &Entity<TextInput>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.focus(&field.read(cx).focus().clone(), cx));
}

#[gpui::test]
fn tab_reaches_fields_and_skips_disabled_ones(cx: &mut TestAppContext) {
    let (fields, cx) = open(3, false, cx);
    fields[1].update(cx, |input, cx| input.set_disabled(true, cx));
    cx.run_until_parked();
    let mut order = Vec::new();
    for _ in 0..2 {
        order.push(cx.update(|window, cx| {
            window.focus_next(cx);
            fields
                .iter()
                .position(|field| field.read(cx).focus().is_focused(window))
        }));
    }
    assert_eq!(order, [Some(0), Some(2)]);
}

#[gpui::test]
fn enter_on_a_clear_button_clears_its_own_field(cx: &mut TestAppContext) {
    let (fields, cx) = open(2, true, cx);
    for field in &fields {
        field.update(cx, |input, cx| input.set_text("kept", cx));
    }
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.focus_next(cx);
        window.focus_next(cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    assert_eq!([text(&fields[0], cx), text(&fields[1], cx)], ["", "kept"]);
}

#[gpui::test]
fn a_composition_undoes_in_one_step(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = &fields[0];
    focus(field, cx);
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "n", None, window, cx);
            input.replace_and_mark_text_in_range(None, "ni", None, window, cx);
            input.replace_text_in_range(None, "你", window, cx);
        })
    });
    assert_eq!(text(field, cx), "你");
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "");
}

#[gpui::test]
fn a_committed_composition_obeys_the_filter_and_length(cx: &mut TestAppContext) {
    let (fields, cx) = open_with(
        1,
        false,
        |input| input.filter(|ch| ch.is_ascii_digit()).max_len(1),
        cx,
    );
    let field = &fields[0];
    focus(field, cx);
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "12a", None, window, cx);
            input.unmark_text(window, cx);
        })
    });
    assert_eq!(text(field, cx), "1");
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "");
}

#[gpui::test]
fn a_browser_paste_undoes_apart_from_the_typing(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = &fields[0];
    focus(field, cx);
    cx.simulate_input("ab");
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            EntityInputHandler::paste(input, ClipboardItem::new_string("cd".into()), window, cx)
        })
    });
    assert_eq!(text(field, cx), "abcd");
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "ab");
}

#[gpui::test]
fn undo_leaves_a_disabled_field_alone(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = &fields[0];
    focus(field, cx);
    cx.simulate_input("a");
    field.update(cx, |input, cx| input.set_disabled(true, cx));
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "a");
}

struct Stepped {
    value: f64,
}

impl Render for Stepped {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        NumberInput::new("stepped", self.value).on_change(move |value, _, cx| {
            view.update(cx, |view, cx| {
                view.value = value;
                cx.notify();
            })
        })
    }
}

#[gpui::test]
fn stepping_starts_from_the_typed_number(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Stepped { value: 4.0 });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("20");
    cx.simulate_keystrokes("up");
    assert_eq!(view.read_with(cx, |view, _| view.value), 21.0);
}

#[gpui::test]
fn a_typed_number_reaches_the_owner_before_any_blur(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Stepped { value: 100.0 });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("250");
    assert_eq!(
        view.read_with(cx, |view, _| view.value),
        250.0,
        "a press elsewhere reads the typed number"
    );
    cx.simulate_input(".");
    assert_eq!(
        view.read_with(cx, |view, _| view.value),
        250.0,
        "a half-typed number waits"
    );
}

struct Committed {
    values: Vec<f64>,
}

impl Render for Committed {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        NumberInput::new("committed", 4.0)
            .on_commit(move |value, _, cx| view.update(cx, |view, _| view.values.push(value)))
    }
}

#[gpui::test]
fn typing_commits_only_on_enter_and_steps(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Committed { values: Vec::new() });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("12");
    assert!(
        view.read_with(cx, |view, _| view.values.is_empty()),
        "typing waits"
    );
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("up");
    assert_eq!(
        view.read_with(cx, |view, _| view.values.clone()),
        [12.0, 13.0]
    );
}

struct Blank {
    values: Vec<f64>,
}

impl Render for Blank {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        NumberInput::new("blank", 0.0)
            .blank("Mixed")
            .on_commit(move |value, _, cx| view.update(cx, |view, _| view.values.push(value)))
    }
}

#[gpui::test]
fn a_blank_field_commits_only_what_is_typed(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Blank { values: Vec::new() });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| window.focus_next(cx));
    assert!(
        view.read_with(cx, |view, _| view.values.is_empty()),
        "Enter and blur on an empty field commit nothing"
    );
    cx.update(|window, cx| window.focus_prev(cx));
    cx.simulate_input("0");
    cx.simulate_keystrokes("enter");
    assert_eq!(view.read_with(cx, |view, _| view.values.clone()), [0.0]);
}

#[gpui::test]
fn a_blur_commits_the_typed_number(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Committed { values: Vec::new() });
    cx.update(|window, cx| {
        window.activate_window();
        window.focus_next(cx);
    });
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("12");
    cx.update(|window, cx| window.blur(cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.values.clone()), [12.0]);
}

struct Pin {
    code: String,
}

impl Render for Pin {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        PinInput::new("pin", 4).on_complete(move |code, _, cx| {
            let code = code.to_string();
            view.update(cx, |view, _| view.code = code);
        })
    }
}

#[gpui::test]
fn select_all_then_typing_replaces_the_pin(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Pin {
        code: String::new(),
    });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_input("4821");
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("1234");
    assert_eq!(view.read_with(cx, |view, _| view.code.clone()), "1234");
}

struct Masked {
    state: Entity<TextInput>,
    mask: &'static str,
}

impl Render for Masked {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        MaskedInput::new("masked", &self.state, self.mask)
    }
}

fn masked<'a>(
    mask: &'static str,
    cx: &'a mut TestAppContext,
) -> (Entity<TextInput>, &'a mut VisualTestContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|window, cx| Masked {
        state: cx.new(|cx| TextInput::new(window, cx)),
        mask,
    });
    let state = view.read_with(cx, |view, _| view.state.clone());
    focus(&state, cx);
    (state, cx)
}

#[gpui::test]
fn select_all_then_typing_refits_a_masked_field(cx: &mut TestAppContext) {
    let (state, cx) = masked("1-999", cx);
    cx.simulate_input("123");
    assert_eq!(text(&state, cx), "1-123");
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("1");
    assert_eq!(text(&state, cx), "1-1");
}

#[gpui::test]
fn a_composition_inside_a_masked_field_fits_from_where_it_began(cx: &mut TestAppContext) {
    let (state, cx) = masked("(999) 999", cx);
    cx.simulate_input("123456");
    assert_eq!(text(&state, cx), "(123) 456");
    cx.update(|window, cx| {
        state.update(cx, |input, cx| {
            input.select(2..2, cx);
            input.replace_and_mark_text_in_range(None, "9", None, window, cx);
            input.replace_text_in_range(None, "9", window, cx);
        })
    });
    assert_eq!(text(&state, cx), "(192) 345");
}

#[gpui::test]
fn a_range_gives_a_box_per_line_it_spans(cx: &mut TestAppContext) {
    let (fields, cx) = open_with(1, false, |input| input.multi_line(2, 4), cx);
    fields[0].update(cx, |input, cx| input.set_text("lift\ntone", cx));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (one, two) = fields[0].read_with(cx, |input, _| {
        (input.bounds_for_range(1..3), input.bounds_for_range(2..7))
    });
    assert_eq!(one.len(), 1);
    assert!(one[0].size.width > px(0.0));
    assert_eq!(two.len(), 2, "{two:?}");
    assert!(two[1].top() > two[0].top());
    assert!(
        two[1].left() < two[0].left(),
        "the second line starts at the edge"
    );
}

#[gpui::test]
fn shift_enter_breaks_only_a_field_of_many_lines(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    focus(&fields[0], cx);
    cx.simulate_input("one line");
    cx.simulate_keystrokes("shift-enter");
    cx.run_until_parked();
    assert_eq!(
        text(&fields[0], cx),
        "one line",
        "a single line stays single"
    );
}

#[gpui::test]
fn only_a_focused_field_blinks(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = fields[0].clone();
    let redraws = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = redraws.clone();
    let _count = cx.update(|_, cx| cx.observe(&field, move |_, _| counted.set(counted.get() + 1)));
    field.update(cx, |input, cx| {
        input.set_text("seeded", cx);
        let word = 0..4;
        input.select(word, cx);
    });
    cx.run_until_parked();
    redraws.set(0);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(3));
    cx.run_until_parked();
    assert_eq!(redraws.get(), 0, "an unfocused field keeps still");
    cx.update(|window, _| window.activate_window());
    focus(&field, cx);
    cx.run_until_parked();
    redraws.set(0);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(3));
    cx.run_until_parked();
    assert!(redraws.get() > 2, "a focused one blinks");
}

#[gpui::test]
fn a_composition_shows_the_caret_as_typing_does(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = fields[0].clone();
    cx.update(|window, _| window.activate_window());
    focus(&field, cx);
    cx.run_until_parked();
    let caret = |cx: &mut VisualTestContext| field.read_with(cx, |input, _| input.caret_on());
    let blink_off = |cx: &mut VisualTestContext| {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(530));
        cx.run_until_parked();
    };
    blink_off(cx);
    assert!(!caret(cx), "the caret blinks off");
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_text_in_range(None, "a", window, cx)
        })
    });
    assert!(caret(cx), "typing shows the caret");
    blink_off(cx);
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "n", None, window, cx)
        })
    });
    assert!(caret(cx), "a composition shows it too");
}

#[gpui::test]
fn a_secondary_binding_follows_the_runtime_os(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = &fields[0];
    focus(field, cx);
    for (os, secondary, other) in [
        (OperatingSystem::Mac, "cmd-z", "ctrl-z"),
        (OperatingSystem::Linux, "ctrl-z", "cmd-z"),
        (OperatingSystem::Windows, "ctrl-z", "cmd-z"), // secondary = Control, like Linux
    ] {
        cx.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Native,
        });
        cx.update(|_, cx| {
            cx.clear_key_bindings();
            bind_keys(cx);
        });
        field.update(cx, |input, cx| input.set_text("", cx));
        cx.simulate_input("a");
        cx.simulate_keystrokes(other);
        assert_eq!(text(field, cx), "a", "{other} is not undo on {os:?}");
        cx.simulate_keystrokes(secondary);
        assert_eq!(text(field, cx), "", "{secondary} is undo on {os:?}");
    }
}
