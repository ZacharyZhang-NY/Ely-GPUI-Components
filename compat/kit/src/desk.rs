use ely_gpui_component::{buttons::Button, overlays::Dialog, primitives::FocusScope};
use gpui_kit::{
    AppContext, Bounds, Context, Entity, FocusHandle, IntoElement, KeyUpEvent, Keystroke,
    ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, WindowBounds,
    WindowOptions,
    component::{
        WindowExt,
        input::{Editor, EditorState, Input, InputState},
    },
    div, point,
    prelude::*,
    px, size,
};

pub const CODE: &str = "fn main() {}";

/// Ely's root scope inside Kit's `Root`.
pub struct Desk {
    pub root: FocusHandle,
    pub open: FocusHandle,
    pub line: Entity<InputState>,
    pub code: Entity<EditorState>,
    pub ely_dialog: bool,
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div().debug_selector(|| "ely-open".into()).child(
                    Button::new("ely-open", "Open")
                        .focus_handle(&self.open)
                        .on_click(|_, window, cx| {
                            window.open_dialog(cx, |dialog, _, _| dialog.title("Kit dialog"));
                        }),
                ),
            )
            .child(Input::new(&self.line))
            .child(div().h(px(120.)).child(Editor::new(&self.code).size_full()))
            .when(self.ely_dialog, |desk| {
                desk.child(
                    Dialog::new("ely-dialog", "Ely dialog", move |_, cx| {
                        view.update(cx, |desk, cx| {
                            desk.ely_dialog = false;
                            cx.notify();
                        })
                    })
                    .action(|_| Button::new("ely-first", "First"))
                    .action(|_| Button::new("ely-second", "Second")),
                )
            })
    }
}

/// Starts both libraries in order; Kit opens the window.
pub fn open(cx: &mut TestAppContext, ely_last: bool) -> (Entity<Desk>, VisualTestContext) {
    cx.update(|cx| {
        if ely_last {
            gpui_kit::init(cx);
            ely_gpui_component::init_for_tests(cx);
        } else {
            ely_gpui_component::init_for_tests(cx);
            gpui_kit::init(cx);
        }
    });
    let (window, desk) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(800.), px(600.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Desk {
                root: cx.focus_handle(),
                open: cx.focus_handle().tab_stop(true),
                line: cx.new(|cx| InputState::new(window, cx)),
                code: cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("rust")
                        .default_value(CODE)
                }),
                ely_dialog: false,
            })
        })
        .expect("Kit opens the window")
    });
    let cx = VisualTestContext::from_window(window, cx);
    cx.run_until_parked();
    (desk, cx)
}

/// Whether the handle holds focus.
pub fn focused(cx: &mut VisualTestContext, handle: &FocusHandle) -> bool {
    cx.update(|window, _| handle.is_focused(window))
}

/// Key down and up; gpui presses on the release.
pub fn press(cx: &mut VisualTestContext, key: &str) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
}
