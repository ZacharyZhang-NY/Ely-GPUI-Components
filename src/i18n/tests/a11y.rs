use gpui::{
    Context, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyBinding, KeyUpEvent, Keystroke,
    Modifiers, ParentElement, Render, Rgba, StatefulInteractiveElement, Styled, TestAppContext,
    VisualTestContext, Window, div, point, px, rgb,
};

use crate::{
    devtools::contrast,
    i18n::SkipLink,
    primitives::{FocusNext, FocusScope},
    theme::{ActiveTheme, Mode, Theme},
};

/// Machado, Oliveira and Fernandes (2009) at severity 1, on linear sRGB.
const DICHROMACY: [(&str, [[f32; 3]; 3]); 3] = [
    (
        "protanopia",
        [
            [0.152286, 1.052583, -0.204868],
            [0.114503, 0.786281, 0.099216],
            [-0.003882, -0.048116, 1.051998],
        ],
    ),
    (
        "deuteranopia",
        [
            [0.367322, 0.860646, -0.227968],
            [0.280085, 0.672501, 0.047413],
            [-0.011820, 0.042940, 0.968881],
        ],
    ),
    (
        "tritanopia",
        [
            [1.255528, -0.076749, -0.178779],
            [-0.078411, 0.930809, 0.147602],
            [0.004733, 0.691367, 0.303900],
        ],
    ),
];

/// A color in linear sRGB, as an eye with `matrix` sees it.
fn seen(color: Hsla, matrix: Option<&[[f32; 3]; 3]>) -> [f32; 3] {
    let rgb = Rgba::from(color);
    let linear = [rgb.r, rgb.g, rgb.b].map(|channel| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    match matrix {
        None => linear,
        Some(matrix) => matrix.map(|row| {
            (row[0] * linear[0] + row[1] * linear[1] + row[2] * linear[2]).clamp(0.0, 1.0)
        }),
    }
}

/// Björn Ottosson's Oklab.
fn oklab([r, g, b]: [f32; 3]) -> [f32; 3] {
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// The two nearest colors' Oklab distance, in hundredths, as `matrix` sees them.
fn nearest(colors: &[Hsla], matrix: Option<&[[f32; 3]; 3]>) -> f32 {
    let labs: Vec<_> = colors
        .iter()
        .map(|&color| oklab(seen(color, matrix)))
        .collect();
    let mut near = f32::MAX;
    for (ix, a) in labs.iter().enumerate() {
        for b in &labs[ix + 1..] {
            let distance = a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f32>();
            near = near.min(distance.sqrt() * 100.0);
        }
    }
    near
}

fn hexes(colors: &[Hsla]) -> String {
    colors
        .iter()
        .map(|&color| {
            let rgb = Rgba::from(color);
            let byte = |channel: f32| (channel * 255.0).round() as u8;
            format!("{:02x}{:02x}{:02x}", byte(rgb.r), byte(rgb.g), byte(rgb.b))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Red and green, far apart for most eyes, run together for deuteranopia: the measure bites.
#[test]
fn red_and_green_run_together_for_deuteranopia() {
    let pair = [rgb(0xd62728).into(), rgb(0x2ca02c).into()];
    assert!(nearest(&pair, None) > 30.0);
    assert!(nearest(&pair, Some(&DICHROMACY[1].1)) < 5.0);
}

/// Color-blind safe charts change Ely's chart hues alone: every pair stays apart for each dichromacy, and each hue is 3:1 on its page.
#[gpui::test]
fn color_blind_safe_hues_stay_apart(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    for mode in [Mode::Light, Mode::Dark] {
        cx.update(|cx| Theme::set_mode(mode, cx));
        let own = cx.read(|cx| cx.theme().palette());
        cx.update(|cx| Theme::set_color_blind_safe(true, cx));
        let safe = cx.read(|cx| cx.theme().palette());
        cx.update(|cx| Theme::set_color_blind_safe(false, cx));
        let mut expected = own.clone();
        expected.chart = safe.chart;
        assert_eq!(safe, expected, "{mode:?}: only the chart changes");
        assert_ne!(safe.chart, own.chart, "{mode:?}: the chart changes");
        let seen = format!(
            "{mode:?}: {} against {}",
            hexes(&safe.chart),
            hexes(&own.chart)
        );
        let near = nearest(&safe.chart, None);
        assert!(near >= 10.0, "{seen}: {near}");
        for (name, matrix) in &DICHROMACY {
            let near = nearest(&safe.chart, Some(matrix));
            assert!(near >= 10.0, "{seen} for {name}: {near}");
        }
        for hue in safe.chart {
            let ratio = contrast(hue, safe.bg);
            assert!(ratio >= 3.0, "{mode:?}: {hue:?} at {ratio}");
        }
    }
}

/// A nav of three stops between a skip link and the main part.
struct Page {
    root: FocusHandle,
    nav: [FocusHandle; 3],
    main: FocusHandle,
    inside: FocusHandle,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let stop = |ix: usize, handle: &FocusHandle| {
            div()
                .id(("nav", ix))
                .track_focus(handle)
                .h(px(24.0))
                .on_click(|_, _, _| {})
        };
        FocusScope::new(&self.root)
            .size_full()
            .child(SkipLink::new("skip", "Skip to content", &self.main))
            .children(
                self.nav
                    .iter()
                    .enumerate()
                    .map(|(ix, handle)| stop(ix, handle)),
            )
            .child(
                div()
                    .track_focus(&self.main)
                    .child(div().track_focus(&self.inside).h(px(24.0))),
            )
    }
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.run_until_parked();
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    cx.run_until_parked();
}

fn page(cx: &mut TestAppContext) -> (gpui::Entity<Page>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Page {
        root: cx.focus_handle(),
        nav: [0, 1, 2].map(|_| cx.focus_handle().tab_stop(true)),
        main: cx.focus_handle(),
        inside: cx.focus_handle().tab_stop(true),
    });
    cx.run_until_parked();
    (view, cx)
}

fn focused(
    view: &gpui::Entity<Page>,
    pick: fn(&Page) -> &FocusHandle,
    cx: &mut VisualTestContext,
) -> bool {
    let handle = view.read_with(cx, |page, _| pick(page).clone());
    cx.update(|window, _| handle.is_focused(window))
}

/// Tab from the page's start reaches the skip link before the nav; Enter hands focus to the main part, and the next Tab lands inside it.
#[gpui::test]
fn a_skip_link_leads_past_the_nav(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    let root = view.read_with(cx, |page, _| page.root.clone());
    cx.update(|window, cx| window.focus(&root, cx));
    press("tab", cx);
    assert!(
        !focused(&view, |page| &page.nav[0], cx),
        "the link comes first"
    );
    assert!(!focused(&view, |page| &page.root, cx));
    press("enter", cx);
    assert!(focused(&view, |page| &page.main, cx), "Enter skips");
    press("tab", cx);
    assert!(
        focused(&view, |page| &page.inside, cx),
        "Tab goes on inside"
    );
}

/// The link shows while focused, so a press at its place skips; at rest nothing lies there and a press reaches the nav.
#[gpui::test]
fn a_skip_link_shows_only_while_focused(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    let spot = point(px(16.0), px(16.0));
    cx.simulate_click(spot, Modifiers::none());
    cx.run_until_parked();
    assert!(
        focused(&view, |page| &page.nav[0], cx),
        "at rest the nav takes it"
    );
    let root = view.read_with(cx, |page, _| page.root.clone());
    cx.update(|window, cx| window.focus(&root, cx));
    press("tab", cx);
    cx.simulate_click(spot, Modifiers::none());
    cx.run_until_parked();
    assert!(
        focused(&view, |page| &page.main, cx),
        "the shown link skips"
    );
}
