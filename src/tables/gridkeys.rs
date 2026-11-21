use std::rc::Rc;

use anyhow::anyhow;
use gpui::{
    App, ClipboardItem, Entity, KeyDownEvent, Pixels, Point, SharedString, TaskExt, Window, point,
};

use super::{
    formula::filled,
    grid::{Face, Merge, OnCells, Sheet, Text, range},
};
use crate::{
    forms::Editing,
    theme::{ActiveTheme, ControlSize, Density},
};

/// Sizes in pixels: a cell, a header, the row labels, and the frozen stretch.
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub wide: Pixels,
    pub tall: Pixels,
    pub label: Pixels,
    pub frozen: Point<Pixels>,
}

impl Metrics {
    /// A face's sizes at the window's rem.
    pub(crate) fn new(face: &Face, window: &Window, cx: &App) -> Self {
        let theme = cx.theme();
        let rem = window.rem_size();
        let (wide, tall) = (
            theme.grid_column().to_pixels(rem),
            theme.table_row(Density::Compact).to_pixels(rem),
        );
        Self {
            wide,
            tall,
            label: if face.numbered {
                theme.control_height(ControlSize::Lg).to_pixels(rem) * 1.25
            } else {
                Pixels::ZERO
            },
            frozen: point(wide * face.frozen.1 as f32, tall * face.frozen.0 as f32),
        }
    }

    /// The cell under a point within the grid, if any.
    pub(crate) fn cell_at(
        &self,
        at: Point<Pixels>,
        offset: Point<Pixels>,
        face: (usize, usize),
    ) -> Option<(usize, usize)> {
        let (x, y) = (at.x - self.label, at.y - self.tall);
        if x < Pixels::ZERO || y < Pixels::ZERO {
            return None;
        }
        let x = if x < self.frozen.x { x } else { x + offset.x };
        let y = if y < self.frozen.y { y } else { y + offset.y };
        let (col, row) = ((x / self.wide) as usize, (y / self.tall) as usize);
        (row < face.0 && col < face.1).then_some((row, col))
    }
}

/// The cell a place stands for: a merge's first cell for any cell it covers.
pub(crate) fn land(merges: &[Merge], (row, col): (usize, usize)) -> (usize, usize) {
    merges
        .iter()
        .find(|merge| {
            (merge.from.0..=merge.to.0).contains(&row) && (merge.from.1..=merge.to.1).contains(&col)
        })
        .map_or((row, col), |merge| merge.from)
}

/// Where a key moves the cursor from a cell: past the far side of a merge it stands on, and onto the first cell of any merge it reaches.
pub(crate) fn stepped(
    merges: &[Merge],
    (row, col): (usize, usize),
    key: &str,
    (rows, cols): (usize, usize),
) -> Option<(usize, usize)> {
    let far = merges
        .iter()
        .find(|merge| merge.from == (row, col))
        .map_or((row, col), |merge| merge.to);
    let to = match key {
        "up" => (row.saturating_sub(1), col),
        "down" | "enter" => ((far.0 + 1).min(rows - 1), col),
        "left" => (row, col.saturating_sub(1)),
        "right" => (row, (far.1 + 1).min(cols - 1)),
        "home" => (row, 0),
        "end" => (row, cols - 1),
        _ => return None,
    };
    Some(land(merges, to))
}

/// Pulls the cursor, its anchor and a fill back inside a grid of `size`; whether anything moved.
pub(crate) fn fit(sheet: &mut Sheet, size: (usize, usize)) -> bool {
    let last = (size.0.saturating_sub(1), size.1.saturating_sub(1));
    let inside = |(row, col): (usize, usize)| (row.min(last.0), col.min(last.1));
    let (cursor, anchor) = (inside(sheet.cursor), inside(sheet.anchor));
    let moved = (cursor, anchor) != (sheet.cursor, sheet.anchor);
    if moved {
        log::info!("grid: now {size:?}; the cursor moved to {cursor:?}");
        (sheet.cursor, sheet.anchor, sheet.fill) = (cursor, anchor, None);
    }
    moved
}

/// Tab and Shift-Tab: keep an open edit, then step across, staying inside the grid.
pub(crate) fn across(
    (sheet, editor): (&Entity<Sheet>, &Entity<Editing>),
    (merges, size, back): (&[Merge], (usize, usize), bool),
    (metrics, frozen): (Metrics, (usize, usize)),
    window: &mut Window,
    cx: &mut App,
) {
    cx.stop_propagation();
    if size.0 == 0 || size.1 == 0 {
        return;
    }
    let (row, col) = sheet.read(cx).cursor;
    if sheet.read(cx).editing {
        editor.update(cx, |editing, cx| editing.finish(false, window, cx));
    }
    let key = if back { "left" } else { "right" };
    let to = stepped(merges, (row, col), key, size).expect("left and right always step");
    sheet.update(cx, |sheet, cx| {
        (sheet.cursor, sheet.anchor) = (to, to);
        reveal(sheet, metrics, frozen);
        cx.notify();
    });
}

/// Scrolls just enough that the cursor shows, past the frozen rows and columns.
pub(crate) fn reveal(sheet: &mut Sheet, metrics: Metrics, frozen: (usize, usize)) {
    let (row, col) = sheet.cursor;
    let room_x = sheet.bounds.size.width - metrics.label - metrics.frozen.x;
    let room_y = sheet.bounds.size.height - metrics.tall - metrics.frozen.y;
    if col >= frozen.1 {
        let x = metrics.wide * col as f32 - metrics.frozen.x;
        sheet.offset.x = sheet
            .offset
            .x
            .min(x)
            .max(x + metrics.wide - room_x)
            .max(Pixels::ZERO);
    }
    if row >= frozen.0 {
        let y = metrics.tall * row as f32 - metrics.frozen.y;
        sheet.offset.y = sheet
            .offset
            .y
            .min(y)
            .max(y + metrics.tall - room_y)
            .max(Pixels::ZERO);
    }
}

/// Opens the cursor's cell with `text`; a kept edit reports and moves down.
pub(crate) fn begin(
    sheet: &Entity<Sheet>,
    editor: &Entity<Editing>,
    on_change: Option<OnCells>,
    text: String,
    at_end: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let (cell, rows) = (sheet.read(cx).cursor, sheet.read(cx).rows);
    let moved = sheet.clone();
    editor.update(cx, |editing, _| {
        editing.on_commit = Some(Rc::new(move |text, window, cx| {
            log::info!("grid: {cell:?} is now {text}");
            if let Some(on_change) = &on_change {
                on_change(&[(cell.0, cell.1, text.clone())], window, cx);
            }
            moved.update(cx, |sheet, _| {
                sheet.cursor.0 = (cell.0 + 1).min(rows.saturating_sub(1));
                sheet.anchor = sheet.cursor;
            });
        }))
    });
    sheet.update(cx, |sheet, _| sheet.editing = true);
    let end = text.len();
    Editing::begin(editor, text, window, cx);
    if at_end && let Some(field) = editor.read(cx).field() {
        field.update(cx, |field, cx| field.select(end..end, cx));
    }
}

/// Text for the clipboard: the range's cells as they show, tabs between, lines below.
fn copied(shown: &Text, sheet: &Sheet) -> String {
    let ((top, left), (bottom, right)) = range(sheet);
    (top..=bottom)
        .map(|row| {
            (left..=right)
                .map(|col| shown(row, col).to_string())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pasted text as cell edits from the cursor on, clipped to the grid.
pub(crate) fn pasted(
    text: &str,
    from: (usize, usize),
    size: (usize, usize),
) -> Vec<(usize, usize, SharedString)> {
    text.lines()
        .enumerate()
        .flat_map(|(dr, line)| {
            line.split('\t').enumerate().map(move |(dc, cell)| {
                (
                    from.0 + dr,
                    from.1 + dc,
                    SharedString::from(cell.to_string()),
                )
            })
        })
        .filter(|(row, col, _)| *row < size.0 && *col < size.1)
        .collect()
}

/// The key handler: arrows walk, stepping over merges, and Shift stretches the range; Enter steps down, typing or F2 edits, Delete clears, Cmd-C and Cmd-V copy and paste.
pub(crate) fn keys(
    face: Face,
    metrics: Metrics,
    sheet: Entity<Sheet>,
    editor: Entity<Editing>,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        if sheet.read(cx).editing {
            return;
        }
        let (rows, cols) = (face.rows, face.cols);
        if rows == 0 || cols == 0 {
            return;
        }
        let stroke = &event.keystroke;
        let held = &stroke.modifiers;
        let (row, col) = sheet.read(cx).cursor;
        if let Some(to) = stepped(&face.merges, (row, col), stroke.key.as_str(), (rows, cols)) {
            cx.stop_propagation();
            let stretch = held.shift && stroke.key != "enter";
            sheet.update(cx, |sheet, cx| {
                sheet.cursor = to;
                if !stretch {
                    sheet.anchor = to;
                }
                reveal(sheet, metrics, face.frozen);
                cx.notify();
            });
            return;
        }
        // Browsers on a Mac send Cmd; wasm's secondary is Control.
        let command = held.secondary(cx) || (cfg!(target_family = "wasm") && held.platform);
        match (stroke.key.as_str(), command) {
            ("escape", _) => sheet.update(cx, |sheet, cx| {
                sheet.anchor = sheet.cursor;
                cx.notify();
            }),
            ("f2", _) => begin(
                &sheet,
                &editor,
                face.on_change.clone(),
                (face.raw)(row, col).to_string(),
                true,
                window,
                cx,
            ),
            ("backspace" | "delete", false) => {
                let ((top, left), (bottom, right)) = range(sheet.read(cx));
                let cleared: Vec<_> = (top..=bottom)
                    .flat_map(|row| {
                        (left..=right).map(move |col| (row, col, SharedString::default()))
                    })
                    .collect();
                if let Some(on_change) = &face.on_change {
                    on_change(&cleared, window, cx);
                }
            }
            ("c", true) => cx.write_to_clipboard(ClipboardItem::new_string(copied(
                &face.shown,
                sheet.read(cx),
            ))),
            ("v", true) => {
                // The web reads its clipboard only through the async reader.
                let read = cx.read_from_clipboard_async();
                let on_change = face.on_change.clone();
                window
                    .spawn(cx, async move |cx| {
                        let item = read.await.map_err(|error| anyhow!("{error:?}"))?;
                        let Some(text) = item.and_then(|item| item.text()) else {
                            log::info!("grid: the clipboard holds no text to paste");
                            return Ok(());
                        };
                        let edits = pasted(&text, (row, col), (rows, cols));
                        log::info!("grid: pasted {} cells", edits.len());
                        if let Some(on_change) = on_change {
                            cx.update(|window, cx| on_change(&edits, window, cx))?;
                        }
                        anyhow::Ok(())
                    })
                    .detach_and_log_err(cx);
            }
            _ => {
                let typed = stroke.key_char.clone().filter(|typed| {
                    !held.platform && !held.control && !typed.chars().any(char::is_control)
                });
                let Some(typed) = typed else {
                    return;
                };
                begin(
                    &sheet,
                    &editor,
                    face.on_change.clone(),
                    typed,
                    true,
                    window,
                    cx,
                );
            }
        }
        cx.stop_propagation();
    }
}

/// Applies a fill: the range's runs carry on down, or across, to where the handle was dropped.
pub(crate) fn fill(
    sheet: &Entity<Sheet>,
    raw: &Text,
    on_change: &Option<OnCells>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((row, col)) = sheet.update(cx, |sheet, cx| {
        cx.notify();
        sheet.fill.take()
    }) else {
        return;
    };
    let ((top, left), (bottom, right)) = range(sheet.read(cx));
    let mut edits = Vec::new();
    if row > bottom {
        for c in left..=right {
            let run: Vec<SharedString> = (top..=bottom).map(|r| raw(r, c)).collect();
            for (ix, text) in filled(&run, row - bottom, false).into_iter().enumerate() {
                edits.push((bottom + 1 + ix, c, text));
            }
        }
    } else if col > right {
        for r in top..=bottom {
            let run: Vec<SharedString> = (left..=right).map(|c| raw(r, c)).collect();
            for (ix, text) in filled(&run, col - right, true).into_iter().enumerate() {
                edits.push((r, right + 1 + ix, text));
            }
        }
    }
    log::info!("grid: filled {} cells", edits.len());
    if let Some(on_change) = on_change.as_ref().filter(|_| !edits.is_empty()) {
        on_change(&edits, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::pasted;

    #[test]
    fn pasted_text_lands_from_the_cursor_and_stops_at_the_edge() {
        let edits: Vec<(usize, usize, String)> = pasted("a\tb\nc\td", (1, 1), (3, 2))
            .into_iter()
            .map(|(row, col, text)| (row, col, text.to_string()))
            .collect();
        assert_eq!(edits, [(1, 1, "a".into()), (2, 1, "c".into())]);
    }
}
