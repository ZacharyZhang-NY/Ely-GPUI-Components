use std::rc::Rc;

use gpui::{
    App, ElementId, Entity, IntoElement, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::*,
};

use super::{Close, Dialog};
use crate::{
    buttons::{Button, ButtonVariant},
    forms::{Input, Run, TextInput},
    i18n,
    primitives::{Icon, Severity},
    theme::{ActiveTheme, ControlSize, IconSize, TextSize},
};

/// The row a message sits in: its severity's icon on a quiet disc, then the text.
fn said(severity: Option<Severity>, message: SharedString, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    div()
        .flex()
        .items_start()
        .gap_3()
        .when_some(severity, |row, severity| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(theme.control_height(ControlSize::Lg))
                    .rounded_full()
                    .bg(severity.subtle(colors))
                    .child(
                        Icon::new(severity.icon())
                            .size(IconSize::Md)
                            .color(severity.color(colors)),
                    ),
            )
        })
        .child(
            div()
                .pt_1p5()
                .text_size(theme.text_size(TextSize::Sm))
                .text_color(colors.fg_muted)
                .child(message),
        )
}

/// A message to acknowledge: its severity's icon, the text and one button. The scrim does not close it.
#[derive(IntoElement)]
pub struct AlertDialog {
    id: ElementId,
    severity: Severity,
    title: SharedString,
    message: SharedString,
    label: SharedString,
    on_close: Run,
}

impl AlertDialog {
    pub fn new(
        id: impl Into<ElementId>,
        severity: Severity,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            title: title.into(),
            message: message.into(),
            label: "OK".into(),
            on_close: Rc::new(on_close),
        }
    }

    /// The button's label, OK unless set.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
}

impl RenderOnce for AlertDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (ok, label) = ((self.id.clone(), "ok"), self.label);
        let on_close = self.on_close;
        Dialog::new(self.id, self.title, move |window, cx| on_close(window, cx))
            .held()
            .child(said(Some(self.severity), self.message, cx))
            .action(move |close| {
                Button::new(ok, label)
                    .primary()
                    .on_click(move |_, window, cx| close(window, cx))
            })
    }
}

/// Asks before an action: Cancel, and a button that confirms, red when the action destroys.
#[derive(IntoElement)]
pub struct ConfirmDialog {
    id: ElementId,
    title: SharedString,
    message: SharedString,
    confirm: SharedString,
    destructive: bool,
    on_confirm: Option<Run>,
    on_close: Run,
}

impl ConfirmDialog {
    /// `on_close` runs on every way out, the confirm button included.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: message.into(),
            confirm: "Confirm".into(),
            destructive: false,
            on_confirm: None,
            on_close: Rc::new(on_close),
        }
    }

    /// The confirming button's label.
    pub fn confirm(mut self, label: impl Into<SharedString>) -> Self {
        self.confirm = label.into();
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn on_confirm(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ConfirmDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (id, on_confirm, label) = (self.id.clone(), self.on_confirm, self.confirm);
        let variant = if self.destructive {
            ButtonVariant::Danger
        } else {
            ButtonVariant::Primary
        };
        let (cancel_id, confirm_id) = ((self.id.clone(), "cancel"), (self.id.clone(), "confirm"));
        let on_close = self.on_close;
        let cancel = i18n::text(cx, "dialog.cancel", &[]);
        Dialog::new(self.id, self.title, move |window, cx| on_close(window, cx))
            .child(said(None, self.message, cx))
            .action(move |close| {
                Button::new(cancel_id, cancel)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .action(move |close| {
                Button::new(confirm_id, label)
                    .variant(variant)
                    .on_click(move |_, window, cx| {
                        log::info!("confirm dialog {id:?}: confirmed");
                        close(window, cx);
                        if let Some(on_confirm) = &on_confirm {
                            on_confirm(window, cx);
                        }
                    })
            })
    }
}

type Check = Rc<dyn Fn(&str) -> Result<(), SharedString>>;
type Submit = Rc<dyn Fn(&str, &mut Window, &mut App)>;
type Send = Rc<dyn Fn(&Close, &mut Window, &mut App)>;

/// Asks for one line, its text selected so typing replaces it. Enter or the submit button sends it once the check passes; the check's reason shows under the field.
#[derive(IntoElement)]
pub struct PromptDialog {
    id: ElementId,
    title: SharedString,
    field: Entity<TextInput>,
    label: Option<SharedString>,
    submit: SharedString,
    check: Option<Check>,
    on_submit: Option<Submit>,
    on_close: Run,
}

impl PromptDialog {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        field: &Entity<TextInput>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            field: field.clone(),
            label: None,
            submit: "OK".into(),
            check: None,
            on_submit: None,
            on_close: Rc::new(on_close),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The submit button's label.
    pub fn submit(mut self, label: impl Into<SharedString>) -> Self {
        self.submit = label.into();
        self
    }

    /// Says why the text cannot go yet.
    pub fn check(mut self, check: impl Fn(&str) -> Result<(), SharedString> + 'static) -> Self {
        self.check = Some(Rc::new(check));
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PromptDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let primed = window.use_keyed_state((self.id.clone(), "primed"), cx, |_, _| false);
        if !*primed.read(cx) {
            primed.update(cx, |primed, _| *primed = true);
            self.field.update(cx, |input, cx| {
                let end = input.text().len();
                input.select(0..end, cx);
            });
        }
        let text = self.field.read(cx).text().to_string();
        let verdict = match &self.check {
            Some(check) => check(&text),
            None => Ok(()),
        };
        let ready = verdict.is_ok();
        let send: Send = {
            let (id, field, check, on_submit) = (
                self.id.clone(),
                self.field.clone(),
                self.check.clone(),
                self.on_submit.clone(),
            );
            Rc::new(move |close, window, cx| {
                let text = field.read(cx).text().to_string();
                if let Some(check) = &check
                    && let Err(reason) = check(&text)
                {
                    log::info!("prompt dialog {id:?}: held back, {reason}");
                    return;
                }
                log::info!("prompt dialog {id:?}: sent");
                close(window, cx);
                if let Some(on_submit) = &on_submit {
                    on_submit(&text, window, cx);
                }
            })
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let (button, lifted) = (send.clone(), send);
        let field = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .when_some(self.label, |field, label| {
                field.child(
                    div()
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(colors.fg_muted)
                        .child(label),
                )
            })
            .child(Input::new(&self.field).invalid(!ready && !text.is_empty()))
            .when_some(
                verdict.err().filter(|_| !text.is_empty()),
                |field, reason| {
                    field.child(
                        div()
                            .id((self.id.clone(), "error"))
                            .role(Role::Label)
                            .aria_label(reason.clone())
                            .text_size(theme.text_size(TextSize::Xs))
                            .text_color(colors.danger)
                            .child(reason),
                    )
                },
            );
        let (cancel_id, submit_id, label) = (
            (self.id.clone(), "cancel"),
            (self.id.clone(), "submit"),
            self.submit,
        );
        let cancel = i18n::text(cx, "dialog.cancel", &[]);
        let on_close = self.on_close;
        Dialog::new(self.id, self.title, move |window, cx| on_close(window, cx))
            .focus_first(self.field.read(cx).focus().clone())
            .on_enter(move |close| Rc::new(move |window, cx| lifted(&close, window, cx)))
            .child(field)
            .action(move |close| {
                Button::new(cancel_id, cancel)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .action(move |close| {
                Button::new(submit_id, label)
                    .primary()
                    .disabled(!ready)
                    .on_click(move |_, window, cx| button(&close, window, cx))
            })
    }
}
