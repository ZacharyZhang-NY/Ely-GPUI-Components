use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement, IntoElement, KeyUpEvent, Keystroke,
    ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div, px,
};

use super::setup;
use crate::{
    buttons::Button,
    forms::{
        Choice, DatePicker, EmailInput, FieldArray, Form, FormField, Input, InputEvent, InputGroup,
        MentionInput, NumberInput, Select, Slider, TagInput, TextInput,
    },
    layout::tests::narrow_width,
};

struct Signup {
    name: Entity<TextInput>,
    sent: usize,
    field_submits: usize,
}

impl Render for Signup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        Form::new("signup")
            .on_submit(move |_, cx| {
                view.update(cx, |view, cx| {
                    view.sent += 1;
                    cx.notify();
                })
            })
            .child(
                FormField::new("signup-name", "Name")
                    .required()
                    .child(Input::new(&self.name)),
            )
            .child(Button::new("signup-save", "Save"))
    }
}

fn signup(cx: &mut TestAppContext) -> (Entity<Signup>, &mut VisualTestContext) {
    setup(cx);
    cx.add_window_view(|window, cx| {
        let name = cx.new(|cx| TextInput::new(window, cx));
        cx.subscribe(&name, |view: &mut Signup, _, event, _| {
            if *event == InputEvent::Submit {
                view.field_submits += 1;
            }
        })
        .detach();
        Signup {
            name,
            sent: 0,
            field_submits: 0,
        }
    })
}

#[gpui::test]
fn cmd_enter_in_a_field_submits_the_form_once(cx: &mut TestAppContext) {
    let (view, cx) = signup(cx);
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_input("Ada");
    cx.simulate_keystrokes("secondary-enter");
    let counts = view.read_with(cx, |view, _| (view.sent, view.field_submits));
    assert_eq!(counts, (1, 0));
}

#[gpui::test]
fn cmd_enter_on_any_control_in_a_form_submits_it(cx: &mut TestAppContext) {
    let (view, cx) = signup(cx);
    cx.update(|window, cx| {
        window.focus_next(cx);
        window.focus_next(cx);
    });
    cx.simulate_keystrokes("secondary-enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.sent),
        1,
        "the button holds focus"
    );
}

struct Rows {
    rows: Vec<&'static str>,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (grow, shrink) = (cx.entity(), cx.entity());
        self.rows.iter().fold(
            FieldArray::new("rows")
                .on_add(move |_, cx| {
                    grow.update(cx, |view, cx| {
                        view.rows.push("new");
                        cx.notify();
                    })
                })
                .on_remove(move |ix, _, cx| {
                    shrink.update(cx, |view, cx| {
                        view.rows.remove(ix);
                        cx.notify();
                    })
                }),
            |array, row| array.row(div().child(*row)),
        )
    }
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os)).unwrap(),
    });
}

#[gpui::test]
fn field_array_buttons_remove_their_own_row_and_add_one(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Rows {
        rows: vec!["a", "b", "c"],
    });
    cx.update(|window, cx| {
        window.focus_next(cx);
        window.focus_next(cx);
    });
    press("space", cx);
    assert_eq!(view.read_with(cx, |view, _| view.rows.clone()), ["a", "c"]);
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..3 {
            window.focus_next(cx);
        }
    });
    press("space", cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.rows.clone()),
        ["a", "c", "new"]
    );
}

/// A field over a long line in a column that a padded block measures by content.
struct Narrow;

impl Render for Narrow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let control = div().debug_selector(|| "narrow-control".into()).h_2();
        div().w(px(280.)).flex().flex_col().child(
            div().p_5().child(
                div()
                    .flex()
                    .flex_col()
                    .child(FormField::new("narrow", "Name").child(control))
                    .child("A line long enough to wrap in a narrow card, so the column fills it."),
            ),
        )
    }
}

#[gpui::test]
fn a_field_fills_a_column_its_block_measures_by_content(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Narrow);
    cx.run_until_parked();
    let control = cx
        .debug_bounds("narrow-control")
        .expect("the control draws");
    assert_eq!(
        control.size.width,
        px(240.),
        "the field spans the card inside its padding"
    );
}

/// A field in a card of a set width, inside its padding.
struct Padded;

impl Render for Padded {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let control = div().debug_selector(|| "padded-control".into()).h_2();
        div()
            .w(px(400.))
            .p_5()
            .child(FormField::new("padded", "Name").child(control))
    }
}

#[gpui::test]
fn a_field_keeps_inside_its_cards_padding(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Padded);
    cx.run_until_parked();
    let control = cx
        .debug_bounds("padded-control")
        .expect("the control draws");
    assert_eq!(
        control.size.width,
        px(360.),
        "a percent min would take the card's outer width"
    );
}

#[gpui::test]
fn fields_fill_a_column_their_block_measures_by_content(cx: &mut TestAppContext) {
    setup(cx);
    let field =
        |window: &mut Window, cx: &mut App| window.use_keyed_state("field", cx, TextInput::new);
    let widths = [
        narrow_width(cx, "field-root", |_, _| {
            Select::new("select", [Choice::new("a", "A")]).into_any_element()
        }),
        narrow_width(cx, "field-root", |_, _| {
            DatePicker::new("date", None).into_any_element()
        }),
        narrow_width(cx, "checked-root", move |window, cx| {
            EmailInput::new(&field(window, cx)).into_any_element()
        }),
        narrow_width(cx, "mention-root", move |window, cx| {
            MentionInput::new("mention", &field(window, cx)).into_any_element()
        }),
        narrow_width(cx, "slider-root", |_, _| {
            Slider::new("slider", 0.5).into_any_element()
        }),
        narrow_width(cx, "number-root", |_, _| {
            NumberInput::new("number", 42.0).into_any_element()
        }),
        narrow_width(cx, "input-root", move |window, cx| {
            Input::new(&field(window, cx)).into_any_element()
        }),
        narrow_width(cx, "group-root", move |window, cx| {
            InputGroup::new(&field(window, cx)).into_any_element()
        }),
        narrow_width(cx, "tag-input tags", |_, _| {
            TagInput::new("tags", ["a"]).into_any_element()
        }),
    ];
    assert_eq!(
        widths,
        [px(240.0); 9],
        "each field spans the card inside its padding"
    );
}
