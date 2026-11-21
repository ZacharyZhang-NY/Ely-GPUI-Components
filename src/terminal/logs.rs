use std::rc::Rc;

use gpui::{
    App, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    ScrollDelta, ScrollEvent, ScrollStrategy, SharedString, Styled, UniformListScrollHandle,
    Window, div, prelude::*, uniform_list,
};

use super::ansi::{AnsiText, printed};
use crate::{
    buttons::{SegmentedControl, ToggleButton, ToggleItem},
    primitives::IconName,
    theme::{ActiveTheme, ControlSize, Palette, TextSize},
    typography::format,
};

/// How loud a log line is, quietest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

const LEVELS: [LogLevel; 5] = [
    LogLevel::Trace,
    LogLevel::Debug,
    LogLevel::Info,
    LogLevel::Warn,
    LogLevel::Error,
];

impl LogLevel {
    fn word(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    fn color(self, colors: &Palette) -> Hsla {
        match self {
            Self::Trace | Self::Debug => colors.fg_subtle,
            Self::Info => colors.info,
            Self::Warn => colors.warning,
            Self::Error => colors.danger,
        }
    }
}

/// One line of a log: when, how loud, from where, and what it says, escape codes and all.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub at: SharedString,
    pub level: LogLevel,
    pub target: SharedString,
    pub message: SharedString,
}

/// The lines at or above `least` whose target or text holds `query`, case aside, by index.
pub(crate) fn shown(lines: &[LogLine], least: LogLevel, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.level >= least)
        .filter(|(_, line)| {
            query.is_empty()
                || line.target.to_lowercase().contains(&query)
                || printed(&line.message).0.to_lowercase().contains(&query)
        })
        .map(|(ix, _)| ix)
        .collect()
}

type OnLevel = Rc<dyn Fn(LogLevel, &mut Window, &mut App)>;
type OnBool = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A log, newest at the bottom: from a chosen level up, found by a query, following new lines until a scroll up lets go. Long logs draw only the lines in view.
#[derive(IntoElement)]
pub struct LogViewer {
    id: ElementId,
    lines: Rc<Vec<LogLine>>,
    least: LogLevel,
    query: SharedString,
    follow: bool,
    on_least: Option<OnLevel>,
    on_follow: Option<OnBool>,
}

impl LogViewer {
    pub fn new(id: impl Into<ElementId>, lines: impl Into<Rc<Vec<LogLine>>>) -> Self {
        Self {
            id: id.into(),
            lines: lines.into(),
            least: LogLevel::Trace,
            query: SharedString::default(),
            follow: true,
            on_least: None,
            on_follow: None,
        }
    }

    /// The quietest level shown.
    pub fn least(mut self, least: LogLevel) -> Self {
        self.least = least;
        self
    }

    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    pub fn on_least(mut self, handler: impl Fn(LogLevel, &mut Window, &mut App) + 'static) -> Self {
        self.on_least = Some(Rc::new(handler));
        self
    }

    pub fn on_follow(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_follow = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for LogViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scroll = window
            .use_keyed_state((self.id.clone(), "scroll"), cx, |_, _| {
                UniformListScrollHandle::new()
            })
            .read(cx)
            .clone();
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let rows = Rc::new(shown(&self.lines, self.least, &self.query));
        if self.follow && !rows.is_empty() {
            scroll.scroll_to_item(rows.len() - 1, ScrollStrategy::Bottom);
        }
        let levels = LEVELS.iter().fold(
            SegmentedControl::new((self.id.clone(), "levels"), self.least.word())
                .size(ControlSize::Sm),
            |control, level| control.segment(level.word(), level.word(), None),
        );
        let levels = match self.on_least {
            Some(on_least) => levels.on_change(move |word, window, cx| {
                let level = LEVELS
                    .into_iter()
                    .find(|level| level.word() == word.as_ref())
                    .expect("a segment names a level");
                on_least(level, window, cx)
            }),
            None => levels,
        };
        let follow = ToggleButton::new(
            (self.id.clone(), "follow"),
            ToggleItem::new("follow")
                .icon(IconName::ArrowDownToLine)
                .tooltip("Follow new lines"),
            self.follow,
        )
        .size(ControlSize::Sm);
        let (release, follow) = match self.on_follow {
            Some(on_follow) => (
                Some(on_follow.clone()),
                follow.on_toggle(move |on, window, cx| on_follow(on, window, cx)),
            ),
            None => (None, follow),
        };
        let (lines, painted) = (self.lines.clone(), rows.clone());
        let list = uniform_list(
            (self.id.clone(), "lines"),
            rows.len(),
            move |range, _, cx| {
                let colors = cx.theme().colors.clone();
                range
                    .map(|ix| {
                        let line = &lines[painted[ix]];
                        div()
                            .id(ix)
                            .w_full()
                            .flex()
                            .gap_3()
                            .px_2()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .when(line.level == LogLevel::Error, |row| {
                                row.bg(colors.danger.opacity(0.06))
                            })
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(colors.fg_subtle)
                                    .child(line.at.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w_12()
                                    .text_color(line.level.color(&colors))
                                    .child(line.level.word()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w_24()
                                    .overflow_hidden()
                                    .text_color(colors.fg_muted)
                                    .child(line.target.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(AnsiText::new(line.message.clone())),
                            )
                    })
                    .collect()
            },
        )
        .track_scroll(&scroll)
        .size_full();
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(theme.text_size(TextSize::Sm))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pb_2()
                    .child(levels)
                    .child(div().flex_1().text_color(colors.fg_subtle).child(format!(
                        "{} of {}",
                        rows.len(),
                        format::plural(self.lines.len() as u64, "line", "lines")
                    )))
                    .child(follow),
            )
            .child(
                div()
                    .id((self.id.clone(), "body"))
                    .flex_1()
                    .min_h_0()
                    .font_family(theme.mono_family.clone())
                    .text_size(theme.text_size(TextSize::Xs))
                    .when_some(release.filter(|_| self.follow), |body, on_follow| {
                        body.on_scroll(move |event: &ScrollEvent, window, cx| {
                            let back = match event.delta {
                                ScrollDelta::Pixels(delta) => delta.y > Pixels::ZERO,
                                ScrollDelta::Lines(delta) => delta.y > 0.0,
                            };
                            if back {
                                on_follow(false, window, cx)
                            }
                        })
                    })
                    .child(list),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(level: LogLevel, target: &str, message: &str) -> LogLine {
        LogLine {
            at: "12:00:00".into(),
            level,
            target: target.to_string().into(),
            message: message.to_string().into(),
        }
    }

    #[test]
    fn lines_show_from_a_level_up_and_by_query() {
        let lines = [
            line(LogLevel::Debug, "gpui", "frame"),
            line(LogLevel::Info, "ely::terminal", "started \x1b[1mzsh\x1b[0m"),
            line(LogLevel::Error, "ely::pty", "the program ended"),
        ];
        assert_eq!(shown(&lines, LogLevel::Info, ""), [1, 2]);
        assert_eq!(
            shown(&lines, LogLevel::Trace, "ZSH"),
            [1],
            "codes and case aside"
        );
        assert_eq!(shown(&lines, LogLevel::Trace, "pty"), [2], "targets count");
    }
}
