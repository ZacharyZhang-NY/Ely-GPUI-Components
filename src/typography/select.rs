use std::ops::Range;

use gpui::{
    App, ClipboardItem, CursorStyle, ElementId, FocusHandle, HighlightStyle, InteractiveElement,
    IntoElement, MouseButton, ParentElement, RenderOnce, SharedString, Styled, StyledText,
    TextLayout, Window, div,
};

use crate::theme::ActiveTheme;

struct Selection {
    text: SharedString,
    focus: FocusHandle,
    anchor: usize,
    head: usize,
    dragging: bool,
}

impl Selection {
    fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

fn index_at(layout: &TextLayout, position: gpui::Point<gpui::Pixels>) -> usize {
    match layout.index_for_position(position) {
        Ok(ix) | Err(ix) => ix,
    }
}

/// Read-only text the pointer can select and copy.
#[derive(IntoElement)]
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
}

impl SelectableText {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl RenderOnce for SelectableText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| Selection {
            text: self.text.clone(),
            focus: cx.focus_handle(),
            anchor: 0,
            head: 0,
            dragging: false,
        });
        if state.read(cx).text != self.text {
            state.update(cx, |selection, _| {
                selection.text = self.text.clone();
                selection.anchor = 0;
                selection.head = 0;
            });
        }
        let (focus, range) = {
            let selection = state.read(cx);
            (selection.focus.clone(), selection.range())
        };
        let highlight = HighlightStyle {
            background_color: Some(cx.theme().colors.selection),
            ..Default::default()
        };
        let styled = StyledText::new(self.text.clone())
            .with_highlights((!range.is_empty()).then_some((range, highlight)));
        let layout = styled.layout().clone();
        let (down, drag, up, keys) = (state.clone(), state.clone(), state.clone(), state);
        let (down_layout, drag_layout) = (layout.clone(), layout);
        let text = self.text;

        div()
            .id(self.id)
            .track_focus(&focus)
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let ix = index_at(&down_layout, event.position);
                down.update(cx, |selection, cx| {
                    window.focus(&selection.focus, cx);
                    if !event.modifiers.shift {
                        selection.anchor = ix;
                    }
                    selection.head = ix;
                    selection.dragging = true;
                    cx.notify();
                });
            })
            .on_mouse_move(move |event, _, cx| {
                drag.update(cx, |selection, cx| {
                    if selection.dragging && event.dragging() {
                        selection.head = index_at(&drag_layout, event.position);
                        cx.notify();
                    }
                })
            })
            .on_mouse_up(MouseButton::Left, {
                let up = up.clone();
                move |_, _, cx| up.update(cx, |selection, _| selection.dragging = false)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |selection, _| selection.dragging = false)
            })
            .on_key_down(move |event, _, cx| {
                let stroke = &event.keystroke;
                if !stroke.modifiers.secondary(cx) {
                    return;
                }
                match stroke.key.as_str() {
                    "c" => {
                        let range = keys.read(cx).range();
                        if !range.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                text[range].to_string(),
                            ));
                        }
                    }
                    "a" => keys.update(cx, |selection, cx| {
                        selection.anchor = 0;
                        selection.head = text.len();
                        cx.notify();
                    }),
                    _ => {}
                }
            })
            .child(styled)
    }
}
