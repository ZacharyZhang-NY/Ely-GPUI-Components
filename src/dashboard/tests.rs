use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, KeyBinding, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window, div, px,
};

use super::{
    AlertList, AlertState, DashboardCard, DashboardFilter, DashboardFilterBar, DashboardGrid,
    EventStream, MonitorAlert, StreamEvent, Tile, TimeWindow,
};
use crate::{
    buttons::Button,
    data_display::Tone,
    forms::Choice,
    primitives::{FocusNext, Severity},
    theme::Theme,
};

/// A view that shows a grid of three tiles and keeps the tiles it heard.
struct Wall {
    tiles: Vec<Tile>,
    heard: Vec<Vec<Tile>>,
    width: f32,
    inside: FocusHandle,
}

impl Render for Wall {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let grid = self.tiles.iter().fold(
            DashboardGrid::new("grid", 12, self.tiles.clone(), move |tiles, _, cx| {
                owner.update(cx, |wall, cx| {
                    wall.tiles = tiles.clone();
                    wall.heard.push(tiles);
                    cx.notify();
                })
            }),
            |grid, tile| {
                let card = DashboardCard::new(tile.key.clone());
                let card = match tile.key.as_ref() {
                    "b" => card.body(Button::new("inside", "Inside").focus_handle(&self.inside)),
                    _ => card,
                };
                grid.card(tile.key.clone(), card)
            },
        );
        div().w(px(self.width)).child(grid)
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn wall(cx: &mut TestAppContext) -> (Entity<Wall>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, cx| Wall {
        tiles: vec![
            Tile::new("a", (0, 0), (6, 2)),
            Tile::new("b", (6, 0), (6, 2)),
            Tile::new("c", (0, 2), (12, 2)),
        ],
        heard: Vec::new(),
        width: 1200.0,
        inside: cx.focus_handle(),
    });
    settle(cx);
    (host, cx)
}

fn place(host: &Entity<Wall>, key: &str, cx: &mut VisualTestContext) -> (u32, u32, u32, u32) {
    host.read_with(cx, |wall, _| {
        let tile = wall
            .tiles
            .iter()
            .find(|tile| tile.key == key)
            .expect("a tile");
        (tile.x, tile.y, tile.w, tile.h)
    })
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
}

/// The first Tab stop is the first tile the grid draws, a.
#[gpui::test]
fn arrows_step_a_focused_tile_past_its_neighbours_and_shift_grows_it(cx: &mut TestAppContext) {
    let (host, cx) = wall(cx);
    cx.update(|window, cx| window.focus_next(cx));
    settle(cx);
    press("down", cx);
    assert_eq!(place(&host, "a", cx), (0, 4, 6, 2), "a went below c");
    press("up", cx);
    assert_eq!(place(&host, "a", cx), (0, 0, 6, 2), "and back on top");
    press("shift-right", cx);
    assert_eq!(place(&host, "a", cx), (0, 0, 7, 2), "a grew a column");
    assert_eq!(place(&host, "b", cx), (6, 2, 6, 2), "pushing b below it");
    assert_eq!(place(&host, "c", cx), (0, 4, 12, 2), "and c below b");
    press("right", cx);
    assert_eq!(place(&host, "a", cx).0, 1, "Right moves a column over");
    press("left", cx);
    assert_eq!(place(&host, "a", cx).0, 0, "and Left back");
}

#[gpui::test]
fn arrows_in_a_control_inside_a_card_leave_the_tiles_where_they_are(cx: &mut TestAppContext) {
    let (host, cx) = wall(cx);
    let inside = host.read_with(cx, |wall, _| wall.inside.clone());
    cx.update(|window, cx| window.focus(&inside, cx));
    settle(cx);
    press("right", cx);
    press("down", cx);
    assert!(
        host.read_with(cx, |wall, _| wall.heard.is_empty()),
        "the button keeps its keys"
    );
}

type Part = fn(&Board, Entity<Board>) -> AnyElement;

/// A view that shows one dashboard part, keeps what it heard, and holds how many events have come.
struct Board {
    part: Part,
    said: Vec<String>,
    events: usize,
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(640.0)).child((self.part)(self, cx.entity()))
    }
}

fn board(part: Part, cx: &mut TestAppContext) -> (Entity<Board>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        crate::forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Board {
        part,
        said: Vec::new(),
        events: 2,
    });
    settle(cx);
    (host, cx)
}

fn say(owner: &Entity<Board>, words: String, cx: &mut App) {
    owner.update(cx, |board, cx| {
        board.said.push(words);
        cx.notify();
    });
}

fn said(host: &Entity<Board>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |board, _| board.said.clone())
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

fn now() -> jiff::Timestamp {
    "2026-09-27T12:00:00Z".parse().expect("a time")
}

fn alerts(_: &Board, owner: Entity<Board>) -> AnyElement {
    let alert = |key: &str, state| MonitorAlert {
        key: key.to_string().into(),
        title: key.to_string().into(),
        source: "prometheus".into(),
        severity: Severity::Danger,
        state,
        since: now(),
    };
    let opened = owner.clone();
    AlertList::new(
        "alerts",
        [
            alert("cpu", AlertState::Firing),
            alert("disk", AlertState::Resolved),
        ],
        now(),
    )
    .on_acknowledge(move |key, _, cx| say(&owner, format!("ack {key}"), cx))
    .on_open(move |key, _, cx| say(&opened, format!("open {key}"), cx))
    .into_any_element()
}

/// Stops: the list, then the firing alert's Acknowledge; a resolved alert has none.
#[gpui::test]
fn a_firing_alert_is_acknowledged_from_its_row_and_enter_opens_it(cx: &mut TestAppContext) {
    let (host, cx) = board(alerts, cx);
    tab(2, cx);
    tap("space", cx);
    tab(3, cx);
    tap("enter", cx);
    assert_eq!(
        said(&host, cx),
        ["ack cpu", "open cpu"],
        "the third stop wraps to the list"
    );
}

fn stream(board: &Board, _: Entity<Board>) -> AnyElement {
    let events = (0..board.events).map(|ix| StreamEvent {
        key: format!("e{ix}").into(),
        kind: "deploy".into(),
        text: format!("event {ix}").into(),
        tone: Tone::Neutral,
        at: now(),
    });
    EventStream::new("stream", events, now()).into_any_element()
}

/// Stops: the deploy chip, then Pause.
#[gpui::test]
fn a_paused_stream_counts_what_came_since(cx: &mut TestAppContext) {
    let (host, cx) = board(stream, cx);
    assert!(cx.debug_bounds("event-stream-live").is_some());
    tab(2, cx);
    tap("space", cx);
    assert!(
        cx.debug_bounds("event-stream-held-0").is_some(),
        "nothing new yet"
    );
    host.update(cx, |board, cx| {
        board.events = 4;
        cx.notify();
    });
    settle(cx);
    assert!(
        cx.debug_bounds("event-stream-held-2").is_some(),
        "two came while it held"
    );
}

fn filters(_: &Board, owner: Entity<Board>) -> AnyElement {
    let filter = |key: &str| DashboardFilter {
        key: key.to_string().into(),
        name: key.to_string().into(),
        choices: vec![Choice::new("a", "A"), Choice::new("b", "B")],
        picked: Some("a".into()),
    };
    DashboardFilterBar::new(
        "filters",
        TimeWindow::ALL[0],
        [filter("region"), filter("env")],
        |_, _, _| {},
        move |key, value, _, cx| say(&owner, format!("{key} {value:?}"), cx),
    )
    .into_any_element()
}

/// Stops: the window, the two filters, then Clear.
#[gpui::test]
fn clear_lets_every_filter_go(cx: &mut TestAppContext) {
    let (host, cx) = board(filters, cx);
    tab(4, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["region None", "env None"]);
}

#[gpui::test]
fn a_narrow_grid_stacks_its_tiles_in_reading_order(cx: &mut TestAppContext) {
    let (host, cx) = wall(cx);
    host.update(cx, |wall, cx| {
        wall.width = 300.0;
        cx.notify();
    });
    settle(cx);
    let mut bounds = |key: &'static str| cx.debug_bounds(key).expect("a stacked tile");
    let (a, b, c) = (
        bounds("stacked-a"),
        bounds("stacked-b"),
        bounds("stacked-c"),
    );
    assert_eq!(
        (a.left(), b.left(), c.left()),
        (a.left(), a.left(), a.left()),
        "one column"
    );
    assert!(
        a.top() < b.top() && b.top() < c.top(),
        "a, b, then c: {a:?} {b:?} {c:?}"
    );
    assert_eq!(a.size.width, px(300.0), "each fills the width");
}

/// A grid at `width` whose tile b has no card and whose card c has no tile.
struct Mismatched(f32);

impl Render for Mismatched {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tiles = [
            Tile::new("a", (0, 0), (6, 2)),
            Tile::new("b", (6, 0), (6, 2)),
        ];
        let grid = DashboardGrid::new("grid", 12, tiles, |_, _, _| {})
            .card("a", DashboardCard::new("A"))
            .card("c", DashboardCard::new("C"));
        div().w(px(self.0)).child(grid)
    }
}

#[gpui::test]
fn tiles_and_cards_that_do_not_match_are_left_out(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    for width in [960.0, 320.0] {
        let (_, cx) = cx.add_window_view(move |_, _| Mismatched(width));
        settle(cx);
    }
}
