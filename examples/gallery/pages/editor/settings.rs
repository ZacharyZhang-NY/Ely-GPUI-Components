use ely_gpui_component::{
    editor::{
        KeySource, Keybinding, KeybindingsEditor, Setting, SettingValue, SettingsEditor,
        settings_json,
    },
    forms::{TextInput, json_highlights},
};
use gpui::{App, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

use crate::ui::{keep, section, set};

fn settings() -> Vec<(SharedString, Vec<Setting>)> {
    let number = |value, min, max| SettingValue::Number {
        value,
        min,
        max,
        step: 1.0,
    };
    let choice = |value: &str, choices: &[&str]| SettingValue::Choice {
        value: value.to_string().into(),
        choices: choices
            .iter()
            .map(|choice| SharedString::from(choice.to_string()))
            .collect(),
    };
    let setting = |key: &str, title: &str, description: &str, value: SettingValue| Setting {
        key: key.to_string().into(),
        title: title.to_string().into(),
        description: description.to_string().into(),
        default: value.clone(),
        value,
    };
    vec![
        (
            "Editor".into(),
            vec![
                setting(
                    "editor.fontSize",
                    "Font size",
                    "Code text size in pixels.",
                    number(14.0, 8.0, 32.0),
                ),
                setting(
                    "editor.minimap",
                    "Minimap",
                    "A narrow map of the file beside the code.",
                    SettingValue::Bool(true),
                ),
                setting(
                    "editor.cursorStyle",
                    "Cursor style",
                    "How the cursor draws.",
                    choice("line", &["line", "block", "underline"]),
                ),
            ],
        ),
        (
            "Files".into(),
            vec![
                setting(
                    "files.autoSave",
                    "Auto save",
                    "When files save themselves.",
                    choice("afterDelay", &["off", "afterDelay", "onFocusChange"]),
                ),
                setting(
                    "files.trimTrailingWhitespace",
                    "Trim trailing whitespace",
                    "Drops spaces at line ends on save.",
                    SettingValue::Bool(false),
                ),
            ],
        ),
    ]
}

pub fn settings_and_keys(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let groups = keep("panels-settings", settings, window, cx);
    let section_now = keep("panels-section", || 0usize, window, cx);
    let json = keep("panels-json", || false, window, cx);
    let json_field = window.use_keyed_state("panels-json-field", cx, |window, cx| {
        TextInput::new(window, cx)
            .multi_line(6, 12)
            .highlighter(json_highlights)
    });
    let (now_groups, now_section, now_json) = (
        groups.read(cx).clone(),
        *section_now.read(cx),
        *json.read(cx),
    );
    let (change, pick, flip, seed, source) = (
        groups.clone(),
        section_now.clone(),
        json.clone(),
        json_field.clone(),
        groups.clone(),
    );
    let recording = keep("panels-recording", || None::<SharedString>, window, cx);
    let bindings = keep(
        "panels-bindings",
        || {
            let binding =
                |command: &str, title: &str, keys: Option<&str>, when: Option<&str>, source| {
                    Keybinding {
                        command: command.to_string().into(),
                        title: title.to_string().into(),
                        keys: keys.map(|keys| keys.to_string().into()),
                        when: when.map(|when| when.to_string().into()),
                        source,
                    }
                };
            vec![
                binding(
                    "editor.save",
                    "Save",
                    Some("secondary-s"),
                    None,
                    KeySource::Default,
                ),
                binding(
                    "editor.sortLines",
                    "Sort lines",
                    Some("secondary-s"),
                    None,
                    KeySource::User,
                ),
                binding(
                    "editor.comment",
                    "Toggle comment",
                    Some("secondary-/"),
                    Some("editorFocus"),
                    KeySource::Default,
                ),
                binding(
                    "editor.selectNext",
                    "Select next match",
                    Some("secondary-d"),
                    Some("editorFocus"),
                    KeySource::Default,
                ),
                binding(
                    "terminal.split",
                    "Split terminal",
                    Some("secondary-s"),
                    Some("terminalFocus"),
                    KeySource::User,
                ),
                binding(
                    "editor.fold",
                    "Fold",
                    None,
                    Some("editorFocus"),
                    KeySource::Default,
                ),
            ]
        },
        window,
        cx,
    );
    let (now_recording, now_bindings) = (recording.read(cx).clone(), bindings.read(cx).clone());
    let (record, bind, done) = (recording.clone(), bindings.clone(), recording.clone());
    let editor = SettingsEditor::new("panels-settings-editor", now_groups.clone())
        .section(now_section)
        .on_section(move |ix, _, cx| set(&pick, ix, cx))
        .on_json(move |on, _, cx| {
            if on {
                let text = settings_json(source.read(cx));
                seed.update(cx, |field, cx| field.set_text(text, cx));
            }
            set(&flip, on, cx)
        })
        .on_change(move |key, value, _, cx| {
            let mut next = change.read(cx).clone();
            for (_, list) in &mut next {
                for setting in list.iter_mut().filter(|setting| setting.key == *key) {
                    setting.value = value.clone();
                }
            }
            set(&change, next, cx)
        });
    let editor = if now_json {
        editor.json(&json_field)
    } else {
        editor
    };
    section(
        "SettingsEditor / KeybindingsEditor",
        "Settings as controls by section, each change marked with a way back, or as the JSON a settings file holds; and every command's keys, clashes marked. Double-press a command to record new keys.",
        cx,
    )
    .child(div().w(px(840.)).child(editor))
    .child(
        div().w(px(840.)).child(
            KeybindingsEditor::new("panels-keys", now_bindings)
                .recording(now_recording)
                .on_record(move |command, _, cx| set(&record, Some(command.clone()), cx))
                .on_bind(move |command, stroke, _, cx| {
                    let mut next = bind.read(cx).clone();
                    if let Some(binding) = next.iter_mut().find(|binding| binding.command == *command) {
                        binding.keys = stroke.map(|stroke| stroke.unparse(cx.platform_info().os).into());
                        binding.source = KeySource::User;
                    }
                    set(&bind, next, cx);
                    set(&done, None, cx)
                }),
        ),
    )
}
