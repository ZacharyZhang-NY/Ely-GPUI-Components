use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Styled,
    TestAppContext, Window,
};

use super::{Desk, desk, said, say, settle, tab, tap};
use crate::{
    settings::{
        SyntaxThemePicker, ThemeDraft, ThemeEditor,
        importer::{self, OnTheme},
        read_vscode_theme,
    },
    theme::{ActiveTheme, Theme, syntax_themes},
};

fn editor(_: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    ThemeEditor::new("editor", ThemeDraft::of(cx.theme()), move |draft, _, cx| {
        say(
            &owner,
            format!(
                "{:?} contrast {} still {}",
                draft.density, draft.high_contrast, draft.reduced_motion
            ),
            cx,
        )
    })
    .into_any_element()
}

/// Stops: the three densities, corners, text size, high contrast, reduced motion, then the colors. A switch hands on a draft with its own flag turned and the rest as it stood.
#[gpui::test]
fn a_switch_turns_its_own_flag(cx: &mut TestAppContext) {
    let (host, cx) = desk(editor, cx);
    tab(6, cx);
    tap("space", cx);
    tab(7, cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        [
            "Standard contrast true still true",
            "Standard contrast false still false"
        ],
        "the desk runs with reduced motion on, so its switch turns it off"
    );
}

fn colors(_: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    ThemeEditor::new("editor", ThemeDraft::of(cx.theme()), move |draft, _, cx| {
        let background = gpui::Rgba::from(draft.colors.bg);
        say(&owner, crate::forms::hex(background), cx)
    })
    .into_any_element()
}

/// The first color is the eighth stop; Enter opens its picker, where the hex field follows the eyedropper, a stop on macOS alone, and takes a new background.
#[gpui::test]
fn a_color_well_sets_its_own_color(cx: &mut TestAppContext) {
    let (host, cx) = desk(colors, cx);
    tab(8, cx);
    tap("enter", cx);
    for _ in 0..if cfg!(target_os = "macos") { 3 } else { 2 } {
        tap("tab", cx);
    }
    tap("secondary-a", cx);
    cx.simulate_input("#224466");
    tap("enter", cx);
    assert_eq!(said(&host, cx), ["#224466"]);
}

fn look(_: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    ThemeEditor::new("editor", ThemeDraft::of(cx.theme()), move |draft, _, cx| {
        let words = format!(
            "{:?} corners {} text {}",
            draft.density, draft.radius_scale, draft.font_scale
        );
        say(&owner, words, cx)
    })
    .into_any_element()
}

/// Stops: Compact, Standard, Comfortable, corners, text size. Compact picks the density alone, and Right on each slider moves its own measure a step.
#[gpui::test]
fn the_density_and_the_sliders_move_their_own_field(cx: &mut TestAppContext) {
    let (host, cx) = desk(look, cx);
    tab(1, cx);
    tap("space", cx);
    tab(4, cx);
    tap("right", cx);
    tab(5, cx);
    tap("right", cx);
    assert_eq!(
        said(&host, cx),
        [
            "Compact corners 1 text 1",
            "Standard corners 1.25 text 1",
            "Standard corners 1 text 1.05"
        ]
    );
}

fn narrow(window: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    gpui::div()
        .debug_selector(|| "narrow".into())
        .w(gpui::px(280.0))
        .child(editor(window, cx, owner))
        .into_any_element()
}

/// At 280px with the text a step and a half larger, the density strip keeps every segment inside the box, its labels giving way.
#[gpui::test]
fn the_density_strip_fits_a_narrow_box_with_larger_text(cx: &mut TestAppContext) {
    let (_, cx) = desk(narrow, cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.font_scale = 1.15));
    settle(cx);
    let frame = cx.debug_bounds("narrow").expect("the box");
    let last = cx
        .debug_bounds("segment Comfortable")
        .expect("the last segment");
    assert!(last.right() <= frame.right(), "{last:?} inside {frame:?}");
}

/// At 280px and the base size the strip has room, so each label keeps its whole width: eleven letters to seven.
#[gpui::test]
fn a_label_gives_way_only_when_the_strip_lacks_room(cx: &mut TestAppContext) {
    let (_, cx) = desk(narrow, cx);
    settle(cx);
    let compact = cx
        .debug_bounds("segment-label Compact")
        .expect("Compact's label");
    let comfortable = cx
        .debug_bounds("segment-label Comfortable")
        .expect("Comfortable's label");
    let ratio = comfortable.size.width / compact.size.width;
    assert!(
        (ratio - 11.0 / 7.0).abs() < 0.01,
        "{comfortable:?} to {compact:?}"
    );
}

/// The thumb covers the chosen segment, top to bottom and side to side.
#[gpui::test]
fn the_thumb_covers_the_chosen_segment(cx: &mut TestAppContext) {
    let (_, cx) = desk(editor, cx);
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2));
        settle(cx);
    }
    let thumb = cx.debug_bounds("segment-thumb").expect("the thumb");
    let chosen = cx
        .debug_bounds("segment Standard")
        .expect("the chosen segment");
    assert_eq!(thumb, chosen);
}

fn syntax(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    SyntaxThemePicker::new("syntax", syntax_themes(), "Ely", move |name, _, cx| {
        say(&owner, name.to_string(), cx)
    })
    .into_any_element()
}

/// Stops: a card for each code palette; Space on the third picks Paper.
#[gpui::test]
fn a_card_picks_its_code_palette(cx: &mut TestAppContext) {
    let (host, cx) = desk(syntax, cx);
    tab(3, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["Paper"]);
}

fn read_row(_: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    let read = read_vscode_theme(
        r##"{"name": "Mist", "type": "dark", "colors": {"editor.background": "#101418"}}"##,
    )
    .expect("a theme");
    let apply: OnTheme = std::rc::Rc::new(move |read, _, cx| {
        say(&owner, format!("{:?} {:?}", read.name, read.mode), cx)
    });
    importer::read_row(&read, &"mist.json".into(), &"importer".into(), apply, cx)
}

/// The row a read theme shows: Apply, its one stop, hands the owner that theme.
#[gpui::test]
fn apply_hands_the_owner_the_theme_it_read(cx: &mut TestAppContext) {
    let (host, cx) = desk(read_row, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["Some(\"Mist\") Dark"]);
}

fn narrow_syntax(window: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    gpui::div()
        .debug_selector(|| "narrow".into())
        .w(gpui::px(280.0))
        .child(syntax(window, cx, owner))
        .into_any_element()
}

/// At 280px a card's line of code wraps inside the card instead of running past it.
#[gpui::test]
fn a_code_sample_stays_inside_a_narrow_card(cx: &mut TestAppContext) {
    let (_, cx) = desk(narrow_syntax, cx);
    settle(cx);
    let frame = cx.debug_bounds("narrow").expect("the box");
    let sample = cx.debug_bounds("syntax-sample").expect("a sample");
    assert!(
        sample.right() <= frame.right(),
        "{sample:?} inside {frame:?}"
    );
}

fn narrow_read_row(_: &mut Window, cx: &mut App, _: Entity<Desk>) -> AnyElement {
    let read = read_vscode_theme(
        r##"{"name": "GitHub Dark Default Colorblind Edition", "type": "dark", "colors": {}}"##,
    )
    .expect("a theme");
    let apply: OnTheme = std::rc::Rc::new(|_, _, _| {});
    gpui::div()
        .debug_selector(|| "narrow".into())
        .w(gpui::px(280.0))
        .child(importer::read_row(
            &read,
            &"long.json".into(),
            &"importer".into(),
            apply,
            cx,
        ))
        .into_any_element()
}

/// At 280px a long theme name gives way, and its mode badge stays inside the box.
#[gpui::test]
fn a_long_theme_name_gives_way_to_its_badge(cx: &mut TestAppContext) {
    let (_, cx) = desk(narrow_read_row, cx);
    settle(cx);
    let frame = cx.debug_bounds("narrow").expect("the box");
    let badge = cx.debug_bounds("theme-import-mode").expect("the badge");
    assert!(badge.right() <= frame.right(), "{badge:?} inside {frame:?}");
}
