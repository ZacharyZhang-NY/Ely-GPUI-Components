use gpui::{AnyElement, Entity, IntoElement, TestAppContext, VisualTestContext};

use super::{
    Stage, said, say, settle, stage,
    tools::{at, drag, layer, pair, press},
};
use crate::canvas::{
    Board, Edge, Link, MindMap, Node, NodeGraph, Port, Tool, Topic, Viewport, Whiteboard,
};

fn tap(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

#[gpui::test]
fn the_connector_links_the_shape_pressed_to_the_one_released_on(cx: &mut TestAppContext) {
    let (host, cx) = stage(|_, owner| layer(Tool::Connector, &[], owner), cx);
    drag((50.0, 50.0), (250.0, 60.0), cx);
    drag((50.0, 50.0), (150.0, 200.0), cx);
    assert_eq!(
        said(&host, cx),
        ["link a b"],
        "a release on nothing links nothing"
    );
}

#[gpui::test]
fn a_double_press_writes_on_a_shape_and_enter_keeps_the_words(cx: &mut TestAppContext) {
    let (host, cx) = stage(|_, owner| layer(Tool::Select, &["a"], owner), cx);
    press(50.0, 50.0, 1, cx);
    press(50.0, 50.0, 2, cx);
    cx.simulate_input("Start");
    tap("enter", cx);
    assert_eq!(said(&host, cx), ["text a Start"]);
}

fn nodes() -> Vec<Node> {
    vec![
        Node::new("load", "Load", (0.0, 0.0)).output(Port::new("image", "Image", "image")),
        Node::new("blur", "Blur", (300.0, 0.0))
            .input(Port::new("image", "Image", "image"))
            .output(Port::new("image", "Image", "image")),
    ]
}

fn graph(wired: bool, owner: Entity<Stage>) -> AnyElement {
    let (joined, parted) = (owner.clone(), owner);
    let edges = wired.then(|| Edge::new(("load", "image"), ("blur", "image")));
    NodeGraph::new("graph", Viewport::new(0.0, 0.0, 1.0), nodes(), edges)
        .on_connect(move |edge, _, cx| {
            say(
                &joined,
                format!(
                    "connect {}.{} {}.{}",
                    edge.from.0, edge.from.1, edge.to.0, edge.to.1
                ),
                cx,
            )
        })
        .on_disconnect(move |edge, _, cx| {
            say(
                &parted,
                format!("disconnect {}.{}", edge.to.0, edge.to.1),
                cx,
            )
        })
        .into_any_element()
}

/// Sockets sit on the node's sides a title and half a row down: 32 + 14.
#[gpui::test]
fn a_drag_from_an_output_wires_it_to_an_input(cx: &mut TestAppContext) {
    let (host, cx) = stage(|_, owner| graph(false, owner), cx);
    drag((200.0, 46.0), (301.0, 47.0), cx);
    assert_eq!(said(&host, cx), ["connect load.image blur.image"]);
}

#[gpui::test]
fn a_press_on_a_wired_input_lifts_its_wire(cx: &mut TestAppContext) {
    let (host, cx) = stage(|_, owner| graph(true, owner), cx);
    drag((300.0, 46.0), (250.0, 250.0), cx);
    assert_eq!(said(&host, cx), ["disconnect blur.image"]);
}

fn map(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let root = Topic::new("root", "Launch")
        .child(Topic::new("a", "Site"))
        .child(Topic::new("b", "Press"))
        .child(Topic::new("c", "Budget"));
    let [picked, added, removed, renamed] = [(); 4].map(|_| owner.clone());
    MindMap::new("map", root, Viewport::new(-60.0, -150.0, 1.0))
        .selected(Some("a"))
        .on_select(move |key, _, cx| say(&picked, format!("select {key:?}"), cx))
        .on_add(move |key, _, cx| say(&added, format!("add under {key}"), cx))
        .on_remove(move |key, _, cx| say(&removed, format!("remove {key}"), cx))
        .on_rename(move |key, text, _, cx| say(&renamed, format!("rename {key} {text}"), cx))
        .into_any_element()
}

#[gpui::test]
fn keys_on_a_mind_map_ask_for_children_siblings_and_removals(cx: &mut TestAppContext) {
    let (host, cx) = stage(map, cx);
    press(276.0, 126.0, 1, cx);
    for key in ["tab", "enter", "down", "backspace"] {
        tap(key, cx);
    }
    assert_eq!(
        said(&host, cx),
        [
            "select Some(\"a\")",
            "add under a",
            "add under root",
            "select Some(\"b\")",
            "remove a",
        ],
        "a press selects and focuses; the keys act on the owner's selection"
    );
}

fn board(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    Whiteboard::new(
        "board",
        Board {
            shapes: pair(),
            links: vec![Link::new("ab", "a", "b")],
        },
    )
    .on_change(move |board, _, cx| {
        let keys: Vec<String> = board
            .shapes
            .iter()
            .map(|shape| shape.key.to_string())
            .collect();
        say(
            &owner,
            format!("shapes {keys:?} links {}", board.links.len()),
            cx,
        )
    })
    .into_any_element()
}

#[gpui::test]
fn backspace_takes_the_selection_off_the_board(cx: &mut TestAppContext) {
    let (host, cx) = stage(board, cx);
    press(250.0, 50.0, 1, cx);
    tap("backspace", cx);
    assert_eq!(
        said(&host, cx),
        ["shapes [\"a\"] links 0"],
        "a link goes with its shape"
    );
}

#[gpui::test]
fn tab_in_a_topic_being_rewritten_keeps_the_words_and_adds_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(map, cx);
    press(276.0, 126.0, 1, cx);
    tap("f2", cx);
    cx.simulate_input("Web");
    tap("tab", cx);
    assert_eq!(said(&host, cx), ["select Some(\"a\")", "rename a Web"]);
}

fn three(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let (joined, parted) = (owner.clone(), owner);
    let gain = Node::new("gain", "Gain", (0.0, 150.0)).output(Port::new("image", "Image", "image"));
    let wired = Edge::new(("load", "image"), ("blur", "image"));
    let edge = |edge: &Edge| {
        format!(
            "{}.{} {}.{}",
            edge.from.0, edge.from.1, edge.to.0, edge.to.1
        )
    };
    NodeGraph::new(
        "graph",
        Viewport::new(0.0, 0.0, 1.0),
        nodes().into_iter().chain([gain]),
        [wired],
    )
    .on_connect(move |made, _, cx| say(&joined, format!("connect {}", edge(made)), cx))
    .on_disconnect(move |gone, _, cx| say(&parted, format!("disconnect {}", edge(gone)), cx))
    .into_any_element()
}

/// Gain's output sits on its right side, a title and half a row down: 150 + 32 + 14.
#[gpui::test]
fn a_new_wire_into_a_wired_input_replaces_the_old(cx: &mut TestAppContext) {
    let (host, cx) = stage(three, cx);
    drag((200.0, 196.0), (301.0, 47.0), cx);
    assert_eq!(
        said(&host, cx),
        [
            "disconnect load.image blur.image",
            "connect gain.image blur.image"
        ]
    );
}

#[gpui::test]
fn a_link_released_on_its_own_shape_links_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(|_, owner| layer(Tool::Connector, &[], owner), cx);
    drag((50.0, 50.0), (60.0, 70.0), cx);
    assert!(said(&host, cx).is_empty());
}

#[gpui::test]
fn enter_and_escape_in_a_topic_hand_the_keys_back_to_the_map(cx: &mut TestAppContext) {
    let (host, cx) = stage(map, cx);
    press(276.0, 126.0, 1, cx);
    tap("f2", cx);
    cx.simulate_input("Web");
    tap("enter", cx);
    tap("down", cx);
    tap("f2", cx);
    cx.simulate_input("Gone");
    tap("escape", cx);
    tap("down", cx);
    assert_eq!(
        said(&host, cx),
        [
            "select Some(\"a\")",
            "rename a Web",
            "select Some(\"b\")",
            "select Some(\"b\")",
        ],
        "the owner keeps a selected, so each Down steps from a"
    );
}

#[gpui::test]
fn after_writing_on_a_shape_its_board_still_takes_backspace(cx: &mut TestAppContext) {
    let (host, cx) = stage(board, cx);
    press(250.0, 50.0, 1, cx);
    press(250.0, 50.0, 2, cx);
    cx.simulate_input("Idea");
    tap("enter", cx);
    tap("backspace", cx);
    assert_eq!(
        said(&host, cx),
        ["shapes [\"a\", \"b\"] links 1", "shapes [\"a\"] links 0"]
    );
}

/// Presses at `from` and drags to `to` in four moves.
fn pull(from: (f32, f32), to: (f32, f32), cx: &mut VisualTestContext) {
    use gpui::{Modifiers, MouseButton};
    cx.simulate_mouse_move(at(from.0, from.1), None, Modifiers::none());
    cx.simulate_mouse_down(at(from.0, from.1), MouseButton::Left, Modifiers::none());
    for step in 1..=4 {
        let share = step as f32 / 4.0;
        let (x, y) = (
            from.0 + (to.0 - from.0) * share,
            from.1 + (to.1 - from.1) * share,
        );
        cx.simulate_mouse_move(at(x, y), Some(MouseButton::Left), Modifiers::none());
        settle(cx);
    }
}

fn release(to: (f32, f32), cx: &mut VisualTestContext) {
    let (left, none) = (gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(at(to.0, to.1), left, none);
    settle(cx);
}

/// The owner changes what it hands the part, mid-gesture.
fn change(host: &Entity<Stage>, cx: &mut VisualTestContext) {
    host.update(cx, |stage, cx| {
        stage.said.push("owner changed".into());
        cx.notify();
    });
    settle(cx);
}

#[gpui::test]
fn f2_on_a_selected_topic_that_left_renames_nothing(cx: &mut TestAppContext) {
    let (_, cx) = stage(
        |_, _| {
            let view = Viewport::new(-200.0, -150.0, 1.0);
            MindMap::new("map", Topic::new("root", "Root"), view)
                .selected(Some("gone"))
                .into_any_element()
        },
        cx,
    );
    press(200.0, 150.0, 1, cx);
    tap("f2", cx);
}

/// Two wireable nodes whose source leaves, or changes kind, once the owner changes.
fn shifting(stage: &Stage, owner: Entity<Stage>, retype: bool) -> AnyElement {
    let mut current = nodes();
    if !stage.said.is_empty() {
        match retype {
            true => current[0].outputs[0].kind = "number".into(),
            false => drop(current.remove(0)),
        }
    }
    NodeGraph::new("graph", Viewport::new(0.0, 0.0, 1.0), current, [])
        .on_connect(move |edge, _, cx| {
            let words = format!(
                "connect {}.{} {}.{}",
                edge.from.0, edge.from.1, edge.to.0, edge.to.1
            );
            say(&owner, words, cx)
        })
        .into_any_element()
}

#[gpui::test]
fn a_wire_whose_source_left_connects_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(|stage, owner| shifting(stage, owner, false), cx);
    pull((200.0, 46.0), (301.0, 47.0), cx);
    change(&host, cx);
    release((301.0, 47.0), cx);
    assert_eq!(said(&host, cx), ["owner changed"]);
}

#[gpui::test]
fn a_wire_whose_source_changed_kind_connects_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(|stage, owner| shifting(stage, owner, true), cx);
    pull((200.0, 46.0), (301.0, 47.0), cx);
    change(&host, cx);
    release((301.0, 47.0), cx);
    assert_eq!(said(&host, cx), ["owner changed"]);
}

#[gpui::test]
fn a_link_whose_shape_left_links_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(
        |stage, owner| {
            let mut shapes = pair();
            if !stage.said.is_empty() {
                shapes.remove(0);
            }
            let view = Viewport::new(0.0, 0.0, 1.0);
            let tool =
                crate::canvas::ToolLayer::new("layer", Tool::Connector, view, shapes.clone())
                    .on_link(move |from, to, _, cx| say(&owner, format!("link {from} {to}"), cx));
            crate::canvas::InfiniteCanvas::new("plane", view)
                .shapes(shapes)
                .layer(tool)
                .into_any_element()
        },
        cx,
    );
    pull((50.0, 50.0), (250.0, 60.0), cx);
    change(&host, cx);
    release((250.0, 60.0), cx);
    assert_eq!(said(&host, cx), ["owner changed"]);
}

#[gpui::test]
fn a_layer_that_left_mid_drag_restacks_nothing(cx: &mut TestAppContext) {
    let (host, cx) = stage(
        |stage, owner| {
            let mut shapes = pair();
            if !stage.said.is_empty() {
                shapes.remove(0);
            }
            crate::canvas::LayerPanel::new("layers", shapes)
                .on_restack(move |keys, _, cx| say(&owner, format!("restack {keys:?}"), cx))
                .into_any_element()
        },
        cx,
    );
    let a = cx.debug_bounds("tree-chevron-a").expect("a row").center();
    let b = cx.debug_bounds("tree-chevron-b").expect("b row").center();
    pull((100.0, f32::from(a.y)), (100.0, f32::from(b.y)), cx);
    change(&host, cx);
    release((100.0, f32::from(b.y)), cx);
    assert_eq!(said(&host, cx), ["owner changed"]);
}
