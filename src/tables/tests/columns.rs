use std::{cell::RefCell, rc::Rc};

use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, Styled,
    TestAppContext, Window, div, point, px,
};

use super::{edit_at, settle};
use crate::{
    tables::{Cell, Column, DataTable, Row},
    theme::Theme,
};

/// How a wide table lays out its rows.
#[derive(Clone, Copy, Debug)]
enum Mode {
    Plain,
    Pinned,
    Long,
}

/// Twenty rows of three editable columns 200 wide in a table 280 wide; it keeps each edit's row and column.
struct Wide(Rc<RefCell<Vec<String>>>, Mode);

impl Render for Wide {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let heard = self.0.clone();
        let column = |key: &'static str| Column::new(key, key).width(gpui::rems(12.5)).editable();
        let first = match self.1 {
            Mode::Pinned => column("a").pinned(),
            _ => column("a"),
        };
        let table = DataTable::new("wide", [first, column("b"), column("c")])
            .rows(
                (0..20)
                    .map(|n| Row::new(format!("r{n}"), ["A".into(), "B".into(), "C".into()]))
                    .collect::<Vec<_>>(),
            )
            .on_edit(move |row, column, _, _, _| heard.borrow_mut().push(format!("{row} {column}")))
            .w(px(280.0));
        match self.1 {
            Mode::Long => table.virtualized().h(px(200.0)),
            _ => table,
        }
    }
}

#[gpui::test]
fn columns_past_the_table_scroll_sideways_and_a_plain_wheel_leaves_them(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    for mode in [Mode::Plain, Mode::Pinned, Mode::Long] {
        let heard = Rc::new(RefCell::new(Vec::new()));
        let seen = heard.clone();
        let (_, cx) = cx.add_window_view(|_, _| Wide(seen, mode));
        settle(cx);
        let cell = point(px(250.0), px(54.0));
        // A long table's rows take a plain wheel down; up leaves them at the top.
        let plain = match mode {
            Mode::Long => px(2000.0),
            _ => px(-2000.0),
        };
        for delta in [point(px(0.0), plain), point(px(-2000.0), px(0.0))] {
            cx.simulate_event(gpui::ScrollEvent {
                position: cell,
                delta: gpui::ScrollDelta::Pixels(delta),
                modifiers: Modifiers::none(),
                touch_phase: gpui::TouchPhase::Moved,
                source: gpui::ScrollSource::Pointer,
            });
            settle(cx);
            edit_at(cell, "x", cx);
        }
        assert_eq!(*heard.borrow(), ["r0 b", "r0 c"], "{mode:?}");
    }
}

/// Two rows under a sortable, editable name; it keeps the row of each edit.
struct Sorting(Rc<RefCell<Vec<String>>>);

impl Render for Sorting {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let heard = self.0.clone();
        DataTable::new("sorting", [Column::new("name", "Name").editable()])
            .rows(vec![
                Row::new("bea", ["Bea".into()]),
                Row::new("ada", ["Ada".into()]),
            ])
            .on_edit(move |row, _, _, _, _| heard.borrow_mut().push(row.to_string()))
            .w(px(400.0))
    }
}

#[gpui::test]
fn tab_reaches_a_header_and_enter_sorts_by_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    let heard = Rc::new(RefCell::new(Vec::new()));
    let seen = heard.clone();
    let (_, cx) = cx.add_window_view(|_, _| Sorting(seen));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("enter");
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").expect("a key"),
    });
    settle(cx);
    edit_at(point(px(120.0), px(54.0)), "x", cx);
    assert_eq!(*heard.borrow(), ["ada"], "Ada rises to the top");
}

/// Five sortable, editable columns 96 wide in a table 280 wide; it keeps the column of each edit.
struct Five(Rc<RefCell<Vec<String>>>);

impl Render for Five {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let heard = self.0.clone();
        let keys = ["a", "b", "c", "d", "e"];
        let columns = keys.map(|key| Column::new(key, key).width(gpui::rems(6.0)).editable());
        DataTable::new("five", columns)
            .rows(vec![Row::new("r", keys.map(Cell::from))])
            .on_edit(move |_, column, _, _, _| heard.borrow_mut().push(column.to_string()))
            .w(px(280.0))
    }
}

#[gpui::test]
fn a_focused_header_past_the_edge_scrolls_into_view(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    let heard = Rc::new(RefCell::new(Vec::new()));
    let seen = heard.clone();
    let (_, cx) = cx.add_window_view(|_, _| Five(seen));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    for _ in 0..5 {
        cx.update(|window, cx| window.focus_next(cx));
    }
    settle(cx);
    edit_at(point(px(270.0), px(54.0)), "x", cx);
    assert_eq!(
        *heard.borrow(),
        ["e"],
        "the last header's column sits at the box's end"
    );
}

/// A table in a column beside a long line, in a box its row measures by content.
struct Exposed;

impl Render for Exposed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let table = DataTable::new(
            "exposed",
            [Column::new("name", "Name"), Column::new("note", "Note")],
        )
        .rows(vec![
            Row::new("a", ["A".into(), "".into()]),
            Row::new("b", ["A much longer name".into(), "Note".into()]),
        ]);
        let line = "A line long enough to wrap in this box, so the column fills it, and more.";
        div()
            .flex()
            .child(div().child(div().flex().flex_col().child(table).child(line)))
    }
}

#[gpui::test]
fn rows_span_the_table_whatever_their_words(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Exposed);
    cx.run_until_parked();
    let short = cx
        .debug_bounds("table-row a")
        .expect("row a draws")
        .size
        .width;
    let long = cx
        .debug_bounds("table-row b")
        .expect("row b draws")
        .size
        .width;
    assert_eq!(short, long, "a short row keeps the long row's width");
}
