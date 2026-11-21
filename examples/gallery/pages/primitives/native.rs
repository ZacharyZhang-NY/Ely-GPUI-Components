use std::f32::consts::TAU;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    primitives::{FocusRing, Icon, IconName},
    theme::{ActiveTheme, Radius, TextSize},
};
use gpui::{
    AnyElement, App, ClipboardItem, FontWeight, HighlightStyle, InteractiveElement, IntoElement,
    ParentElement, PathBuilder, SharedString, StatefulInteractiveElement, Styled, StyledText,
    Transformation, Window, anchored, canvas, deferred, div, point, prelude::*, px, radians, svg,
};

use crate::{
    probe::probe,
    ui::{code, row, section, set},
};

pub fn boxes(cx: &App) -> impl IntoElement + use<> {
    let colors = &cx.theme().colors;
    section(
        "Box / View",
        "gpui's div() is the box. Ely adds nothing.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(
                div()
                    .size_16()
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.surface),
            )
            .child(div().w_32().h_16().rounded_lg().bg(colors.sunken))
            .child(code("div()", cx)),
    )
}

pub fn text(cx: &App) -> impl IntoElement + use<> {
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    section("Text", "A string is an element. StyledText adds runs.", cx).child(
        row()
            .gap_6()
            .child("Plain text, as a child.")
            .child(StyledText::new("One accent, used once.").with_highlights([(4..10, bold)]))
            .child(code("StyledText::new(…)", cx)),
    )
}

pub fn svg_demo(cx: &App) -> impl IntoElement + use<> {
    let color = cx.theme().colors.fg_muted;
    section(
        "Svg",
        "gpui's svg(). Any path from the asset source, tinted.",
        cx,
    )
    .child(
        row()
            .gap_6()
            .child(
                svg()
                    .path(IconName::Compass.path())
                    .size_12()
                    .text_color(color),
            )
            .child(
                svg()
                    .path(IconName::Compass.path())
                    .size_12()
                    .text_color(color)
                    .with_transformation(Transformation::rotate(radians(0.6))),
            )
            .child(code("svg().path(…)", cx)),
    )
}

pub fn canvas_demo(cx: &App) -> impl IntoElement + use<> {
    let color = cx.theme().colors.focus;
    section("Canvas", "gpui's canvas(). Paint with paths.", cx).child(
        row()
            .gap_6()
            .child(
                div().w(px(320.0)).h(px(80.0)).child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let mut wave = PathBuilder::stroke(px(1.5));
                            for step in 0..=96 {
                                let t = step as f32 / 96.0;
                                let x = bounds.origin.x + bounds.size.width * t;
                                let lift = 0.5 - 0.38 * (t * TAU * 1.5).sin() * (1.0 - t * 0.5);
                                let y = bounds.origin.y + bounds.size.height * lift;
                                if step == 0 {
                                    wave.move_to(point(x, y));
                                } else {
                                    wave.line_to(point(x, y));
                                }
                            }
                            window.paint_path(wave.build().expect("wave path"), color);
                        },
                    )
                    .size_full(),
                ),
            )
            .child(code("canvas(prepaint, paint)", cx)),
    )
}

pub fn spacer(cx: &App) -> impl IntoElement + use<> {
    let colors = &cx.theme().colors;
    section(
        "Spacer",
        "flex_1() takes the slack. No component needed.",
        cx,
    )
    .child(
        div()
            .flex()
            .items_center()
            .w(px(420.0))
            .h_10()
            .px_3()
            .rounded(cx.theme().radius(Radius::Md))
            .border_1()
            .border_color(colors.border)
            .child(Icon::new(IconName::Menu).color(colors.fg_muted))
            .child(div().flex_1())
            .child(code("div().flex_1()", cx))
            .child(div().flex_1())
            .child(Icon::new(IconName::Settings).color(colors.fg_muted)),
    )
}

pub fn portal(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let open = window.use_keyed_state("prim-portal", cx, |_, _| false);
    let shown = *open.read(cx);
    let theme = cx.theme();
    let note = div()
        .w(px(260.0))
        .p_3()
        .rounded(theme.radius(Radius::Lg))
        .bg(theme.colors.overlay)
        .border_1()
        .border_color(theme.colors.border)
        .shadow(theme.elevation(ely_gpui_component::theme::Elevation::Floating))
        .text_size(theme.text_size(TextSize::Sm))
        .child("Painted last, above the clipped box.");
    section(
        "Portal",
        "deferred(anchored()) escapes clipping and paints on top.",
        cx,
    )
    .child(
        row()
            .gap_6()
            .child(
                div()
                    .flex()
                    .items_start()
                    .w(px(220.0))
                    .h_16()
                    .p_3()
                    .overflow_hidden()
                    .rounded(theme.radius(Radius::Md))
                    .border_1()
                    .border_color(theme.colors.border)
                    .child(probe(
                        "portal-toggle",
                        Button::new(
                            "portal-toggle",
                            if shown { "Hide note" } else { "Show note" },
                        )
                        .on_click(move |_, _, cx| {
                            open.update(cx, |open, cx| {
                                *open = !*open;
                                cx.notify();
                            })
                        }),
                    ))
                    .when(shown, |clip| {
                        clip.child(deferred(
                            anchored().offset(point(px(0.0), px(8.0))).child(note),
                        ))
                    }),
            )
            .child(code("deferred(anchored())", cx)),
    )
}

fn framed(title: &'static str, body: impl IntoElement, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .w(px(180.0))
        .rounded(theme.radius(Radius::Lg))
        .border_1()
        .border_color(theme.colors.border)
        .child(
            div()
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(theme.colors.fg_subtle)
                .child(title),
        )
        .child(body)
}

pub fn slot(cx: &App) -> impl IntoElement + use<> {
    section(
        "Slot",
        "Take impl IntoElement. The caller fills the hole.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(framed("slot: icon", Icon::new(IconName::Sparkles), cx))
            .child(framed(
                "slot: button",
                Button::new("slot-button", "Save").primary(),
                cx,
            ))
            .child(code("fn framed(body: impl IntoElement)", cx)),
    )
}

pub fn show(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let on = window.use_keyed_state("prim-show", cx, |_, _| true);
    let shown = *on.read(cx);
    let muted = cx.theme().colors.fg_muted;
    section(
        "Show / When",
        ".when() and .when_some() branch inside a builder.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(
                Button::new("show-toggle", if shown { "Hide" } else { "Show" }).on_click(
                    move |_, _, cx| {
                        on.update(cx, |on, cx| {
                            *on = !*on;
                            cx.notify();
                        })
                    },
                ),
            )
            .when(shown, |row| {
                row.child(div().text_color(muted).child("Here while true."))
            })
            .child(code(".when(cond, |el| …)", cx)),
    )
}

pub fn each(cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let steps = ["Draft", "Review", "Ship"];
    section("For / Each", ".children() takes any iterator.", cx).child(
        row()
            .gap_6()
            .children(steps.iter().enumerate().map(|(ix, step)| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_family(theme.mono_family.clone())
                            .text_size(theme.text_size(TextSize::Xs))
                            .text_color(theme.colors.fg_subtle)
                            .child(SharedString::from(format!("{:02}", ix + 1))),
                    )
                    .child(*step)
            }))
            .child(code(".children(iter)", cx)),
    )
}

fn pair(cx: &App) -> [AnyElement; 2] {
    let muted = cx.theme().colors.fg_muted;
    [
        Icon::new(IconName::Layers).color(muted).into_any_element(),
        div()
            .text_color(muted)
            .child("two siblings, no wrapper")
            .into_any_element(),
    ]
}

pub fn fragment(cx: &App) -> impl IntoElement + use<> {
    section(
        "Fragment",
        "Return several elements; the parent adopts them.",
        cx,
    )
    .child(
        row()
            .gap_2()
            .children(pair(cx))
            .child(div().w_6())
            .child(code(".children([a, b])", cx)),
    )
}

pub fn click_outside(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let open = window.use_keyed_state("prim-outside", cx, |_, _| false);
    let shown = *open.read(cx);
    let theme = cx.theme();
    let close = open.clone();
    section(
        "ClickOutside",
        "on_mouse_down_out closes what the pointer left.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(
                Button::new("outside-open", "Open panel")
                    .disabled(shown)
                    .on_click(move |_, _, cx| {
                        open.update(cx, |open, cx| {
                            *open = true;
                            cx.notify();
                        })
                    }),
            )
            .when(shown, |row| {
                row.child(
                    div()
                        .id("outside-panel")
                        .px_3()
                        .py_2()
                        .rounded(theme.radius(Radius::Md))
                        .bg(theme.colors.sunken)
                        .child("Click anywhere else.")
                        .on_mouse_down_out(move |_, _, cx| {
                            close.update(cx, |open, cx| {
                                *open = false;
                                cx.notify();
                            })
                        }),
                )
            })
            .child(code(".on_mouse_down_out(…)", cx)),
    )
}

pub fn hover_area(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let inside = window.use_keyed_state("prim-hover", cx, |_, _| false);
    let hovered = *inside.read(cx);
    let theme = cx.theme();
    section(
        "HoverArea",
        "on_hover reports enter and leave. .hover() restyles.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(
                div()
                    .id("hover-area")
                    .w(px(220.0))
                    .h_16()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme.radius(Radius::Lg))
                    .border_1()
                    .border_color(theme.colors.border)
                    .hover(|style| style.bg(theme.colors.hover))
                    .child(if hovered {
                        "Pointer inside"
                    } else {
                        "Pointer outside"
                    })
                    .on_hover(move |hovered, _, cx| {
                        inside.update(cx, |inside, cx| {
                            *inside = *hovered;
                            cx.notify();
                        })
                    }),
            )
            .child(code(".on_hover(…)", cx)),
    )
}

pub fn keyboard(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let focus = window.use_keyed_state("prim-keys-focus", cx, |_, cx| cx.focus_handle());
    let last = window.use_keyed_state("prim-keys", cx, |_, _| SharedString::from("—"));
    let handle = focus.read(cx).clone();
    let shown = last.read(cx).clone();
    let theme = cx.theme();
    section(
        "KeyboardHandler",
        "on_key_down on a focused element. Click, then type.",
        cx,
    )
    .child(
        row()
            .gap_4()
            .child(
                div()
                    .id("keys")
                    .track_focus(&handle)
                    .w(px(220.0))
                    .h_16()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme.radius(Radius::Lg))
                    .border_1()
                    .border_color(theme.colors.border)
                    .font_family(theme.mono_family.clone())
                    .focus_ring(cx)
                    .child(shown)
                    .on_key_down(move |event, _, cx| {
                        let stroke = SharedString::from(
                            event.keystroke.display(cx.platform_info().os).to_string(),
                        );
                        last.update(cx, |last, cx| {
                            *last = stroke;
                            cx.notify();
                        })
                    }),
            )
            .child(code(".on_key_down(…)", cx)),
    )
}

pub fn clipboard(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let pasted = window.use_keyed_state("prim-clip", cx, |_, _| None::<SharedString>);
    let shown = pasted.read(cx).clone();
    let muted = cx.theme().colors.fg_muted;
    section(
        "Clipboard",
        "write_to_clipboard and read_from_clipboard_async on App; a browser asks before it hands the clipboard over.",
        cx,
    )
    .child(
        row()
            .gap_3()
            .child(
                Button::new("clip-copy", "Copy “Ely”")
                    .icon(IconName::Copy)
                    .on_click(|_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string("Ely".into()))
                    }),
            )
            .child(
                Button::new("clip-paste", "Paste")
                    .variant(ButtonVariant::Ghost)
                    .icon(IconName::Clipboard)
                    .on_click(move |_, _, cx| {
                        let (read, pasted) = (cx.read_from_clipboard_async(), pasted.clone());
                        cx.spawn(async move |cx| {
                            let said: SharedString = match read.await {
                                Ok(item) => match item.and_then(|item| item.text()) {
                                    Some(text) => text.into(),
                                    None => "The clipboard holds no text.".into(),
                                },
                                Err(error) => format!("The clipboard stayed shut: {error:?}").into(),
                            };
                            cx.update(|cx| set(&pasted, Some(said), cx));
                        })
                        .detach();
                    }),
            )
            .when_some(shown, |row, text| {
                row.child(div().text_color(muted).child(text))
            })
            .child(code("cx.write_to_clipboard(…)", cx)),
    )
}
