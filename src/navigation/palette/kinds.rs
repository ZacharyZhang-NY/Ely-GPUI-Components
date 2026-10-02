use std::{iter, ops::Range, rc::Rc};

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window};

use super::{Fit, Group, Palette, Row, fuzzy, marked, query_field};
use crate::{
    forms::{Choice, OnValue, Run},
    i18n,
    primitives::IconName,
    typography::{Highlight, KbdCombo},
};

/// Items that fit `query`, best first; ties go to the shorter text, then the given order.
fn ranked<T>(
    items: impl IntoIterator<Item = T>,
    query: &str,
    text: impl Fn(&T) -> &str,
) -> Vec<(T, Fit)> {
    let mut fits: Vec<(T, Fit)> = items
        .into_iter()
        .filter_map(|item| fuzzy(query, text(&item)).map(|fit| (item, fit)))
        .collect();
    fits.sort_by(|(a, fa), (b, fb)| {
        fb.score
            .cmp(&fa.score)
            .then(text(a).len().cmp(&text(b).len()))
    });
    fits
}

/// A command a palette runs.
#[derive(Clone, Debug)]
pub struct Command {
    pub value: SharedString,
    pub label: SharedString,
    pub icon: Option<IconName>,
    pub keys: Option<SharedString>,
}

impl Command {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            keys: None,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Its chord in gpui key syntax, such as `"secondary-shift-p"`.
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    fn row(&self, hits: Vec<Range<usize>>, cx: &App) -> Row {
        Row {
            value: self.value.clone(),
            name: self.label.clone(),
            icon: self.icon,
            label: marked(self.label.clone(), hits, cx),
            detail: None,
            end: self
                .keys
                .as_ref()
                .map(|keys| KbdCombo::new(keys).into_any_element()),
        }
    }
}

/// Commands in groups. Recent ones lead while the query is empty; a query ranks fuzzy matches and marks the letters that matched.
#[derive(IntoElement)]
pub struct CommandPalette {
    id: ElementId,
    groups: Vec<(SharedString, Vec<Command>)>,
    recent: Vec<SharedString>,
    on_run: Option<OnValue>,
    on_close: Run,
}

impl CommandPalette {
    /// Render it while open; `on_close` runs on Escape, a click outside, or a run.
    pub fn new(
        id: impl Into<ElementId>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            groups: Vec::new(),
            recent: Vec::new(),
            on_run: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn group(
        mut self,
        title: impl Into<SharedString>,
        commands: impl IntoIterator<Item = Command>,
    ) -> Self {
        self.groups
            .push((title.into(), commands.into_iter().collect()));
        self
    }

    /// Values of recent commands, newest first.
    pub fn recent(mut self, values: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.recent = values.into_iter().map(Into::into).collect();
        self
    }

    pub fn on_run(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let placeholder = i18n::text(cx, "palette.command.placeholder", &[]);
        let input = query_field(&self.id, placeholder, window, cx);
        let query = input.read(cx).text().trim().to_string();
        let groups = if query.is_empty() {
            let find = |value: &SharedString| {
                self.groups
                    .iter()
                    .flat_map(|(_, commands)| commands)
                    .find(|command| command.value == *value)
                    .unwrap_or_else(|| panic!("recent command {value} is not in the palette"))
            };
            let recent = Group {
                title: Some(i18n::text(cx, "palette.recent", &[])),
                rows: self
                    .recent
                    .iter()
                    .map(|value| find(value).row(Vec::new(), cx))
                    .collect(),
            };
            iter::once(recent)
                .chain(self.groups.iter().map(|(title, commands)| {
                    Group {
                        title: Some(title.clone()),
                        rows: commands
                            .iter()
                            .filter(|command| !self.recent.contains(&command.value))
                            .map(|command| command.row(Vec::new(), cx))
                            .collect(),
                    }
                }))
                .filter(|group| !group.rows.is_empty())
                .collect()
        } else {
            let mut found: Vec<(i32, Group)> = self
                .groups
                .iter()
                .filter_map(|(title, commands)| {
                    let fits = ranked(commands.iter(), &query, |command| &command.label[..]);
                    let best = fits.first()?.1.score;
                    let rows = fits
                        .into_iter()
                        .map(|(command, fit)| command.row(fit.hits, cx))
                        .collect();
                    Some((
                        best,
                        Group {
                            title: Some(title.clone()),
                            rows,
                        },
                    ))
                })
                .collect();
            found.sort_by(|(a, _), (b, _)| b.cmp(a));
            found.into_iter().map(|(_, group)| group).collect()
        };
        Palette {
            id: self.id,
            input,
            groups,
            start: 0,
            empty: i18n::text(cx, "palette.command.empty", &[]),
            on_pick: self.on_run,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}

// ponytail: the best 50; a longer query reaches the rest.
const SHOWN: usize = 50;

/// Files by path. A query ranks fuzzy matches over the whole path; recent files lead while it is empty.
#[derive(IntoElement)]
pub struct QuickOpen {
    id: ElementId,
    paths: Vec<SharedString>,
    recent: Vec<SharedString>,
    on_open: Option<OnValue>,
    on_close: Run,
}

impl QuickOpen {
    pub fn new(
        id: impl Into<ElementId>,
        paths: impl IntoIterator<Item = impl Into<SharedString>>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            paths: paths.into_iter().map(Into::into).collect(),
            recent: Vec::new(),
            on_open: None,
            on_close: Rc::new(on_close),
        }
    }

    /// Recent paths, newest first.
    pub fn recent(mut self, paths: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.recent = paths.into_iter().map(Into::into).collect();
        self
    }

    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

/// A file row: its name, then its folder, each with the hits that fall in it.
fn file_row(path: &SharedString, hits: &[Range<usize>], cx: &App) -> Row {
    let name_at = path.rfind('/').map_or(0, |slash| slash + 1);
    let within = |from: usize, to: usize| -> Vec<Range<usize>> {
        hits.iter()
            .map(|hit| hit.start.max(from)..hit.end.min(to))
            .filter(|hit| hit.start < hit.end)
            .map(|hit| hit.start - from..hit.end - from)
            .collect()
    };
    let folder = &path[..name_at.saturating_sub(1)];
    Row {
        value: path.clone(),
        name: path[name_at..].to_string().into(),
        icon: Some(IconName::File),
        label: marked(
            path[name_at..].to_string().into(),
            within(name_at, path.len()),
            cx,
        ),
        detail: (!folder.is_empty())
            .then(|| marked(folder.to_string().into(), within(0, folder.len()), cx)),
        end: None,
    }
}

impl RenderOnce for QuickOpen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let placeholder = i18n::text(cx, "palette.open.placeholder", &[]);
        let input = query_field(&self.id, placeholder, window, cx);
        let query = input.read(cx).text().trim().to_string();
        let (group, empty) = if query.is_empty() {
            let rows = self
                .recent
                .iter()
                .map(|path| file_row(path, &[], cx))
                .collect();
            (
                Group {
                    title: Some(i18n::text(cx, "palette.recent", &[])),
                    rows,
                },
                i18n::text(cx, "palette.open.idle", &[]),
            )
        } else {
            let rows = ranked(self.paths.iter(), &query, |path| &path[..])
                .into_iter()
                .take(SHOWN)
                .map(|(path, fit)| file_row(path, &fit.hits, cx))
                .collect();
            (
                Group { title: None, rows },
                i18n::text(cx, "palette.open.empty", &[]),
            )
        };
        Palette {
            id: self.id,
            input,
            groups: vec![group],
            start: 0,
            empty,
            on_pick: self.on_open,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}

/// Documents, sessions and projects, most recent first. It opens on the one before the current, so Enter goes back.
#[derive(IntoElement)]
pub struct QuickSwitcher {
    id: ElementId,
    items: Vec<Choice>,
    on_switch: Option<OnValue>,
    on_close: Run,
}

impl QuickSwitcher {
    /// Each item's note says what it is.
    pub fn new(
        id: impl Into<ElementId>,
        items: impl IntoIterator<Item = Choice>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        let items: Vec<Choice> = items.into_iter().collect();
        assert!(
            items.iter().all(|item| !item.disabled),
            "a switcher cannot hold disabled items"
        );
        Self {
            id: id.into(),
            items,
            on_switch: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn on_switch(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_switch = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for QuickSwitcher {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let placeholder = i18n::text(cx, "palette.switch.placeholder", &[]);
        let input = query_field(&self.id, placeholder, window, cx);
        let query = input.read(cx).text().trim().to_string();
        let row = |item: &Choice, hits: Vec<Range<usize>>| Row {
            value: item.value.clone(),
            name: item.label.clone(),
            icon: item.icon,
            label: marked(item.label.clone(), hits, cx),
            detail: None,
            end: item.note.clone().map(|note| note.into_any_element()),
        };
        let (rows, start) = if query.is_empty() {
            (
                self.items
                    .iter()
                    .map(|item| row(item, Vec::new()))
                    .collect(),
                1,
            )
        } else {
            let fits = ranked(self.items.iter(), &query, |item| &item.label[..]);
            (
                fits.into_iter()
                    .map(|(item, fit)| row(item, fit.hits))
                    .collect(),
                0,
            )
        };
        Palette {
            id: self.id,
            input,
            groups: vec![Group { title: None, rows }],
            start,
            empty: i18n::text(cx, "palette.switch.empty", &[]),
            on_pick: self.on_switch,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}

pub(crate) type Search = Rc<dyn Fn(&str) -> Vec<(SharedString, Vec<Choice>)>>;

/// Searches through the owner's function, which answers a query with results grouped by where they were found. Matches are marked.
#[derive(IntoElement)]
pub struct SearchPalette {
    id: ElementId,
    search: Search,
    on_open: Option<OnValue>,
    on_close: Run,
}

impl SearchPalette {
    /// `search` answers the empty query too, with what shows first. Results cannot be disabled.
    pub fn new(
        id: impl Into<ElementId>,
        search: impl Fn(&str) -> Vec<(SharedString, Vec<Choice>)> + 'static,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            search: Rc::new(search),
            on_open: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    pub(crate) fn from_parts(
        id: ElementId,
        search: Search,
        on_open: Option<OnValue>,
        on_close: Run,
    ) -> Self {
        Self {
            id,
            search,
            on_open,
            on_close,
        }
    }

    pub(crate) fn palette(self, window: &mut Window, cx: &mut App) -> Palette {
        let placeholder = i18n::text(cx, "palette.search.placeholder", &[]);
        let input = query_field(&self.id, placeholder, window, cx);
        let query = input.read(cx).text().trim().to_string();
        let groups = (self.search)(&query)
            .into_iter()
            .filter(|(_, results)| !results.is_empty())
            .map(|(title, results)| Group {
                title: Some(title),
                rows: results
                    .into_iter()
                    .inspect(|result| {
                        assert!(
                            !result.disabled,
                            "search result {} is disabled",
                            result.value
                        );
                    })
                    .map(|result| Row {
                        value: result.value.clone(),
                        name: result.label.clone(),
                        icon: result.icon,
                        label: Highlight::matching(result.label, &query).into_any_element(),
                        detail: result
                            .note
                            .map(|note| Highlight::matching(note, &query).into_any_element()),
                        end: None,
                    })
                    .collect(),
            })
            .collect();
        Palette {
            id: self.id,
            input,
            groups,
            start: 0,
            empty: if query.is_empty() {
                i18n::text(cx, "palette.search.idle", &[])
            } else {
                i18n::text(cx, "palette.search.empty", &[("query", &query)])
            },
            on_pick: self.on_open,
            on_close: self.on_close,
        }
    }
}

impl RenderOnce for SearchPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.palette(window, cx).overlay(window, cx)
    }
}
