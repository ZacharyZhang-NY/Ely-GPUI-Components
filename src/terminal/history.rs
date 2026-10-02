use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};

use crate::{
    i18n,
    navigation::{Group, Palette, Row, fuzzy, marked, query_field},
    primitives::IconName,
    theme::ActiveTheme,
};

/// A command run before: what, where, when, and how it ended when that is known.
#[derive(Clone, Debug, PartialEq)]
pub struct PastCommand {
    pub command: SharedString,
    pub cwd: SharedString,
    pub when: SharedString,
    pub code: Option<i32>,
}

type OnPick = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// Commands run before, newest first, found by any of their letters and picked to run again: a palette over the terminal.
#[derive(IntoElement)]
pub struct CommandHistory {
    id: ElementId,
    commands: Vec<PastCommand>,
    on_pick: Option<OnPick>,
    on_close: Run,
}

impl CommandHistory {
    /// `commands` newest first.
    pub fn new(
        id: impl Into<ElementId>,
        commands: impl IntoIterator<Item = PastCommand>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            commands: commands.into_iter().collect(),
            on_pick: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn on_pick(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandHistory {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input = query_field(
            &self.id,
            i18n::text(cx, "palette.history.placeholder", &[]),
            window,
            cx,
        );
        let query = input.read(cx).text().trim().to_string();
        let colors = cx.theme().colors.clone();
        let mut found: Vec<(i32, Row)> = self
            .commands
            .iter()
            .filter_map(|past| {
                let (score, hits) = if query.is_empty() {
                    (0, Vec::new())
                } else {
                    let fit = fuzzy(&query, &past.command)?;
                    (fit.score, fit.hits)
                };
                let icon = match past.code {
                    Some(0) => IconName::CircleCheck,
                    Some(_) => IconName::CircleX,
                    None => IconName::History,
                };
                Some((
                    score,
                    Row {
                        value: past.command.clone(),
                        name: past.command.clone(),
                        icon: Some(icon),
                        label: marked(past.command.clone(), hits, cx),
                        detail: Some(
                            div()
                                .text_color(colors.fg_subtle)
                                .child(past.cwd.clone())
                                .into_any_element(),
                        ),
                        end: Some(
                            div()
                                .text_color(colors.fg_subtle)
                                .child(past.when.clone())
                                .into_any_element(),
                        ),
                    },
                ))
            })
            .collect();
        if !query.is_empty() {
            found.sort_by(|(a, _), (b, _)| b.cmp(a));
        }
        Palette {
            id: self.id,
            input,
            groups: vec![Group {
                title: None,
                rows: found.into_iter().map(|(_, row)| row).collect(),
            }],
            start: 0,
            empty: i18n::text(cx, "palette.history.empty", &[]),
            on_pick: self.on_pick,
            on_close: self.on_close,
        }
        .overlay(window, cx)
    }
}
