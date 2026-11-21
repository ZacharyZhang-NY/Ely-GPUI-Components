# GPUI Kit

Ely runs beside Longbridge's [GPUI Kit](https://gpui-kit.com) in one app and one window. Both build on one GPUI: Ely pins `gpui-pre =0.3.12`, the snapshot Kit 0.7.1 pins.

## Add the dependencies

```toml
[dependencies]
gpui-kit = "0.7"
ely-gpui-component = { git = "https://github.com/ZacharyZhang-NY/Ely-GPUI-Components" }
```

Kit brings GPUI and puts it at its root, so the app writes `use gpui_kit::*` and lists no `gpui` line of its own. Ely follows Kit's default GPUI.

In an app, Cargo picks the newest Kit that shares Ely's GPUI. A Kit release on another `gpui-pre` waits until Ely's pin follows. Ely's CI asks for Kit's newest release on every push and fails on such a release, naming `gpui-pre`.

## Start both

| Step | Why |
| --- | --- |
| `gpui_kit::application().with_assets(Assets::before(gpui_kit::assets::Assets))` | GPUI takes one asset source. Ely's files load from Ely, every other path from Kit. `ely_gpui_component::init` returns an error if one of Ely's files does not load. |
| `gpui_kit::init(cx)` and `ely_gpui_component::init(cx)?` | In either order. |
| `gpui_kit::open_window(options, cx, build)` | Kit's `Root` hosts Kit's dialogs, sheets and notifications. |
| `FocusScope::new(&handle).root()` around the view, `handle` focused | Ely's Tab, focus return and overlays live in the scope. |

## Focus

Tab and Shift-Tab run through Ely's and Kit's stops in one order. Ely binds them under `ElyFocus`, the key context of its scopes, so Kit's code editor keeps Tab for indenting whichever `init` ran last. A Kit dialog opened from an Ely button keeps Tab inside and hands focus back when it closes; so does an Ely dialog over Kit's fields.

## Two themes

Each library keeps its own theme. A switch turns both:

| Ely | Kit |
| --- | --- |
| `Theme::set_mode(Mode::Dark, cx)` | `Theme::change(ThemeMode::Dark, Some(window), cx)` |

Locales work the same way: Ely reads its `I18n` global, Kit its `gpui_kit::component::set_locale`.

## Inspector

In debug builds Kit's `init` installs Kit's inspector. Call `ely_gpui_component::tooling::install_inspector(cx)` after it to draw Ely's.

## A whole app

[`compat/kit/examples/start.rs`](https://github.com/ZacharyZhang-NY/Ely-GPUI-Components/blob/main/compat/kit/examples/start.rs) puts Ely's heading and button beside Kit's field and dialog, and one button turns both themes. From a clone, run it with `cargo run --manifest-path compat/kit/Cargo.toml --example start`. The crate's tests hold each claim on this page.
