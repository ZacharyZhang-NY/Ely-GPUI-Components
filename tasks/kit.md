# GPUI Kit

Started 2026-10-07. Ely and Longbridge's GPUI Kit (`gpui-kit` on crates.io) share one app and one window. Measured before work:

- Ely took gpui from Zed's git at `1a28cff`; Kit 0.7.1 takes `gpui-pre =0.3.8`, a snapshot of Zed at `279fe07`, 109 commits on. Two sources make two crates, and their elements do not mix. Ely on `gpui-pre` 0.3.8 builds and passes all 1,685 tests.
- Actions and key contexts do not collide: Ely's namespaces and contexts all start with `ely`/`Ely`.
- Ely binds Tab with no context, which gpui ranks at the deepest level. In a Kit code editor inside an Ely scope, Tab ties with Kit's indent, and the later `init` wins.
- An app has one asset source. Of Ely's 306 icons, Kit's source holds 290 paths, 289 with the same drawing; `credit-card` differs by one stroke (Lucide 1.48 against 1.43). Ely's `init` checked one icon, so Kit's source alone passed and Ely's other files went missing later.
- Kit's `init` installs its inspector in debug builds; Ely's `install_inspector` replaces it when called after.
- Kit's main branch adds a `gpui-fast` engine, unreleased in 0.7.1; at the owner's word, Ely follows Kit's default gpui alone.

Rules: the work order follows dependencies, since the PRD has no chapter for it. Each item passes `scripts/check.sh` and a codex review (`gpt-6.1-sol`, effort `high`) in a herdr split, five rounds at most, then commits and pushes.

## Items

- [x] K01 gpui from `gpui-pre =0.3.12`, Kit's pin: crate, gallery, web build, docs, CI
- [x] K02 Tab and Shift-Tab bound under `FocusScope`'s own context, so a deeper context's Tab wins whatever the order of `init`
- [x] K03 Assets: `init` checks every embedded file through the app's source; one source serves Ely's files and another's
- [x] K04 `compat/kit`: a crate on the newest Kit that runs both libraries in one window, in `scripts/check.sh` and CI
- [x] K05 Docs: README, the site's guide, AGENTS.md
- [x] K06 Acceptance table

## Log

One line per item: what changed, what stayed and why, review rounds.
- K01: gpui, gpui_platform and gpui_web come from `gpui-pre` `=0.3.8`; the lock holds no git source and one copy of each. `gpui-pre` split leak detection from `test-support`, so this crate's `test-support` turns on `gpui/leak-detection`, and a test that leaks on purpose proves it. The check, the wasm check, the web build (20.0 MB), a headless Chromium shot and the site build pass. README and the install guide name the snapshot. Review: 2 rounds.
- K02: `FocusScope` sets the key context `ElyFocus`, and `init` binds Tab and Shift-Tab under it. A host's Tab in a deeper context wins whatever the order of `init` (a test binds the host's first, the order that lost: 0 indents before, 1 after); Ely's terminal, editor, traps and the inspector's scope behave as before. Review: 1 round.
- K03: `init` loads every file Ely bundles (318, Plex among them) through the app's source and fails naming the first it cannot; an icons-only source like Kit's passed the old one-icon check and fails now. `Assets::before(other)` serves Ely's paths first and the rest from `other`, its errors passing through. Docs and AGENTS.md say so. Review: 1 round.
- K04: `compat/kit`, its own workspace, opens Ely's root scope inside Kit's `Root` and tests Tab through both libraries in each `init` order (red with K02 undone), Kit's dialog from an Ely button by key and by pointer, Ely's dialog over Kit's fields, and one asset source for both. `check.sh` resolves it afresh and holds `gpui-kit` to the newest release, so a Kit release on another gpui fails resolution naming `gpui-pre` (shown with Kit 0.7.0). Review: 2 rounds.
- K05: the site's GPUI Kit guide (`/docs/gpui-kit`): dependencies, the start in four steps, focus, two themes, the inspector; the README points to it. `compat/kit/examples/start.rs` is a whole app, compiled by `check.sh` and run on Windows, where both libraries draw in one window. The guide says an app's Cargo keeps the newest Kit that shares Ely's GPUI, and names no unreleased Kit feature. Review: 2 rounds.
- K04, reopened: CI's macOS job colors Cargo's output, so `cargo search` hid the version from `sed` and `check.sh` stopped there; the search now passes `--color never` (red with `CARGO_TERM_COLOR=always`, green with the flag). Review: round 3.
- K06: the table below; every row checked against the commits, the reviews and CI. Review: 1 round.

## Result

| Surface | Before | Now | Proof |
|---|---|---|---|
| gpui | Zed's git at `1a28cff`; Kit on `gpui-pre =0.3.8`: two crates whose elements do not mix | `gpui-pre =0.3.8` for gpui, gpui_platform and gpui_web | The lock holds one copy of each; Kit 0.7.1 and Ely resolve to one `gpui-pre` in `compat/kit` |
| Ely on the new gpui | — | Builds natively and for wasm32 | `check.sh`; the web build (20.0 MB) drawn in headless Chromium |
| Tests' leak guard | `gpui-pre` dropped leak detection from `test-support` | `test-support` turns on `gpui/leak-detection` | `tests::a_leaked_entity_fails_its_test`, red without it |
| Actions, key contexts | No collision | Unchanged | Namespaces `ely*` against Kit's `root`, `input`, `ui`, `dock`, `number_input`, `inspector` |
| Tab | No context: Kit's code editor won or lost by the order of `init` | Under `ElyFocus`, the scope's context | `a_deeper_context_keeps_its_own_tab_whatever_init_ran_last`; `compat/kit` `focus`, red with the binding undone |
| Assets | One icon checked; Kit's source passed and Ely's other files went missing | Every bundled file checked; `Assets::before` joins Kit's | `assets::tests`; `compat/kit` `assets` |
| Root and dialogs | Untested | Ely's scope inside Kit's `Root`; each library's dialog keeps Tab and hands focus back | `compat/kit` `dialogs`: by key, by pointer, Ely's over Kit's fields |
| Themes, locales, inspector | Separate | Separate, by design; the guide names both switches and the inspector order | The guide; `compat/kit/examples/start.rs` |
| A real window | — | Both libraries draw in one window | `start` run on Windows |
| Kit's next release | Unwatched | `check.sh` asks for Kit's newest release and fails on another gpui, naming `gpui-pre` | Shown with Kit 0.7.0; CI run 37661560047: macOS ran 1,693 Ely tests and the 6 in `compat/kit`, and five platforms built |
| `gpui-fast` | — | Out of scope, at the owner's word | Unreleased in Kit 0.7.1 |
