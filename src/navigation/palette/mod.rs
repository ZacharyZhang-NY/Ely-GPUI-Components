mod kinds;
mod launcher;
#[cfg(all(test, feature = "test-support"))]
mod tests;

use std::{ops::Range, rc::Rc};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, Entity, FontWeight, HighlightStyle,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Point, Role, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div, prelude::*,
};

use crate::{
    forms::{Down, Enter, OnValue, Pick, Run, TextInput, Up},
    motion,
    primitives::{Backdrop, FocusScope, Icon, IconName, give_back, take_focus},
    theme::{ActiveTheme, ControlSize, Elevation, IconSize, Radius, TextSize},
};

pub use kinds::{Command, CommandPalette, QuickOpen, QuickSwitcher, SearchPalette};
pub use launcher::QuickLauncher;

/// How well a query fits a text, and the byte ranges it covers.
#[derive(Debug, PartialEq)]
pub(crate) struct Fit {
    pub score: i32,
    pub hits: Vec<Range<usize>>,
}

fn fold(c: char) -> char {
    c.to_lowercase()
        .next()
        .expect("a char lowercases to at least one char")
}

/// The query's characters in order within `text`, ignoring case and spaces. Word starts and runs score higher, gaps lower.
pub(crate) fn fuzzy(query: &str, text: &str) -> Option<Fit> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(fold)
        .collect();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut next = 0;
    let end = match query.len() {
        0 => {
            return Some(Fit {
                score: 0,
                hits: Vec::new(),
            });
        }
        len => chars.iter().position(|&(_, c)| {
            next += usize::from(fold(c) == query[next]);
            next == len
        })?,
    };
    // ponytail: the tightest window at the first full match, not the best; score every window if rankings disappoint.
    let mut at = Vec::with_capacity(query.len());
    for ix in (0..=end).rev() {
        if at.len() < query.len() && fold(chars[ix].1) == query[query.len() - 1 - at.len()] {
            at.push(ix);
        }
    }
    at.reverse();
    let starts_word = |ix: usize| {
        ix == 0 || {
            let (before, here) = (chars[ix - 1].1, chars[ix].1);
            !before.is_alphanumeric() || (before.is_lowercase() && here.is_uppercase())
        }
    };
    let mut score = 0;
    let mut hits: Vec<Range<usize>> = Vec::new();
    for (k, &ix) in at.iter().enumerate() {
        let bonus = if starts_word(ix) { 8 } else { 0 };
        score += 16 + if k == 0 { bonus * 2 } else { bonus };
        if k > 0 {
            let gap = (ix - at[k - 1] - 1) as i32;
            score += if gap == 0 { 4 } else { -2 - gap };
        }
        let (from, c) = chars[ix];
        match hits.last_mut() {
            Some(last) if last.end == from => last.end = from + c.len_utf8(),
            _ => hits.push(from..from + c.len_utf8()),
        }
    }
    Some(Fit { score, hits })
}

/// `text` with the letters a query matched drawn in the accent color.
pub(crate) fn marked(text: SharedString, hits: Vec<Range<usize>>, cx: &App) -> AnyElement {
    let mark = HighlightStyle {
        color: Some(cx.theme().colors.accent),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(text)
        .with_highlights(hits.into_iter().map(|hit| (hit, mark)))
        .into_any_element()
}

/// One result: the value a pick hands back, then what the row shows.
pub(crate) struct Row {
    pub value: SharedString,
    /// The row's plain name, for assistive technology.
    pub name: SharedString,
    pub icon: Option<IconName>,
    pub label: AnyElement,
    pub detail: Option<AnyElement>,
    pub end: Option<AnyElement>,
}

/// Rows under an optional heading.
pub(crate) struct Group {
    pub title: Option<SharedString>,
    pub rows: Vec<Row>,
}

/// The row under the keyboard, the query it was set for, and the list's scroll.
#[derive(Default)]
struct Cursor {
    at: usize,
    query: Option<String>,
    scroll: ScrollHandle,
}

/// The query field kept for palette `id`.
pub(crate) fn query_field(
    id: &ElementId,
    placeholder: SharedString,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TextInput> {
    window.use_keyed_state((id.clone(), "query"), cx, |window, cx| {
        TextInput::new(window, cx).placeholder(placeholder)
    })
}

/// A query field over grouped rows. Arrows move, Enter or a click picks, Escape closes.
pub(crate) struct Palette {
    pub id: ElementId,
    pub input: Entity<TextInput>,
    pub groups: Vec<Group>,
    /// The row marked whenever the query changes.
    pub start: usize,
    /// Said when no row fits.
    pub empty: SharedString,
    pub on_pick: Option<OnValue>,
    pub on_close: Run,
}

impl Palette {
    /// Over a scrim; a click on it closes. Tab stays inside, and focus returns on every close.
    pub(crate) fn overlay(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let takeover = take_focus((self.id.clone(), "takeover"), window, cx);
        let scope = takeover.read(cx).focus.clone();
        if scope.is_focused(window) {
            window.focus(&self.input.read(cx).focus().clone(), cx);
        }
        let close = self.on_close.clone();
        let close: Run = Rc::new(move |window, cx| {
            give_back(&takeover, window, cx);
            close(window, cx)
        });
        self.on_close = close.clone();
        Backdrop::new((self.id.clone(), "scrim"))
            .on_dismiss(move |window, cx| close(window, cx))
            .child(FocusScope::new(&scope).trap().child(self.card(window, cx)))
    }

    /// The card alone, as in a window of its own.
    pub(crate) fn card(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cursor =
            window.use_keyed_state((self.id.clone(), "cursor"), cx, |_, _| Cursor::default());
        let values: Vec<SharedString> = self
            .groups
            .iter()
            .flat_map(|group| group.rows.iter().map(|row| row.value.clone()))
            .collect();
        let count = values.len();
        let text = self.input.read(cx).text().to_string();
        if cursor.read(cx).query.as_deref() != Some(text.as_str()) {
            let start = self.start.min(count.saturating_sub(1));
            cursor.update(cx, |cursor, _| {
                cursor.at = start;
                cursor.query = Some(text);
                cursor.scroll.set_offset(Point::default());
            });
        }
        let at = cursor.read(cx).at.min(count.saturating_sub(1));
        let scroll = cursor.read(cx).scroll.clone();
        let (mut slots, mut heads) = (Vec::with_capacity(count), Vec::new());
        let mut slot = 0;
        for group in &self.groups {
            if group.title.is_some() {
                slot += 1;
                heads.push(slots.len());
            }
            slots.extend(slot..slot + group.rows.len());
            slot += group.rows.len();
        }
        let slots = Rc::new((slots, heads));
        let go = {
            let (cursor, slots) = (cursor.clone(), slots.clone());
            move |by: isize, cx: &mut App| {
                if count == 0 {
                    return;
                }
                cursor.update(cx, |cursor, cx| {
                    let at = cursor.at.min(count - 1) as isize;
                    cursor.at = (at + by).rem_euclid(count as isize) as usize;
                    let (slots, heads) = &*slots;
                    let head = heads.contains(&cursor.at) && (by < 0 || cursor.at == 0);
                    cursor
                        .scroll
                        .scroll_to_item(slots[cursor.at] - usize::from(head));
                    cx.notify();
                })
            }
        };
        let pick: Pick = {
            let (id, on_pick, close) = (self.id.clone(), self.on_pick, self.on_close.clone());
            Rc::new(move |ix, window, cx| {
                let value = &values[ix];
                log::info!("palette {id:?}: picked {value}");
                close(window, cx);
                if let Some(on_pick) = &on_pick {
                    on_pick(value, window, cx);
                }
            })
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let mut children: Vec<AnyElement> = Vec::with_capacity(slot);
        let mut ix = 0;
        for group in self.groups {
            if let Some(title) = group.title {
                children.push(
                    div()
                        .px_2()
                        .pt_3()
                        .pb_1()
                        .text_size(theme.text_size(TextSize::Xs))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.fg_subtle)
                        .child(title)
                        .into_any_element(),
                );
            }
            for row in group.rows {
                let (here, pick, hover) = (ix, pick.clone(), cursor.clone());
                children.push(
                    div()
                        .id(("row", ix))
                        .role(Role::ListBoxOption)
                        .aria_selected(ix == at)
                        .aria_label(row.name.clone())
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap_3()
                        .h(theme.control_height(ControlSize::Lg))
                        .px_2()
                        .rounded(theme.radius(Radius::Md))
                        .when(ix == at, |row| row.bg(colors.hover))
                        .cursor_pointer()
                        .on_mouse_move(move |_, _, cx| {
                            if hover.read(cx).at != here {
                                hover.update(cx, |cursor, cx| {
                                    cursor.at = here;
                                    cx.notify();
                                });
                            }
                        })
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            pick(here, window, cx);
                        })
                        .when_some(row.icon, |line, icon| {
                            line.child(Icon::new(icon).size(IconSize::Sm).color(colors.fg_muted))
                        })
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(div().flex_none().child(row.label))
                                .when_some(row.detail, |line, detail| {
                                    line.child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .text_color(colors.fg_subtle)
                                            .child(detail),
                                    )
                                }),
                        )
                        .when_some(row.end, |line, end| {
                            line.child(div().flex_none().text_color(colors.fg_subtle).child(end))
                        })
                        .into_any_element(),
                );
                ix += 1;
            }
        }
        let (up, down, escape) = (go.clone(), go, self.on_close);
        let (enter, chosen) = (pick, cursor.clone());
        let card = div()
            .id(self.id.clone())
            .debug_selector(|| "palette-card".into())
            .flex()
            .flex_col()
            .w(theme.palette_size().width)
            .h(theme.palette_size().height)
            .rounded(theme.radius(Radius::Xl))
            .bg(colors.overlay)
            .border_1()
            .border_color(colors.border)
            .shadow(theme.elevation(Elevation::Modal))
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(colors.fg)
            .capture_action(move |_: &Up, _, cx| {
                cx.stop_propagation();
                up(-1, cx);
            })
            .capture_action(move |_: &Down, _, cx| {
                cx.stop_propagation();
                down(1, cx);
            })
            .capture_action(move |_: &Enter, window, cx| {
                cx.stop_propagation();
                if count > 0 {
                    enter(chosen.read(cx).at.min(count - 1), window, cx);
                }
            })
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    escape(window, cx);
                }
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .text_size(theme.text_size(TextSize::Md))
                    .child(
                        Icon::new(IconName::Search)
                            .size(IconSize::Sm)
                            .color(colors.fg_subtle),
                    )
                    .child(div().flex_1().min_w_0().child(self.input.clone())),
            )
            .child(
                div()
                    .id((self.id.clone(), "list"))
                    .role(Role::ListBox)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(children)
                    .when(count == 0, |list| {
                        list.child(
                            div()
                                .flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .text_color(colors.fg_subtle)
                                .child(self.empty),
                        )
                    }),
            );
        // Padding grows the box the scrim centers.
        div().child(card).with_animation(
            (self.id, "in"),
            Animation::new(motion::duration(motion::BASE, cx)).with_easing(motion::ease_out_cubic),
            |card, t| card.opacity(t).pt(motion::NUDGE * (1.0 - t)),
        )
    }
}
