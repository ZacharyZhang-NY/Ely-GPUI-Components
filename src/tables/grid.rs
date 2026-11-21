use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Div, DragMoveEvent, ElementId, Entity, EntityId, FocusHandle,
    InteractiveElement, IntoElement, MouseButton, MouseMoveEvent, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, point, prelude::*,
};

use super::gridkeys::{Metrics, across, begin, fill, fit, keys, land};
use crate::{
    forms::{Editing, Input},
    primitives::{FocusNext, FocusPrev},
    theme::{ActiveTheme, ControlSize, TextSize},
    typography::Ellipsis,
};

pub(crate) type OnCells = Rc<dyn Fn(&[(usize, usize, SharedString)], &mut Window, &mut App)>;
pub(crate) type Text = Rc<dyn Fn(usize, usize) -> SharedString>;

/// A rectangle of cells shown as one, from its first corner to its last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Merge {
    pub from: (usize, usize),
    pub to: (usize, usize),
}

/// A grid's own state: the cursor and the corner its range reaches from, whether it edits, how far it has scrolled, its bounds, and where a fill would reach.
#[derive(Default)]
pub(crate) struct Sheet {
    pub cursor: (usize, usize),
    pub anchor: (usize, usize),
    pub editing: bool,
    pub offset: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
    pub fill: Option<(usize, usize)>,
    pub rows: usize,
}

/// What a grid face supplies: its size, its labels, the text a cell holds and shows, and who hears edits.
pub(crate) struct Face {
    pub id: ElementId,
    pub rows: usize,
    pub cols: usize,
    pub titles: Rc<dyn Fn(usize) -> SharedString>,
    pub numbered: bool,
    pub raw: Text,
    pub shown: Text,
    pub merges: Vec<Merge>,
    pub frozen: (usize, usize),
    pub fills: bool,
    pub on_change: Option<OnCells>,
}

/// A drag of a grid's fill handle, named by its owner.
pub(crate) struct FillDrag {
    owner: EntityId,
}

/// The range the cursor and its anchor span, as top-left and bottom-right.
pub(crate) fn range(sheet: &Sheet) -> ((usize, usize), (usize, usize)) {
    let ((r0, c0), (r1, c1)) = (sheet.anchor, sheet.cursor);
    ((r0.min(r1), c0.min(c1)), (r0.max(r1), c0.max(c1)))
}

/// Draws a grid face: headers, labels, visible cells, the range, the cursor and its fill handle.
pub(crate) fn grid(face: Face, base: Div, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let id = face.id.clone();
    let focus: FocusHandle =
        crate::primitives::tab_stop((id.clone(), "focus").into(), true, window, cx);
    let sheet: Entity<Sheet> =
        window.use_keyed_state((id.clone(), "sheet"), cx, |_, _| Sheet::default());
    let editor = window.use_keyed_state((id.clone(), "editor"), cx, |_, _| Editing::default());
    let shrunk = sheet.update(cx, |sheet, _| {
        sheet.rows = face.rows;
        fit(sheet, (face.rows, face.cols))
    });
    if shrunk {
        editor.update(cx, |editing, cx| editing.finish(true, window, cx));
    }
    if sheet.read(cx).editing && editor.read(cx).field().is_none() {
        sheet.update(cx, |sheet, _| sheet.editing = false);
        window.focus(&focus, cx);
    }
    let theme = cx.theme();
    let colors = theme.colors.clone();
    let rem = window.rem_size();
    let metrics = Metrics::new(&face, window, cx);
    let (rows, cols) = (face.rows, face.cols);
    let (offset, bounds, editing, fill_to) = {
        let sheet = sheet.read(cx);
        (sheet.offset, sheet.bounds, sheet.editing, sheet.fill)
    };
    let ((top, left), (bottom, right)) = range(sheet.read(cx));
    let cursor = sheet.read(cx).cursor;
    let focused = focus.contains_focused(window, cx);
    let span = |from: Pixels, cell: Pixels, count: usize, size: Pixels| {
        let first = (from / cell).floor() as usize;
        let last = ((from + size) / cell).ceil() as usize;
        first.min(count)..last.min(count)
    };
    let view_rows = span(
        offset.y + metrics.frozen.y,
        metrics.tall,
        rows,
        bounds.size.height,
    );
    let view_cols = span(
        offset.x + metrics.frozen.x,
        metrics.wide,
        cols,
        bounds.size.width,
    );
    let x_of = |col: usize| {
        let x = metrics.wide * col as f32;
        metrics.label + if col < face.frozen.1 { x } else { x - offset.x }
    };
    let y_of = |row: usize| {
        let y = metrics.tall * row as f32;
        metrics.tall + if row < face.frozen.0 { y } else { y - offset.y }
    };
    let merged = |row: usize, col: usize| {
        face.merges.iter().find(|merge| {
            (merge.from.0..=merge.to.0).contains(&row) && (merge.from.1..=merge.to.1).contains(&col)
        })
    };
    let field = editing.then(|| editor.read(cx).field()).flatten();
    let cell = |row: usize, col: usize| -> Option<AnyElement> {
        let merge = merged(row, col);
        if merge.is_some_and(|merge| merge.from != (row, col)) {
            return None;
        }
        let (wide, tall) = match merge {
            Some(merge) => (
                metrics.wide * (merge.to.1 - col + 1) as f32,
                metrics.tall * (merge.to.0 - row + 1) as f32,
            ),
            None => (metrics.wide, metrics.tall),
        };
        let text = (face.shown)(row, col);
        let number = text.trim().replace(',', "").parse::<f64>().is_ok();
        let content: AnyElement = match (&field, (row, col) == cursor) {
            (Some(field), true) => {
                let editor = editor.clone();
                div()
                    .size_full()
                    .on_key_down(move |event, window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            editor.update(cx, |editing, cx| editing.finish(true, window, cx));
                        }
                    })
                    .child(Input::new(field).size(ControlSize::Sm))
                    .into_any_element()
            }
            _ => Ellipsis::new(text).into_any_element(),
        };
        Some(
            div()
                .absolute()
                .left(x_of(col))
                .top(y_of(row))
                .w(wide)
                .h(tall)
                .flex()
                .items_center()
                .when(number, |cell| cell.justify_end())
                .px_2()
                .border_r_1()
                .border_b_1()
                .border_color(colors.border)
                .bg(colors.bg)
                .child(content)
                .into_any_element(),
        )
    };
    let frozen_rows = 0..face.frozen.0.min(rows);
    let frozen_cols = 0..face.frozen.1.min(cols);
    let layer = |rows: std::ops::Range<usize>, cols: std::ops::Range<usize>| {
        rows.flat_map(move |row| cols.clone().map(move |col| (row, col)))
            .filter_map(|(row, col)| cell(row, col))
            .collect::<Vec<_>>()
    };
    let main = layer(view_rows.clone(), view_cols.clone());
    let top_band = layer(frozen_rows.clone(), view_cols.clone());
    let left_band = layer(view_rows.clone(), frozen_cols.clone());
    let corner_band = layer(frozen_rows, frozen_cols);
    let headers = view_cols
        .clone()
        .chain(0..face.frozen.1.min(cols))
        .map(|col| {
            div()
                .absolute()
                .left(x_of(col))
                .top_0()
                .w(metrics.wide)
                .h(metrics.tall)
                .flex()
                .items_center()
                .justify_center()
                .border_r_1()
                .border_b_1()
                .border_color(colors.border)
                .bg(if (left..=right).contains(&col) && focused {
                    colors.active
                } else {
                    colors.sunken
                })
                .text_color(colors.fg_muted)
                .child((face.titles)(col))
        })
        .collect::<Vec<_>>();
    let labels = face.numbered.then(|| {
        view_rows
            .clone()
            .chain(0..face.frozen.0.min(rows))
            .map(|row| {
                div()
                    .absolute()
                    .left_0()
                    .top(y_of(row))
                    .w(metrics.label)
                    .h(metrics.tall)
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_r_1()
                    .border_b_1()
                    .border_color(colors.border)
                    .bg(if (top..=bottom).contains(&row) && focused {
                        colors.active
                    } else {
                        colors.sunken
                    })
                    .text_color(colors.fg_muted)
                    .child((row + 1).to_string())
            })
            .collect::<Vec<_>>()
    });
    let area = Bounds::new(
        point(x_of(left), y_of(top)),
        gpui::size(
            metrics.wide * (right - left + 1) as f32,
            metrics.tall * (bottom - top + 1) as f32,
        ),
    );
    let owner = sheet.entity_id();
    let marks = [
        div()
            .absolute()
            .left(area.origin.x)
            .top(area.origin.y)
            .w(area.size.width)
            .h(area.size.height)
            .bg(colors.selection)
            .border_1()
            .border_color(colors.accent.opacity(0.6))
            .into_any_element(),
        div()
            .absolute()
            .left(x_of(cursor.1))
            .top(y_of(cursor.0))
            .w(metrics.wide)
            .h(metrics.tall)
            .border_2()
            .border_color(colors.accent)
            .into_any_element(),
    ];
    let preview = fill_to.map(|(row, col)| {
        let down = row > bottom;
        let (to_row, to_col) = if down {
            (row, right)
        } else {
            (bottom, col.max(right))
        };
        div()
            .absolute()
            .left(area.origin.x)
            .top(area.origin.y)
            .w(metrics.wide * (to_col - left + 1) as f32)
            .h(metrics.tall * (to_row - top + 1) as f32)
            .border_1()
            .border_color(colors.fg_muted)
    });
    let handle = (face.fills && !editing).then(|| {
        let reach = theme.status_dot();
        div()
            .id((id.clone(), "fill"))
            .absolute()
            .left(area.origin.x + area.size.width - reach.to_pixels(rem) * 0.5)
            .top(area.origin.y + area.size.height - reach.to_pixels(rem) * 0.5)
            .size(reach)
            .bg(colors.accent)
            .border_1()
            .border_color(colors.bg)
            .cursor_crosshair()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_drag(FillDrag { owner }, |_, _, _, cx| {
                cx.new(|_| gpui::EmptyView)
            })
    });
    let (face_rows, face_cols) = (rows, cols);
    let (pressed, dragged, moved, measured, scrolled, dropped) = (
        sheet.clone(),
        sheet.clone(),
        sheet.clone(),
        sheet.clone(),
        sheet.clone(),
        sheet.clone(),
    );
    let (raw, on_change, begin_with) = (face.raw.clone(), face.on_change.clone(), editor.clone());
    let (fill_raw, fill_hears) = (face.raw.clone(), face.on_change.clone());
    let numbered = face.numbered;
    let (merges, frozen) = (Rc::new(face.merges.clone()), face.frozen);
    let hovered_merges = merges.clone();
    let ahead = (sheet.clone(), editor.clone(), merges.clone());
    let behind = (sheet.clone(), editor.clone(), merges.clone());
    let keyed = keys(face, metrics, sheet.clone(), editor.clone());
    let focus_on_press = focus.clone();
    let text_size = theme.text_size(TextSize::Sm);
    base.id(id.clone())
        .track_focus(&focus)
        .relative()
        .overflow_hidden()
        .text_size(text_size)
        .text_color(colors.fg)
        .bg(colors.bg)
        .border_1()
        .border_color(colors.border)
        .on_key_down(keyed)
        .capture_action(move |_: &FocusNext, window, cx| {
            let (sheet, editor, merges) = &ahead;
            let size = (face_rows, face_cols);
            across(
                (sheet, editor),
                (merges, size, false),
                (metrics, frozen),
                window,
                cx,
            )
        })
        .capture_action(move |_: &FocusPrev, window, cx| {
            let (sheet, editor, merges) = &behind;
            let size = (face_rows, face_cols);
            across(
                (sheet, editor),
                (merges, size, true),
                (metrics, frozen),
                window,
                cx,
            )
        })
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            let at = event.position - pressed.read(cx).bounds.origin;
            let Some(cell) = metrics.cell_at(at, pressed.read(cx).offset, (face_rows, face_cols))
            else {
                return;
            };
            let cell = land(&merges, cell);
            window.focus(&focus_on_press, cx);
            pressed.update(cx, |sheet, cx| {
                sheet.cursor = cell;
                if !event.modifiers.shift {
                    sheet.anchor = cell;
                }
                cx.notify();
            });
            if event.click_count == 2 {
                let text = raw(cell.0, cell.1).to_string();
                begin(
                    &pressed,
                    &begin_with,
                    on_change.clone(),
                    text,
                    false,
                    window,
                    cx,
                );
            }
        })
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
            if event.pressed_button != Some(MouseButton::Left) || cx.has_active_drag() {
                return;
            }
            let at = event.position - moved.read(cx).bounds.origin;
            if let Some(cell) = metrics.cell_at(at, moved.read(cx).offset, (face_rows, face_cols)) {
                let cell = land(&hovered_merges, cell);
                moved.update(cx, |sheet, cx| {
                    if sheet.cursor != cell && !sheet.editing {
                        sheet.cursor = cell;
                        cx.notify();
                    }
                });
            }
        })
        .on_scroll(move |event, _, cx| {
            let delta = event.delta.pixel_delta(metrics.tall);
            scrolled.update(cx, |sheet, cx| {
                let most = point(
                    (metrics.wide * face_cols as f32 + metrics.label - sheet.bounds.size.width)
                        .max(Pixels::ZERO),
                    (metrics.tall * (face_rows + 1) as f32 - sheet.bounds.size.height)
                        .max(Pixels::ZERO),
                );
                sheet.offset = point(
                    (sheet.offset.x - delta.x).clamp(Pixels::ZERO, most.x),
                    (sheet.offset.y - delta.y).clamp(Pixels::ZERO, most.y),
                );
                cx.notify();
            });
            cx.stop_propagation();
        })
        .on_drag_move(move |event: &DragMoveEvent<FillDrag>, _, cx| {
            if event.drag(cx).owner != owner {
                return;
            }
            let at = event.event.position - dragged.read(cx).bounds.origin;
            let cell = metrics.cell_at(at, dragged.read(cx).offset, (face_rows, face_cols));
            dragged.update(cx, |sheet, cx| {
                if sheet.fill != cell {
                    sheet.fill = cell;
                    cx.notify();
                }
            });
        })
        .on_drop(move |drag: &FillDrag, window, cx| {
            if drag.owner == owner {
                fill(&dropped, &fill_raw, &fill_hears, window, cx);
            }
        })
        .children(main)
        .children(top_band)
        .children(left_band)
        .children(corner_band)
        .children(marks)
        .children(preview)
        .children(handle)
        .children(headers)
        .children(labels.into_iter().flatten())
        .when(numbered, |grid| {
            grid.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(metrics.label)
                    .h(metrics.tall)
                    .bg(colors.sunken)
                    .border_r_1()
                    .border_b_1()
                    .border_color(colors.border),
            )
        })
        .child(
            canvas(
                move |bounds, _, cx| {
                    if measured.read(cx).bounds != bounds {
                        measured.update(cx, |sheet, cx| {
                            sheet.bounds = bounds;
                            cx.notify();
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
}
