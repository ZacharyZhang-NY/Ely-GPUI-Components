# Installation

## Requirements

- Rust 1.95 or later, the version Ely's `Cargo.toml` names.
- macOS, where Ely is tested. Windows and Linux builds are not tested yet.
- No full Xcode: GPUI compiles its Metal shaders at run time.

## Add the dependencies

Ely is not on crates.io. Depend on it by git, with GPUI at the snapshot Ely pins. `gpui-pre` republishes Zed's GPUI on crates.io, and GPUI Kit pins the same one:

```toml
[dependencies]
ely-gpui-component = { git = "https://github.com/ZacharyZhang-NY/Ely-GPUI-Components" }
gpui = { package = "gpui-pre", version = "=0.3.12" }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", features = ["font-kit"] }
```

| Line | Why |
| --- | --- |
| `ely-gpui-component` | The library. `Cargo.lock` records the commit; add `rev = "<commit>"` to hold it in `Cargo.toml` as well. |
| `gpui` | The same snapshot as Ely's, under the name `gpui`. Another builds a second GPUI, and its types do not match the ones Ely takes. |
| `gpui_platform` | Opens windows on each platform. Without `font-kit`, macOS falls back to GPUI's no-op text system and draws no text. |

## Shaders

Ely's default feature, `runtime_shaders`, turns on the same feature in `gpui_platform`, so GPUI compiles its Metal shaders at run time. With `default-features = false` the build runs `xcrun metal` instead, which needs Xcode's Metal compiler.

## Check the install

Copy the [first window](/docs/#a-first-window) into `src/main.rs` and run it:

```sh
cargo run
```

A window opens with a counter and a Press button. From a clone of the repository, `cargo run --example gallery` opens every chapter.

## When it fails

| Symptom | Cause and fix |
| --- | --- |
| `init` error: `ely: the asset source lacks N of Ely's files, first <path>: <reason>` | The app's source cannot load every file Ely bundles. Pass `Assets`, or `Assets::before(yours)` when the app has a source of its own. |
| `init` error: `ely: embedded fonts failed to register` | GPUI's text system refused Ely's fonts. File a bug with the log. |
| Windows open with no text on macOS | `gpui_platform` lacks the `font-kit` feature. |
| `mismatched types`, with the note ``there are multiple different versions of crate `gpui` in the dependency graph`` | `gpui` names another snapshot or a git source. Use `gpui-pre` at `=0.3.12`. |
| `metal shader compilation failed` | Default features are off and Xcode's Metal compiler is missing. Keep `runtime_shaders`. |
