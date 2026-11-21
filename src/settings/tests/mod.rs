use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px,
};

use super::{
    Appearance, Changed, DeveloperModeToggle, FeatureFlags, Flag, FontSizeControl,
    ImportExportSettings, Notices, NotificationSettings, Privacy, PrivacySettings, Proxy,
    ProxySettings, ResetToDefault, SettingEntry, SettingsLayout, SettingsSearch, Stage, Startup,
    StartupSettings, StorageSettings, Store, ThemeSelector,
};
use crate::{
    forms::{self, Choice, TextInput},
    primitives::{FocusNext, IconName},
    theme::Theme,
};

type Part = fn(&mut Window, &mut App, Entity<Desk>) -> AnyElement;

/// A view that shows one setting's part and keeps the words it heard.
struct Desk {
    part: Part,
    said: Vec<String>,
}

impl Render for Desk {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let part = (self.part)(window, cx, owner);
        div().w(px(640.0)).child(part)
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn desk(part: Part, cx: &mut TestAppContext) -> (Entity<Desk>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Desk {
        part,
        said: Vec::new(),
    });
    settle(cx);
    (host, cx)
}

fn say(owner: &Entity<Desk>, words: String, cx: &mut App) {
    owner.update(cx, |desk, cx| {
        desk.said.push(words);
        cx.notify();
    });
}

fn said(host: &Entity<Desk>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |desk, _| desk.said.clone())
}

fn tab(stops: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        (0..stops).for_each(|_| window.focus_next(cx));
    });
    settle(cx);
}

fn tap(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    let os = cx.read(|app| app.platform_info().os);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, os).expect("a key"),
    });
    settle(cx);
}

fn search(window: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    let field = window.use_keyed_state("search", cx, TextInput::new);
    let entry = |key: &str, name: &str, words: &[&str]| SettingEntry {
        key: key.to_string().into(),
        section: "Appearance".into(),
        name: name.to_string().into(),
        words: words.iter().map(|word| word.to_string().into()).collect(),
    };
    SettingsSearch::new(
        "search",
        [
            entry("theme", "Theme", &["dark"]),
            entry("size", "Font size", &["zoom"]),
        ],
        &field,
        move |key, _, cx| say(&owner, format!("go {key}"), cx),
    )
    .into_any_element()
}

/// Stops: the field, then the list of what answers.
#[gpui::test]
fn a_search_narrows_the_settings_and_enter_goes_to_one(cx: &mut TestAppContext) {
    let (host, cx) = desk(search, cx);
    tab(1, cx);
    cx.simulate_input("zoom");
    settle(cx);
    tab(2, cx);
    tap("enter", cx);
    assert_eq!(said(&host, cx), ["go size"]);
}

fn reset(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let changed = |key: &str| Changed {
        key: key.to_string().into(),
        name: key.to_string().into(),
        now: "on".into(),
        default: "off".into(),
    };
    let (one, all) = (owner.clone(), owner);
    ResetToDefault::new(
        "reset",
        [changed("a"), changed("b")],
        move |key, _, cx| say(&one, format!("reset {key}"), cx),
        move |_, cx| say(&all, "reset all".into(), cx),
    )
    .into_any_element()
}

/// Stops: each row's way back, then Reset all, which asks twice.
#[gpui::test]
fn a_row_goes_back_alone_and_reset_all_asks_twice(cx: &mut TestAppContext) {
    let (host, cx) = desk(reset, cx);
    tab(2, cx);
    tap("space", cx);
    tab(3, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["reset b"], "the first press only arms");
    tap("space", cx);
    assert_eq!(said(&host, cx), ["reset b", "reset all"]);
}

fn flags(window: &mut Window, cx: &mut App, owner: Entity<Desk>) -> AnyElement {
    let field = window.use_keyed_state("flags-search", cx, TextInput::new);
    let flag = |key: &str, on| Flag {
        key: key.to_string().into(),
        name: key.to_string().into(),
        description: "does a thing".into(),
        stage: Stage::Beta,
        on,
    };
    FeatureFlags::new(
        "flags",
        [flag("tabs", false), flag("sync", true)],
        &field,
        move |key, on, _, cx| say(&owner, format!("{key} {on}"), cx),
    )
    .into_any_element()
}

/// Stops: the search, then each flag's switch.
#[gpui::test]
fn a_flags_switch_turns_it(cx: &mut TestAppContext) {
    let (host, cx) = desk(flags, cx);
    tab(3, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["sync false"]);
}

fn notices(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let notices = Notices {
        channels: vec!["App".into(), "Email".into()],
        kinds: vec![("mention".into(), "Mentions".into())],
        on: vec![("mention".into(), "App".into())],
        quiet: false,
    };
    NotificationSettings::new("notices", notices, move |notices, _, cx| {
        say(&owner, format!("{:?} {}", notices.on, notices.quiet), cx)
    })
    .into_any_element()
}

/// Stops: Do not disturb, then a box per channel.
#[gpui::test]
fn a_box_turns_its_kind_on_its_channel(cx: &mut TestAppContext) {
    let (host, cx) = desk(notices, cx);
    tab(3, cx);
    tap("space", cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        [
            "[(\"mention\", \"App\"), (\"mention\", \"Email\")] false",
            "[(\"mention\", \"App\")] true"
        ]
    );
}

fn storage(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    StorageSettings::new(
        "storage",
        [
            Store {
                key: "cache".into(),
                name: "Cache".into(),
                bytes: 3 << 20,
                clearable: true,
            },
            Store {
                key: "data".into(),
                name: "Data".into(),
                bytes: 9 << 20,
                clearable: false,
            },
        ],
        move |key, _, cx| say(&owner, format!("clear {key}"), cx),
    )
    .into_any_element()
}

#[gpui::test]
fn a_store_clears_on_the_second_press(cx: &mut TestAppContext) {
    let (host, cx) = desk(storage, cx);
    tab(1, cx);
    tap("space", cx);
    assert!(said(&host, cx).is_empty(), "the first press only arms");
    tap("space", cx);
    assert_eq!(said(&host, cx), ["clear cache"]);
}

fn size(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    FontSizeControl::new("size", 18.0, 14.0, (12.0, 24.0), move |size, _, cx| {
        say(&owner, format!("size {size}"), cx)
    })
    .into_any_element()
}

/// Stops: the slider, then the way back.
#[gpui::test]
fn the_way_back_gives_the_default_size(cx: &mut TestAppContext) {
    let (host, cx) = desk(size, cx);
    tab(2, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["size 14"]);
}

fn theme(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    ThemeSelector::new("theme", Appearance::Light, move |appearance, _, cx| {
        say(&owner, format!("{appearance:?}"), cx)
    })
    .into_any_element()
}

/// Stops: a segment each.
#[gpui::test]
fn a_segment_picks_the_appearance(cx: &mut TestAppContext) {
    let (host, cx) = desk(theme, cx);
    tab(3, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["System"]);
}

fn proxy(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let proxy = Proxy::Manual {
        host: "proxy.corp".into(),
        port: 8080,
        bypass: vec!["localhost".into()],
    };
    ProxySettings::new("proxy", proxy, move |proxy, _, cx| {
        say(&owner, format!("{proxy:?}"), cx)
    })
    .into_any_element()
}

/// Apply is the last stop, so a step back from nothing reaches it.
#[gpui::test]
fn apply_hands_on_the_manual_proxy(cx: &mut TestAppContext) {
    let (host, cx) = desk(proxy, cx);
    cx.update(|window, cx| {
        window.blur(cx);
        window.focus_prev(cx);
    });
    settle(cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        ["Manual { host: \"proxy.corp\", port: 8080, bypass: [\"localhost\"] }"]
    );
}

fn layout(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    SettingsLayout::new(
        "layout",
        [
            ("a", "Appearance", IconName::Palette),
            ("b", "Network", IconName::Globe),
        ],
        "a",
        move |key, _, cx| say(&owner, format!("open {key}"), cx),
    )
    .page(div())
    .into_any_element()
}

/// Stops: the section list.
#[gpui::test]
fn the_arrows_open_the_next_section(cx: &mut TestAppContext) {
    let (host, cx) = desk(layout, cx);
    tab(1, cx);
    tap("down", cx);
    assert_eq!(said(&host, cx), ["open b"]);
}

fn privacy(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let privacy = Privacy {
        telemetry: true,
        crash_reports: true,
        personalization: false,
        history: true,
    };
    PrivacySettings::new("privacy", privacy, move |next, _, cx| {
        say(&owner, format!("{next:?}"), cx)
    })
    .into_any_element()
}

/// Stops: a switch per field, telemetry first.
#[gpui::test]
fn a_privacy_switch_turns_its_own_field(cx: &mut TestAppContext) {
    let (host, cx) = desk(privacy, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(
        said(&host, cx),
        [
            "Privacy { telemetry: false, crash_reports: true, personalization: false, history: true }"
        ]
    );
}

fn quiet(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let notices = Notices {
        channels: vec!["App".into()],
        kinds: vec![("replies".into(), "Replies".into())],
        on: vec![("replies".into(), "App".into())],
        quiet: false,
    };
    NotificationSettings::new("notices", notices, move |next, _, cx| {
        say(
            &owner,
            format!("quiet {} on {}", next.quiet, next.on.len()),
            cx,
        )
    })
    .into_any_element()
}

/// Stops: Do not disturb, then a box per kind and channel.
#[gpui::test]
fn do_not_disturb_holds_notices_and_keeps_the_boxes(cx: &mut TestAppContext) {
    let (host, cx) = desk(quiet, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["quiet true on 1"]);
}

fn startup(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let startup = Startup {
        at_login: false,
        restore: true,
        opens: "last".into(),
        updates: true,
    };
    StartupSettings::new(
        "startup",
        startup,
        [Choice::new("last", "Where I left off")],
        move |next, _, cx| say(&owner, format!("login {}", next.at_login), cx),
    )
    .into_any_element()
}

/// Stops: Open at login first.
#[gpui::test]
fn open_at_login_turns_on(cx: &mut TestAppContext) {
    let (host, cx) = desk(startup, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["login true"]);
}

fn developer(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    DeveloperModeToggle::new("developer", false, move |on, _, cx| {
        say(&owner, format!("developer {on}"), cx)
    })
    .into_any_element()
}

/// Stops: the switch.
#[gpui::test]
fn developer_mode_turns_on(cx: &mut TestAppContext) {
    let (host, cx) = desk(developer, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["developer true"]);
}

fn files(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    ImportExportSettings::new(
        "files",
        move |_, cx| say(&owner, "export".into(), cx),
        |_, _, _| {},
    )
    .into_any_element()
}

/// Stops: Export first.
#[gpui::test]
fn export_asks_the_owner(cx: &mut TestAppContext) {
    let (host, cx) = desk(files, cx);
    tab(1, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["export"]);
}

fn bad_port(_: &mut Window, _: &mut App, owner: Entity<Desk>) -> AnyElement {
    let proxy = Proxy::Manual {
        host: "proxy.corp".into(),
        port: 0,
        bypass: Vec::new(),
    };
    ProxySettings::new("proxy", proxy, move |proxy, _, cx| {
        say(&owner, format!("{proxy:?}"), cx)
    })
    .into_any_element()
}

/// With port 0 Apply rests, so a step back from nothing lands on the field before it.
#[gpui::test]
fn apply_rests_while_the_port_is_bad(cx: &mut TestAppContext) {
    let (host, cx) = desk(bad_port, cx);
    cx.update(|window, cx| {
        window.blur(cx);
        window.focus_prev(cx);
    });
    settle(cx);
    tap("space", cx);
    assert!(said(&host, cx).is_empty());
}

/// Stops: Do not disturb, then App, which is on.
#[gpui::test]
fn a_box_pressed_off_lets_its_pair_go(cx: &mut TestAppContext) {
    let (host, cx) = desk(notices, cx);
    tab(2, cx);
    tap("space", cx);
    assert_eq!(said(&host, cx), ["[] false"]);
}

mod appearance;
mod theme;
