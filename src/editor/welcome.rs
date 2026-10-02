use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*,
};

use crate::{
    buttons::{ButtonVariant, IconButton},
    forms::Checkbox,
    i18n,
    navigation::{Group, Palette, Row, fuzzy, marked, query_field},
    primitives::{Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::{Ellipsis, KbdCombo},
};

/// A project opened before: its name, where it lives, its branch, when it was last opened, and whether it is pinned.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub name: SharedString,
    pub path: SharedString,
    pub branch: Option<SharedString>,
    pub opened: SharedString,
    pub pinned: bool,
}

/// Pinned projects first, then the rest in the order given.
pub(crate) fn ordered(projects: &[Project]) -> Vec<(usize, &Project)> {
    let mut all: Vec<(usize, &Project)> = projects.iter().enumerate().collect();
    all.sort_by_key(|(_, project)| !project.pinned);
    all
}

type OnIndex = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Projects opened before, pinned ones first: each with its path, branch and when; a press opens one, the pin keeps it on top.
#[derive(IntoElement)]
pub struct RecentProjects {
    id: ElementId,
    projects: Vec<Project>,
    on_open: Option<OnIndex>,
    on_pin: Option<OnIndex>,
    on_remove: Option<OnIndex>,
}

impl RecentProjects {
    pub fn new(id: impl Into<ElementId>, projects: impl IntoIterator<Item = Project>) -> Self {
        Self {
            id: id.into(),
            projects: projects.into_iter().collect(),
            on_open: None,
            on_pin: None,
            on_remove: None,
        }
    }

    pub fn on_open(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    pub fn on_pin(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pin = Some(Rc::new(handler));
        self
    }

    pub fn on_remove(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RecentProjects {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let rows = ordered(&self.projects).into_iter().map(|(ix, project)| {
            let (open, pin, remove) = (
                self.on_open.clone(),
                self.on_pin.clone(),
                self.on_remove.clone(),
            );
            let button = |key: &'static str,
                          icon: IconName,
                          words: SharedString,
                          action: Option<OnIndex>| {
                IconButton::new((self.id.clone(), format!("{key}-{ix}")), icon)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip(words)
                    .when_some(action, move |button, action| {
                        button.on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            action(ix, window, cx)
                        })
                    })
            };
            let (pin_icon, pin_tip) = if project.pinned {
                (
                    IconName::PinOff,
                    i18n::text(cx, "palette.project.unpin", &[]),
                )
            } else {
                (IconName::Pin, i18n::text(cx, "palette.project.pin", &[]))
            };
            let remove_tip = i18n::text(cx, "palette.project.remove", &[]);
            div()
                .id((self.id.clone(), format!("project-{ix}")))
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .py_1p5()
                .rounded(theme.radius(Radius::Md))
                .cursor_pointer()
                .hover(|row| row.bg(colors.hover))
                .when_some(open, |row, open| {
                    row.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                        .on_click(move |_, window, cx| {
                            log::info!("recent projects: open {ix}");
                            open(ix, window, cx)
                        })
                })
                .child(
                    Icon::new(IconName::Folder)
                        .size(IconSize::Md)
                        .color(colors.fg_muted),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors.fg)
                                .child(project.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(theme.text_size(TextSize::Xs))
                                .text_color(colors.fg_subtle)
                                .child(Ellipsis::new(project.path.clone())),
                        ),
                )
                .children(project.branch.clone().map(|branch| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(theme.text_size(TextSize::Xs))
                        .text_color(colors.fg_muted)
                        .child(
                            Icon::new(IconName::GitBranch)
                                .size(IconSize::Xs)
                                .color(colors.fg_subtle),
                        )
                        .child(branch)
                }))
                .child(
                    div()
                        .text_size(theme.text_size(TextSize::Xs))
                        .text_color(colors.fg_subtle)
                        .child(project.opened.clone()),
                )
                .child(button("pin", pin_icon, pin_tip, pin))
                .child(button("remove", IconName::X, remove_tip, remove))
        });
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .text_size(theme.text_size(TextSize::Sm))
            .children(rows)
    }
}

type OnClose = Rc<dyn Fn(&mut Window, &mut App)>;

/// A palette of projects by name or path, pinned ones first; a pick opens it.
#[derive(IntoElement)]
pub struct ProjectSwitcher {
    id: ElementId,
    projects: Vec<Project>,
    on_open: Option<OnIndex>,
    on_close: OnClose,
}

impl ProjectSwitcher {
    /// Render it while open; `on_close` runs on Escape, a click outside, or a pick.
    pub fn new(
        id: impl Into<ElementId>,
        projects: impl IntoIterator<Item = Project>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            projects: projects.into_iter().collect(),
            on_open: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn on_open(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ProjectSwitcher {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input = query_field(
            &self.id,
            i18n::text(cx, "palette.project.placeholder", &[]),
            window,
            cx,
        );
        let query = input.read(cx).text().trim().to_string();
        let colors = cx.theme().colors.clone();
        let mut found: Vec<(i32, Row)> = ordered(&self.projects)
            .into_iter()
            .filter_map(|(ix, project)| {
                let (score, hits) = if query.is_empty() {
                    (0, Vec::new())
                } else {
                    fit(&query, project)?
                };
                Some((
                    score,
                    Row {
                        value: ix.to_string().into(),
                        name: project.name.clone(),
                        icon: Some(if project.pinned {
                            IconName::Pin
                        } else {
                            IconName::Folder
                        }),
                        label: marked(project.name.clone(), hits, cx),
                        detail: Some(
                            div()
                                .text_color(colors.fg_subtle)
                                .child(project.path.clone())
                                .into_any_element(),
                        ),
                        end: Some(
                            div()
                                .text_color(colors.fg_subtle)
                                .child(project.opened.clone())
                                .into_any_element(),
                        ),
                    },
                ))
            })
            .collect();
        if !query.is_empty() {
            found.sort_by(|(a, _), (b, _)| b.cmp(a));
        }
        let on_pick = self.on_open.map(|open| {
            Rc::new(
                move |value: &SharedString, window: &mut Window, cx: &mut App| {
                    open(
                        value.parse().expect("a project row carries its place"),
                        window,
                        cx,
                    )
                },
            ) as Rc<dyn Fn(&SharedString, &mut Window, &mut App)>
        });
        Palette {
            id: self.id,
            input,
            groups: vec![Group {
                title: None,
                rows: found.into_iter().map(|(_, row)| row).collect(),
            }],
            start: 0,
            empty: i18n::text(cx, "palette.project.empty", &[]),
            on_pick,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}

/// How well `query` fits a project, by name or else by path, and the name's letters it marks; a path match marks none.
fn fit(query: &str, project: &Project) -> Option<(i32, Vec<std::ops::Range<usize>>)> {
    if let Some(fit) = fuzzy(query, &project.name) {
        return Some((fit.score, fit.hits));
    }
    fuzzy(query, &project.path).map(|fit| (fit.score, Vec::new()))
}

/// A way to start from the welcome page: its icon, words and keys.
#[derive(Clone, Debug, PartialEq)]
pub struct StartAction {
    pub icon: IconName,
    pub label: SharedString,
    pub keys: Option<SharedString>,
}

type OnFlag = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The first page an editor shows: ways to start, recent projects, and whether to show it at startup.
#[derive(IntoElement)]
pub struct WelcomePage {
    id: ElementId,
    title: SharedString,
    tagline: SharedString,
    actions: Vec<StartAction>,
    recent: Option<AnyElement>,
    show_at_start: bool,
    on_action: Option<OnIndex>,
    on_show: Option<OnFlag>,
}

impl WelcomePage {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        tagline: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tagline: tagline.into(),
            actions: Vec::new(),
            recent: None,
            show_at_start: true,
            on_action: None,
            on_show: None,
        }
    }

    pub fn action(
        mut self,
        icon: IconName,
        label: impl Into<SharedString>,
        keys: Option<&str>,
    ) -> Self {
        self.actions.push(StartAction {
            icon,
            label: label.into(),
            keys: keys.map(|keys| keys.to_string().into()),
        });
        self
    }

    /// Recent projects, usually a `RecentProjects`.
    pub fn recent(mut self, recent: impl IntoElement) -> Self {
        self.recent = Some(recent.into_any_element());
        self
    }

    pub fn show_at_start(mut self, show: bool) -> Self {
        self.show_at_start = show;
        self
    }

    pub fn on_action(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    pub fn on_show(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_show = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for WelcomePage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let heading = |words: &'static str| {
            div()
                .text_size(theme.text_size(TextSize::Xs))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors.fg_subtle)
                .child(words)
        };
        let actions = self.actions.iter().enumerate().map(|(ix, action)| {
            let pick = self.on_action.clone();
            div()
                .id((self.id.clone(), format!("action-{ix}")))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(theme.radius(Radius::Md))
                .cursor_pointer()
                .text_color(colors.link)
                .hover(|row| row.bg(colors.hover))
                .when_some(pick, |row, pick| {
                    row.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                        .on_click(move |_, window, cx| pick(ix, window, cx))
                })
                .child(Icon::new(action.icon).size(IconSize::Sm).color(colors.link))
                .child(div().flex_1().child(action.label.clone()))
                .children(action.keys.as_ref().map(|keys| KbdCombo::new(keys)))
        });
        let show = Checkbox::new((self.id.clone(), "show"), self.show_at_start)
            .label("Show this page at startup");
        let show = match self.on_show {
            Some(on_show) => show.on_change(move |on, window, cx| on_show(on, window, cx)),
            None => show,
        };
        div()
            .flex()
            .flex_col()
            .gap_8()
            .p_8()
            .text_size(theme.text_size(TextSize::Sm))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme.text_size(TextSize::Xxl))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.fg)
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(theme.text_size(TextSize::Md))
                            .text_color(colors.fg_muted)
                            .child(self.tagline),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_12()
                    .child(
                        div()
                            .w(theme.label_width() * 2.5)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(heading("Start"))
                            .children(actions),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(heading("Recent"))
                            .children(self.recent),
                    ),
            )
            .child(show)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_name_match_marks_the_name() {
        let project = Project {
            name: "中文".into(),
            path: "foo".into(),
            branch: None,
            opened: "now".into(),
            pinned: false,
        };
        assert_eq!(fit("oo", &project).map(|(_, hits)| hits), Some(Vec::new()));
        assert!(fit("zz", &project).is_none());
    }

    #[test]
    fn pinned_projects_lead_and_keep_their_places() {
        let project = |name: &str, pinned: bool| Project {
            name: name.to_string().into(),
            path: "~".into(),
            branch: None,
            opened: "now".into(),
            pinned,
        };
        let projects = [project("a", false), project("b", true), project("c", false)];
        let order: Vec<usize> = ordered(&projects).into_iter().map(|(ix, _)| ix).collect();
        assert_eq!(order, [1, 0, 2]);
    }
}
