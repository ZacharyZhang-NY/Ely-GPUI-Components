use std::rc::Rc;

use gpui::{
    Action, AnyElement, App, ElementId, Entity, InteractiveElement, IntoElement, KeyBinding,
    KeyBindingContextPredicate, ParentElement, RenderOnce, SharedString, Styled, Window, actions,
    div, prelude::*,
};

use super::{
    counts::{ReadingTime, WordCount},
    format::{Format, active, format},
    link::LinkEditor,
    suggest::offers,
    toolbar::{FixedFormatToolbar, INLINE, tool},
};
use crate::{
    forms::{Run, TextInput},
    overlays::FloatingToolbar,
    theme::{ActiveTheme, Radius, TextSize},
};

actions!(ely_rich_text, [ToggleBold, ToggleItalic, EditLink]);

fn binding<A: Action>(cx: &App, strokes: &str, action: A, context: Option<&str>) -> KeyBinding {
    let predicate =
        context.map(|c| Rc::new(KeyBindingContextPredicate::parse(c).expect("context")));
    KeyBinding::load(strokes, Box::new(action), predicate, false, None, cx)
        .unwrap_or_else(|error| panic!("bad keystroke {strokes:?}: {error}"))
}

const CONTEXT: &str = "ElyRichText";

/// Binds the rich text keys. `init` calls it.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        binding(cx, "secondary-b", ToggleBold, Some(CONTEXT)),
        binding(cx, "secondary-i", ToggleItalic, Some(CONTEXT)),
        binding(cx, "secondary-k", EditLink, Some(CONTEXT)),
    ]);
}

/// Markdown that reads as it is written: styles show as they are typed, the toolbar above and the one over a selection apply them, a link opens its editor, `@` and `:` suggest people and emoji, and the foot counts words and minutes.
#[derive(IntoElement)]
pub struct RichTextEditor {
    id: ElementId,
    field: Entity<TextInput>,
    people: Vec<SharedString>,
    end: Option<AnyElement>,
    beside: Option<AnyElement>,
}

impl RichTextEditor {
    /// Give the field several lines and `markdown_highlights` as its highlighter.
    pub fn new(id: impl Into<ElementId>, field: &Entity<TextInput>) -> Self {
        Self {
            id: id.into(),
            field: field.clone(),
            people: Vec::new(),
            end: None,
            beside: None,
        }
    }

    /// Sits at the toolbar's far end, such as a switch of views.
    pub fn toolbar_end(mut self, element: impl IntoElement) -> Self {
        self.end = Some(element.into_any_element());
        self
    }

    /// Sits beside the text, as a preview does.
    pub fn beside(mut self, element: impl IntoElement) -> Self {
        self.beside = Some(element.into_any_element());
        self
    }

    /// Handles `@` suggests.
    pub fn people(mut self, handles: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.people = handles.into_iter().map(Into::into).collect();
        self
    }
}

impl RenderOnce for RichTextEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input = self.field.read(cx);
        assert!(
            input.is_multi_line(),
            "rich text {:?} needs a field of several lines",
            self.id
        );
        let text = SharedString::from(input.text().to_string());
        let pressed = active(input.text(), input.selection());
        let linking = window.use_keyed_state((self.id.clone(), "linking"), cx, |_, _| false);
        let set_linking = |open: bool| -> Run {
            let linking = linking.clone();
            Rc::new(move |_, cx| {
                log::info!(
                    "rich text: link editor {}",
                    if open { "opens" } else { "closes" }
                );
                linking.update(cx, |linking, cx| {
                    *linking = open;
                    cx.notify();
                });
            })
        };
        let (open_link, close_link) = (set_linking(true), set_linking(false));
        let float_id: ElementId = (self.id.clone(), "float").into();
        let platform = cx.theme().platform;
        let floating = FloatingToolbar::new(float_id.clone(), &self.field).children(
            INLINE.iter().map(|tool_of| {
                let on = pressed.contains(&tool_of.0);
                tool(
                    &float_id,
                    &self.field,
                    *tool_of,
                    on,
                    Some(open_link.clone()),
                    platform,
                )
            }),
        );
        let suggestions = offers(
            (self.id.clone(), "offers").into(),
            &self.field,
            &self.people,
            None,
            cx,
        )
        .wrap(self.field.clone(), window, cx);
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let (bold, italic, link) = (self.field.clone(), self.field.clone(), open_link.clone());
        div()
            .id(self.id.clone())
            .key_context(CONTEXT)
            .on_action(move |_: &ToggleBold, _, cx| format(&bold, Format::Bold, cx))
            .on_action(move |_: &ToggleItalic, _, cx| format(&italic, Format::Italic, cx))
            .on_action(move |_: &EditLink, window, cx| link(window, cx))
            .flex()
            .flex_col()
            .rounded(theme.radius(Radius::Lg))
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        FixedFormatToolbar::new((self.id.clone(), "tools"), &self.field)
                            .on_link(move |window, cx| open_link(window, cx)),
                    )
                    .children(self.end),
            )
            .child(
                div()
                    .flex()
                    .child(div().flex_1().min_w_0().px_4().py_3().child(suggestions))
                    .children(self.beside.map(|beside| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_4()
                            .py_3()
                            .border_l_1()
                            .border_color(colors.border)
                            .child(beside)
                    })),
            )
            .child(floating)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(colors.border)
                    .text_size(theme.text_size(TextSize::Xs))
                    .child(WordCount::new(text.clone()))
                    .child(ReadingTime::new(text)),
            )
            .when(*linking.read(cx), |editor| {
                editor.child(LinkEditor::new(
                    (self.id.clone(), "link"),
                    &self.field,
                    move |window, cx| close_link(window, cx),
                ))
            })
    }
}
