# Testing

Ely tests itself on GPUI's test platform: every chapter has tests, and `cargo test --lib --features test-support` runs them all. `scripts/check.sh` runs them with formatting, clippy and the house rules.

## The `test-support` feature

`test-support` turns on GPUI's own `test-support`, which brings `TestAppContext`, `#[gpui::test]` and the simulated input. It stays off for the gallery and for apps: GPUI then tracks every entity handle for leaks and records every `debug_selector`'s bounds each frame.

## Testing an app that uses Ely

GPUI's `TestAppContext` serves no assets, so `ely_gpui_component::init` returns an error there. Call `ely_gpui_component::init_for_tests(cx)` instead, under the `test-support` feature: it sets the theme and key bindings without assets or fonts.

## Writing a test inside Ely

| Need | How |
| --- | --- |
| A window | `#[gpui::test] fn name(cx: &mut TestAppContext)`, `Theme::init` in `cx.update`, then `cx.add_window_view(..)`, which returns the view and a `VisualTestContext`. |
| A drawn frame | `cx.run_until_parked()`, `window.refresh()`, then `run_until_parked()` again. The test platform has no display link; a test refreshes the window instead. |
| Next-frame callbacks | They run only when the test calls `Window::simulate_next_frame`. |
| Keys | `simulate_keystrokes("tab enter")` sends key downs alone. GPUI presses a focused element on a key's release, so a test of that dispatches a `KeyUp` as well. |
| Focus events | GPUI calls focus listeners only while the window is active: call `window.activate_window()`. |
| Text | Each character takes a fixed advance, unless the context comes from `TestAppContext::build_with_text_system`. Click where layout does not hang on text, such as padding. |
| Time | Timers and animations both run on the executor's clock, which the test advances (`advance_clock`); a wall-clock sleep advances neither. To see an animation settled, turn on the theme's reduced motion and advance past its 1 ms. |
| Bounds | GPUI clears `debug_bounds` each frame, so a test sees an element leave as well as appear. |

## Tests to read

- [Layers](https://github.com/ZacharyZhang-NY/Ely-GPUI-Components/blob/main/src/primitives/tests.rs): what is raised inside a raise lies over it, and keeps its state.
- [Feedback](https://github.com/ZacharyZhang-NY/Ely-GPUI-Components/blob/main/src/feedback/tests/mod.rs): toasts and alerts, and focus handed to the root when a focused toast leaves.
- [Tables](https://github.com/ZacharyZhang-NY/Ely-GPUI-Components/blob/main/src/tables/tests/mod.rs): clicks, double presses, Tab across a grid.
- [Internationalization](https://github.com/ZacharyZhang-NY/Ely-GPUI-Components/blob/main/src/i18n/tests/mod.rs): catalogs, locales and a mirrored row.
