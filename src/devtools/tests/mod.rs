use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px,
};

use super::{Connection, ConnectionForm, ConnectionManager, Engine, Field, TableStructureEditor};
use crate::{editor::CodeEditor, forms, primitives::FocusNext, theme::Theme};

mod api;
mod formats;
mod tools;

type Part = fn(&Bench, &mut Window, &mut App, Entity<Bench>) -> AnyElement;

/// A view that shows one tool, keeps the words it heard, and holds what a test hands its part: an editor, and a choice among the part's cases.
struct Bench {
    part: Part,
    said: Vec<String>,
    editor: Option<Entity<CodeEditor>>,
    choice: usize,
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let part = (self.part)(self, window, cx, owner);
        div().w(px(640.0)).child(part)
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn bench(part: Part, cx: &mut TestAppContext) -> (Entity<Bench>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Bench {
        part,
        said: Vec::new(),
        editor: None,
        choice: 0,
    });
    settle(cx);
    (host, cx)
}

fn say(owner: &Entity<Bench>, words: String, cx: &mut gpui::App) {
    owner.update(cx, |bench, cx| {
        bench.said.push(words);
        cx.notify();
    });
}

fn said(host: &Entity<Bench>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |bench, _| bench.said.clone())
}

fn tab(stops: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        (0..stops).for_each(|_| window.focus_next(cx));
    });
    settle(cx);
}

fn tap(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn manager(_: &Bench, _: &mut Window, _: &mut App, owner: Entity<Bench>) -> AnyElement {
    let [opened, edited, picked] = [(); 3].map(|_| owner.clone());
    ConnectionManager::new(
        "connections",
        [
            Connection::new("local", "Local", Engine::Postgres),
            Connection::new("cache", "Cache", Engine::Redis),
        ],
    )
    .on_open(move |key, _, cx| say(&opened, format!("open {key}"), cx))
    .on_edit(move |key, _, cx| say(&edited, format!("edit {key}"), cx))
    .on_select(move |key, _, cx| say(&picked, format!("select {key}"), cx))
    .into_any_element()
}

/// Stops run: New, the list, then each row's Edit and Remove.
#[gpui::test]
fn enter_opens_a_connection_and_a_rows_button_leaves_the_row_alone(cx: &mut TestAppContext) {
    let (host, cx) = bench(manager, cx);
    tab(2, cx);
    tap("enter", cx);
    tab(3, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["open local", "edit local"]);
}

fn structure(_: &Bench, _: &mut Window, _: &mut App, owner: Entity<Bench>) -> AnyElement {
    TableStructureEditor::new(
        "structure",
        [Field::new("id", "int").primary()],
        move |fields, _, cx| {
            let names: Vec<String> = fields
                .iter()
                .map(|field| format!("{}:{}", field.name, field.ty))
                .collect();
            say(&owner, names.join(" "), cx)
        },
    )
    .into_any_element()
}

/// A row's stops: name, type, empty, key, default and remove; Add field comes after.
#[gpui::test]
fn add_field_appends_a_free_name(cx: &mut TestAppContext) {
    let (host, cx) = bench(structure, cx);
    tab(7, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["id:int field_1:text"]);
}

#[test]
fn an_address_names_the_host_or_the_file() {
    let mut local = Connection::new("local", "Local", Engine::Postgres);
    local.database = "shop".into();
    assert_eq!(local.address(), "localhost:5432/shop");
    assert_eq!(
        Connection::new("cache", "Cache", Engine::Redis).address(),
        "localhost:6379"
    );
    let mut file = Connection::new("file", "File", Engine::Sqlite);
    file.database = "app.db".into();
    assert_eq!(file.address(), "app.db");
}

fn form(bench: &Bench, _: &mut Window, _: &mut App, owner: Entity<Bench>) -> AnyElement {
    let connection = match bench.choice {
        0 => {
            let mut shop = Connection::new("shop", "Shop", Engine::Postgres);
            shop.database = "shop".into();
            shop
        }
        _ => Connection::new("cache", "Cache", Engine::Redis),
    };
    ConnectionForm::new("form", connection)
        .on_save(move |connection, _, cx| {
            say(
                &owner,
                format!("save {} {}", connection.name, connection.address()),
                cx,
            )
        })
        .into_any_element()
}

/// Save is the form's last stop, so a step back from nothing reaches it.
fn last(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        window.focus_prev(cx);
    });
    settle(cx);
}

#[gpui::test]
fn save_takes_the_form_and_a_new_connection_refills_it(cx: &mut TestAppContext) {
    let (host, cx) = bench(form, cx);
    last(cx);
    tap("space", cx);
    host.update(cx, |bench, cx| {
        bench.choice = 1;
        cx.notify();
    });
    settle(cx);
    last(cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        ["save Shop localhost:5432/shop", "save Cache localhost:6379"]
    );
}

/// Stops: the name, then the engine.
#[gpui::test]
fn another_engine_brings_its_port(cx: &mut TestAppContext) {
    let (host, cx) = bench(form, cx);
    tab(2, cx);
    for key in ["down", "down", "enter"] {
        tap(key, cx);
    }
    last(cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["save Shop localhost:3306/shop"]);
}

fn narrow(_: &Bench, _: &mut Window, _: &mut App, _: Entity<Bench>) -> AnyElement {
    div()
        .w(px(280.0))
        .child(TableStructureEditor::new(
            "structure",
            [
                Field::new("user_id", "timestamptz"),
                Field::new("total", "numeric"),
            ],
            |_, _, _| {},
        ))
        .into_any_element()
}

/// In 280px each field stacks on its own, its default under its name, and nothing runs past the box.
#[gpui::test]
fn a_narrow_editor_stacks_each_field_inside_its_box(cx: &mut TestAppContext) {
    let (_, cx) = bench(narrow, cx);
    let name = cx
        .debug_bounds("inline-edit structure-name-0")
        .expect("the first name");
    let default = cx
        .debug_bounds("inline-edit structure-default-0")
        .expect("the first default");
    assert!(
        default.top() > name.bottom(),
        "the default sits under the name"
    );
    assert!(
        default.right() <= px(280.0) && name.right() <= px(280.0),
        "inside the box: {default:?}"
    );
}

/// In 640px the fields line up as a table, the default beside the name.
#[gpui::test]
fn a_wide_editor_lines_the_fields_up_as_a_table(cx: &mut TestAppContext) {
    let (_, cx) = bench(structure, cx);
    let name = cx
        .debug_bounds("inline-edit structure-name-0")
        .expect("the first name");
    let default = cx
        .debug_bounds("inline-edit structure-default-0")
        .expect("the first default");
    assert_eq!(default.top(), name.top());
    assert!(default.left() > name.right());
}
