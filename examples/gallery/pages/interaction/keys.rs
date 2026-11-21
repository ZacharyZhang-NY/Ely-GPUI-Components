use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    forms::{History, Input, TextInput},
    interaction::{RovingFocus, ScrollSync},
    lists::{ListItem, SelectableList},
    primitives::{FocusRing, Icon, IconName},
    theme::{ActiveTheme, IconSize, Radius, TextSize},
    typography::{KbdCombo, format::plural},
};
use gpui::{
    App, Axis, Context, Focusable, InteractiveElement, IntoElement, KeyBinding, ParentElement,
    SharedString, Styled, Subscription, Window, actions, div, prelude::*, px, uniform_list,
};

use crate::{
    probe::probe,
    ui::{change, code, keep, section, specimen, specimens},
};

actions!(gallery_interaction, [Shout, SaveAll]);

const KEYS: &str = "InteractionKeys";

/// Binds the page's shortcut and chord under its own key context.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-shift-h", Shout, Some(KEYS)),
        KeyBinding::new("cmd-k cmd-s", SaveAll, Some(KEYS)),
    ]);
}

/// The key demo: what the bindings heard, and the first stroke of a chord while it waits.
struct Heard {
    shouts: usize,
    saves: usize,
    pending: Option<SharedString>,
    _pending: Subscription,
}

pub fn keys(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let state = window.use_keyed_state(
        "interaction-heard",
        cx,
        |window, cx: &mut Context<Heard>| Heard {
            shouts: 0,
            saves: 0,
            pending: None,
            _pending: cx.observe_pending_input(window, |heard, window, cx| {
                heard.pending = window.pending_input_keystrokes().map(|strokes| {
                    strokes
                        .iter()
                        .map(|stroke| stroke.unparse(cx.platform_info().os))
                        .collect::<Vec<_>>()
                        .join(" ")
                        .into()
                });
                cx.notify();
            }),
        },
    );
    let now = state.read(cx);
    let (shouts, saves, pending) = (now.shouts, now.saves, now.pending.clone());
    let theme = cx.theme();
    let [shout, save] = [(); 2].map(|_| state.clone());
    section(
        "Hotkeys / KeyBinding · KeyChord",
        "gpui's own bindings: actions! names them, bind_keys ties keys to them under a key context, and a binding of two strokes waits for the second while the first shows. Focus the box and try them. Ely adds nothing.",
        cx,
    )
    .child(
        probe(
            "interaction-keys",
            div()
                .id("interaction-keys")
                .key_context(KEYS)
                .w(px(420.))
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(theme.radius(Radius::Lg))
                .border_1()
                .border_color(theme.colors.border)
                .tab_index(0)
                .focus_ring(cx)
                .on_action(move |_: &Shout, _, cx| change(&shout, cx, |heard| heard.shouts += 1))
                .on_action(move |_: &SaveAll, _, cx| change(&save, cx, |heard| heard.saves += 1))
                .child(div().flex().items_center().gap_2().child(KbdCombo::new("cmd-shift-h")).child(format!("heard {}", plural(shouts as u64, "time", "times"))))
                .child(div().flex().items_center().gap_2().child(KbdCombo::new("cmd-k cmd-s")).child(format!("saved {}", plural(saves as u64, "time", "times"))))
                .child(
                    div()
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(theme.colors.fg_muted)
                        .child(match pending {
                            Some(first) => format!("{first} … waiting for the second key"),
                            None => "No chord waiting.".to_string(),
                        }),
                ),
        ),
    )
    .child(code("actions!(…) · cx.bind_keys([KeyBinding::new(\"cmd-k cmd-s\", SaveAll, Some(\"InteractionKeys\"))]) · .key_context(…) · .on_action(…)", cx))
}

/// The undo demo: a count, and its history.
#[derive(Default)]
struct Tally {
    count: i32,
    history: History<i32>,
}

pub fn undo(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let state = keep("interaction-tally", Tally::default, window, cx);
    let now = state.read(cx);
    let (count, can_undo, can_redo) = (now.count, now.history.can_undo(), now.history.can_redo());
    let [add, take, back, again] = [(); 4].map(|_| state.clone());
    let theme = cx.theme();
    section(
        "Undo / Redo Manager",
        "forms::History keeps whole snapshots, the one every editor here undoes with; a burst of typing makes one step.",
        cx,
    )
    .child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().w(px(64.)).text_size(theme.text_size(TextSize::Lg)).child(count.to_string()))
            .child(Button::new("interaction-add", "Add one").on_click(move |_, _, cx| {
                change(&add, cx, |tally| {
                    tally.history.record(tally.count, false);
                    tally.count += 1;
                })
            }))
            .child(Button::new("interaction-take", "Take one").on_click(move |_, _, cx| {
                change(&take, cx, |tally| {
                    tally.history.record(tally.count, false);
                    tally.count -= 1;
                })
            }))
            .child(
                IconButton::new("interaction-undo", IconName::Undo2)
                    .variant(ButtonVariant::Ghost)
                    .tooltip("Undo")
                    .disabled(!can_undo)
                    .on_click(move |_, _, cx| {
                        change(&back, cx, |tally| {
                            let previous = tally.history.undo(tally.count).expect("Undo rests with no step");
                            tally.count = previous;
                        })
                    }),
            )
            .child(
                IconButton::new("interaction-redo", IconName::Redo2)
                    .variant(ButtonVariant::Ghost)
                    .tooltip("Redo")
                    .disabled(!can_redo)
                    .on_click(move |_, _, cx| {
                        change(&again, cx, |tally| {
                            let next = tally.history.redo(tally.count).expect("Redo rests with no step");
                            tally.count = next;
                        })
                    }),
            ),
    )
}

pub fn scrolling(cx: &mut App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (border, muted) = (theme.colors.border, theme.colors.fg_muted);
    let text = theme.text_size(TextSize::Sm);
    let lines = |side: &'static str| {
        div().flex().flex_col().children((1..=40).map(move |n| {
            div()
                .px_3()
                .py_1()
                .text_size(text)
                .child(format!("{side} line {n}"))
        }))
    };
    section(
        "VirtualScroller · ScrollSync · InfiniteScroll Trigger",
        "gpui's uniform_list draws only the rows in view, ten thousand here. Two panes scroll as one: a wheel on either takes the other along. An endless list is lists::InfiniteList.",
        cx,
    )
    .child(
        specimens()
            .child(specimen(
                "uniform_list",
                div().w(px(220.)).h(px(200.)).rounded_lg().border_1().border_color(border).child(
                    uniform_list("interaction-rows", 10_000, move |range, _, _| {
                        range
                            .map(|ix| div().w_full().px_3().py_1().text_size(text).text_color(muted).child(format!("Row {}", ix + 1)))
                            .collect()
                    })
                    .h_full(),
                ),
                cx,
            ))
            .child(specimen(
                "ScrollSync",
                ScrollSync::new("interaction-sync", Axis::Vertical)
                    .w(px(360.))
                    .h(px(200.))
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .pane(lines("Before"))
                    .pane(div().border_l_1().border_color(border).child(lines("After"))),
                cx,
            )),
    )
    .child(code("uniform_list(id, count, |range, window, cx| rows)", cx))
}

/// The focus demos: whether the field shows, the tool pressed, and the fruit picked.
struct Focusing {
    field: bool,
    tool: SharedString,
    fruit: Vec<SharedString>,
}

pub fn focus(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let state = keep(
        "interaction-focusing",
        || Focusing {
            field: false,
            tool: "bold".into(),
            fruit: vec!["apple".into()],
        },
        window,
        cx,
    );
    let field = window.use_keyed_state("interaction-autofocus", cx, |window, cx| {
        TextInput::new(window, cx).placeholder("Focused as it appears")
    });
    let now = state.read(cx);
    let (shown, tool, fruit) = (now.field, now.tool.clone(), now.fruit.clone());
    let [show, press, pick] = [(); 3].map(|_| state.clone());
    let focus = field.read(cx).focus_handle(cx);
    let theme = cx.theme();
    let tools = [
        ("bold", IconName::Bold),
        ("italic", IconName::Italic),
        ("underline", IconName::Underline),
        ("strike", IconName::Strikethrough),
    ];
    let toolbar = tools.into_iter().fold(
        RovingFocus::new("interaction-tools", Axis::Horizontal, move |key, _, cx| {
            let key = key.clone();
            change(&press, cx, |focusing| focusing.tool = key)
        })
        .gap_0p5()
        .p_0p5()
        .rounded_lg()
        .border_1()
        .border_color(theme.colors.border),
        |bar, (key, icon)| {
            let on = tool.as_ref() == key;
            bar.item(
                key,
                div()
                    .p_1p5()
                    .rounded_md()
                    .when(on, |tool| tool.bg(theme.colors.active))
                    .child(Icon::new(icon).size(IconSize::Sm).color(if on {
                        theme.colors.fg
                    } else {
                        theme.colors.fg_muted
                    })),
            )
        },
    );
    const FRUIT: [&str; 6] = ["Apple", "Apricot", "Banana", "Blueberry", "Cherry", "Grape"];
    let list = FRUIT.iter().fold(
        SelectableList::new("interaction-fruit")
            .w(px(200.))
            .h(px(180.)),
        |list, name| {
            list.row(
                name.to_lowercase(),
                ListItem::new(
                    SharedString::from(format!("interaction-fruit-{name}")),
                    *name,
                ),
            )
        },
    );
    section(
        "Autofocus · RovingTabIndex · TypeAhead · FocusVisible",
        "A field takes focus as it appears through gpui's window.focus. A toolbar keeps one Tab stop and the arrows walk it. Typing a row's first letters moves a list to it. The focus ring is primitives::FocusRing.",
        cx,
    )
    .child(
        specimens()
            .child(specimen(
                "Autofocus",
                div().w(px(240.)).flex().flex_col().gap_2().child(
                    Button::new("interaction-show-field", if shown { "Hide the field" } else { "Show a field" })
                        .variant(ButtonVariant::Secondary)
                        .on_click(move |_, window, cx| {
                            change(&show, cx, |focusing| focusing.field = !focusing.field);
                            let focus = focus.clone();
                            window.defer(cx, move |window, cx| window.focus(&focus, cx));
                        }),
                )
                .when(shown, |column| column.child(Input::new(&field))),
                cx,
            ))
            .child(specimen(
                "RovingFocus",
                probe(
                    "interaction-tools",
                    toolbar,
                ),
                cx,
            ))
            .child(specimen(
                "TypeAhead → lists::SelectableList",
                probe(
                    "interaction-fruit",
                    list.selected(fruit).on_change(move |keys, _, cx| {
                        let keys = keys.to_vec();
                        change(&pick, cx, |focusing| focusing.fruit = keys)
                    }),
                ),
                cx,
            )),
    )
}
