use gpui::{AnyElement, Entity, IntoElement, TestAppContext, VisualTestContext};

use super::{Stage, said, say, settle, stage};
use crate::canvas::{
    AssetPanel, AutoLayout, AutoLayoutControls, ExportFormat, ExportPanel, ExportSetting, Flow,
    Frame, LayerPanel, Place, Shape, ShapeKind,
};

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

fn layers(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let shapes = ["a", "b", "c"].map(|key| {
        Shape::new(
            key,
            key.to_uppercase(),
            ShapeKind::Rect,
            Frame::new(0.0, 0.0, 10.0, 10.0),
        )
    });
    LayerPanel::new("layers", shapes)
        .on_show(move |keys, _, cx| {
            let mut keys = keys.to_vec();
            keys.sort();
            say(&owner, format!("shown {keys:?}"), cx)
        })
        .into_any_element()
}

#[gpui::test]
fn space_on_the_top_layer_hides_it(cx: &mut TestAppContext) {
    let (host, cx) = stage(layers, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        ["shown [\"a\", \"b\"]"],
        "c paints last, so it heads the list"
    );
}

fn places(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let layout = AutoLayout {
        flow: Flow::Row,
        gap: 8.0,
        padding: (8.0, 8.0),
        place: (Place::Start, Place::Start),
    };
    AutoLayoutControls::new("auto", layout, move |next, _, cx| {
        say(&owner, format!("{:?}", next.place), cx)
    })
    .into_any_element()
}

#[gpui::test]
fn arrows_move_the_place_and_stop_at_the_edge(cx: &mut TestAppContext) {
    let (host, cx) = stage(places, cx);
    let grid = cx.debug_bounds("places").expect("the place grid");
    cx.simulate_click(grid.center(), gpui::Modifiers::none());
    settle(cx);
    for key in ["right", "down", "left", "up"] {
        tap(key, cx);
    }
    assert_eq!(
        said(&host, cx),
        ["(Center, Center)", "(Center, Start)", "(Start, Center)"],
        "a press picks; each arrow steps from the owner's place, and none steps past the edge"
    );
}

fn sizes(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let (added, sent) = (owner.clone(), owner);
    ExportPanel::new(
        "export",
        "Card",
        [ExportSetting {
            scale: 1.0,
            format: ExportFormat::Png,
        }],
        move |next, _, cx| {
            let scales: Vec<f32> = next.iter().map(|setting| setting.scale).collect();
            say(&added, format!("sizes {scales:?}"), cx)
        },
        move |files, _, cx| say(&sent, format!("files {files:?}"), cx),
    )
    .into_any_element()
}

#[gpui::test]
fn plus_adds_the_next_size_and_export_hands_on_the_files(cx: &mut TestAppContext) {
    let (host, cx) = stage(sizes, cx);
    tab(1, cx);
    tap("space", cx);
    tab(5, cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        ["sizes [1.0, 2.0]", "files [\"Card.png\"]"]
    );
}

fn assets(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    let shape =
        |key: &'static str, kind| Shape::new(key, key, kind, Frame::new(0.0, 0.0, 40.0, 40.0));
    AssetPanel::new("assets")
        .group(
            "Shapes",
            [
                shape("Button", ShapeKind::Rect),
                shape("Hexagon", ShapeKind::Polygon(6)),
            ],
        )
        .on_pick(move |shape, _, cx| say(&owner, format!("pick {}", shape.key), cx))
        .into_any_element()
}

#[gpui::test]
fn a_search_narrows_the_assets_and_enter_picks_the_first(cx: &mut TestAppContext) {
    let (host, cx) = stage(assets, cx);
    tab(1, cx);
    cx.simulate_input("hex");
    settle(cx);
    tap("enter", cx);
    assert_eq!(said(&host, cx), ["pick Hexagon"]);
}
