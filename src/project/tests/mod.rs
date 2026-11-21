use gpui::{
    AnyElement, Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers,
    ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div, px,
};

use gpui::SharedString;
use jiff::civil::date;
use web_time::Instant;

use super::{AssigneePicker, IssueId, Person, Priority, Status, StatusSelect, Task, TaskList};
use crate::{documents::Checklist, forms::bind_keys, primitives::FocusNext, theme::Theme};

mod boards;
mod time;

/// A view that shows one project part and keeps what it heard.
struct Desk {
    part: fn(&Desk, Entity<Desk>) -> AnyElement,
    heard: Vec<String>,
    clock: Option<(SharedString, Instant)>,
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(640.0)).child((self.part)(self, cx.entity()))
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn desk(
    part: fn(&Desk, Entity<Desk>) -> AnyElement,
    cx: &mut TestAppContext,
) -> (Entity<Desk>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Desk {
        part,
        heard: Vec::new(),
        clock: None,
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (host, cx)
}

fn note(owner: &Entity<Desk>, what: String, cx: &mut gpui::App) {
    owner.update(cx, |desk, cx| {
        desk.heard.push(what);
        cx.notify();
    });
}

fn heard(host: &Entity<Desk>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |desk, _| desk.heard.clone())
}

/// Moves focus to the `nth` Tab stop from none.
fn tab_to(nth: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..nth {
            window.focus_next(cx);
        }
    });
    settle(cx);
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn tasks() -> Vec<Task> {
    let ana = Person::new("ana", "Ana Lima");
    vec![
        Task::new("brief", "Write the brief")
            .issue(IssueId::new("ENG", 12))
            .priority(Priority::High)
            .due(date(2026, 9, 25))
            .assignee(ana),
        Task::new("specs", "Check the specs").status(Status::Done),
    ]
}

fn list(_: &Desk, owner: Entity<Desk>) -> AnyElement {
    let (opened, ticked) = (owner.clone(), owner);
    TaskList::new("tasks", tasks())
        .today(date(2026, 9, 27))
        .on_open(move |key, _, cx| note(&opened, format!("open {key}"), cx))
        .on_toggle(move |key, done, _, cx| note(&ticked, format!("tick {key} {done}"), cx))
        .into_any_element()
}

#[gpui::test]
fn a_box_ticks_its_task_and_leaves_the_row_alone(cx: &mut TestAppContext) {
    let (host, cx) = desk(list, cx);
    let tick = cx.debug_bounds("task-box specs").expect("the box draws");
    cx.simulate_click(tick.center(), Modifiers::none());
    settle(cx);
    tab_to(2, cx);
    press("enter", cx);
    tab_to(1, cx);
    press("enter", cx);
    assert_eq!(
        heard(&host, cx),
        ["tick specs false", "tick brief true", "open brief"],
        "a press on a box leaves the cursor, and Enter on a box does not open its row"
    );
}

#[gpui::test]
fn enter_opens_the_task_under_the_cursor(cx: &mut TestAppContext) {
    let (host, cx) = desk(list, cx);
    tab_to(1, cx);
    press("enter", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(heard(&host, cx), ["open brief", "open specs"]);
}

fn status(_: &Desk, owner: Entity<Desk>) -> AnyElement {
    StatusSelect::new("status", Status::Todo)
        .on_change(move |status, _, cx| note(&owner, status.words().into(), cx))
        .into_any_element()
}

#[gpui::test]
fn a_status_picked_by_key_hands_on_its_status(cx: &mut TestAppContext) {
    let (host, cx) = desk(status, cx);
    tab_to(1, cx);
    cx.simulate_keystrokes("down down enter");
    settle(cx);
    assert_eq!(heard(&host, cx), ["In progress"]);
}

fn assignee(_: &Desk, owner: Entity<Desk>) -> AnyElement {
    let people = [
        Person::new("ana", "Ana Lima"),
        Person::new("ben", "Ben Ito"),
    ];
    AssigneePicker::new("assignee", people)
        .assignee("ana")
        .on_change(move |person, _, cx| {
            let name = person.map_or("no one".into(), |person| person.name.to_string());
            note(&owner, name, cx)
        })
        .into_any_element()
}

#[gpui::test]
fn the_picker_hands_on_a_person_or_no_one(cx: &mut TestAppContext) {
    let (host, cx) = desk(assignee, cx);
    tab_to(1, cx);
    cx.simulate_keystrokes("down down enter");
    settle(cx);
    tab_to(1, cx);
    cx.simulate_keystrokes("down up enter");
    settle(cx);
    assert_eq!(heard(&host, cx), ["Ben Ito", "no one"]);
}

fn checklist(_: &Desk, owner: Entity<Desk>) -> AnyElement {
    Checklist::new(
        "subtasks",
        "Subtasks",
        [("Outline", true), ("Draft", false)],
    )
    .on_add("Add a subtask", move |words, _, cx| {
        note(&owner, format!("add {words}"), cx)
    })
    .into_any_element()
}

#[gpui::test]
fn a_checklist_adds_what_is_typed_and_clears(cx: &mut TestAppContext) {
    let (host, cx) = desk(checklist, cx);
    tab_to(3, cx);
    cx.simulate_input("  Review  ");
    press("enter", cx);
    press("enter", cx);
    assert_eq!(
        heard(&host, cx),
        ["add Review"],
        "the field clears, and empty adds nothing"
    );
}
