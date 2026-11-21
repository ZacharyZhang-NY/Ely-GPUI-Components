use std::sync::Arc;

use gpui::{
    App, Bounds, DragMoveEvent, ElementId, EmptyView, EntityId, ImageAssetLoader, ImageSource,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderImage,
    RenderOnce, Resource, StatefulInteractiveElement, Styled, Window, canvas, div, img, prelude::*,
    transparent_black,
};

use super::turn::{Turn, Turned};
use crate::{
    buttons::IconButton,
    documents::zoom_step,
    motion::Spinner,
    primitives::{FocusRing, Icon, IconName, tab_stop},
    theme::{ActiveTheme, ControlSize, IconSize, TextSize},
    typography::tabular,
};

/// The zoom that shows the whole picture in the stage, never past actual size.
pub(crate) fn fit(picture: (f32, f32), stage: (f32, f32)) -> f32 {
    (stage.0 / picture.0).min(stage.1 / picture.1).min(1.0)
}

/// An offset of the picture's middle from the stage's middle, kept so no edge comes inside the stage while the picture overhangs it, and centered where it fits.
pub(crate) fn held(offset: (f32, f32), shown: (f32, f32), stage: (f32, f32)) -> (f32, f32) {
    let reach = |picture: f32, stage: f32| ((picture - stage) / 2.0).max(0.0);
    let (x, y) = (reach(shown.0, stage.0), reach(shown.1, stage.1));
    (offset.0.clamp(-x, x), offset.1.clamp(-y, y))
}

/// The offset after zooming from `from` to `to` about `at`, a place from the stage's middle, so the spot under it stays put.
pub(crate) fn zoom_about(offset: (f32, f32), at: (f32, f32), from: f32, to: f32) -> (f32, f32) {
    let scale = to / from;
    (
        at.0 - (at.0 - offset.0) * scale,
        at.1 - (at.1 - offset.1) * scale,
    )
}

/// Most a picture grows, as the document zoom steps do.
const MOST: f32 = 4.0;
/// Share of the stage an arrow key pans.
const PAN: f32 = 0.1;

/// The view's own state: the picture it shows; its zoom, none while it fits; its offset in points; its quarter turns; the stage's bounds; and where a drag was last.
#[derive(Default)]
struct View {
    source: Option<Resource>,
    zoom: Option<f32>,
    offset: (f32, f32),
    turns: u8,
    stage: Bounds<Pixels>,
    from: Point<Pixels>,
    failed: bool,
}

/// What a toolbar button does to the view, given the picture's size.
type Act = Box<dyn Fn(&mut View, (f32, f32))>;

/// A panning drag, marked with its viewer.
struct Pan(EntityId);

impl View {
    fn size(&self) -> (f32, f32) {
        (
            f32::from(self.stage.size.width),
            f32::from(self.stage.size.height),
        )
    }

    /// Zooms to `to`, or to fit, about `at`, keeping the offset held.
    fn zoom_to(&mut self, to: Option<f32>, at: (f32, f32), picture: (f32, f32)) {
        let stage = self.size();
        let fitted = fit(picture, stage);
        let (from, next) = (
            self.zoom.unwrap_or(fitted),
            to.unwrap_or(fitted).clamp(fitted.min(0.25), MOST),
        );
        let offset = zoom_about(self.offset, at, from, next);
        self.zoom = to.map(|_| next);
        self.offset = held(offset, (picture.0 * next, picture.1 * next), stage);
        log::info!("image viewer: zoom {:.0}%", next * 100.0);
    }

    fn pan(&mut self, by: (f32, f32), picture: (f32, f32)) {
        let zoom = self.zoom.unwrap_or_else(|| fit(picture, self.size()));
        let offset = (self.offset.0 + by.0, self.offset.1 + by.1);
        self.offset = held(offset, (picture.0 * zoom, picture.1 * zoom), self.size());
    }
}

/// A picture to look at closely. It fits its stage at first, and again when the host gives it another; the buttons and the plus and minus keys zoom by steps and Cmd-scroll smoothly, 0 fits and 1 shows actual size, a double press toggles the two; a drag, a scroll or the arrows pan once it overhangs; R turns it a quarter clockwise and Shift-R back. The host gives it a height.
#[derive(IntoElement)]
pub struct ImageViewer {
    id: ElementId,
    source: Resource,
}

impl ImageViewer {
    /// `source` is a file or a web address.
    pub fn new(id: impl Into<ElementId>, source: impl Into<Resource>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
        }
    }
}

impl RenderOnce for ImageViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = window.use_keyed_state((self.id.clone(), "view"), cx, |_, _| View::default());
        if view.read(cx).source.as_ref() != Some(&self.source) {
            view.update(cx, |view, _| {
                *view = View {
                    source: Some(self.source.clone()),
                    stage: view.stage,
                    ..View::default()
                }
            });
        }
        let decoded = window.use_asset::<ImageAssetLoader>(&self.source, cx);
        let turns = view.read(cx).turns;
        let picture: Option<Arc<RenderImage>> = match decoded {
            Some(Ok(image)) if turns.is_multiple_of(4) => Some(image),
            Some(Ok(image)) => window.use_asset::<Turned>(&Turn(image, turns), cx),
            Some(Err(error)) => {
                if !view.read(cx).failed {
                    log::error!(
                        "image viewer {:?}: {:?} can't be opened: {error}",
                        self.id,
                        self.source
                    );
                    view.update(cx, |view, _| view.failed = true);
                }
                None
            }
            None => None,
        };
        let failed = view.read(cx).failed;
        let pixels = picture.as_ref().map(|image| {
            let size = image.size(0);
            (size.width.0 as f32, size.height.0 as f32)
        });
        let focus = tab_stop(
            (self.id.clone(), "focus").into(),
            pixels.is_some(),
            window,
            cx,
        );
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let (zoom, offset, stage) = {
            let view = view.read(cx);
            let stage = view.size();
            let zoom = pixels.map(|pixels| view.zoom.unwrap_or_else(|| fit(pixels, stage)));
            (zoom, view.offset, stage)
        };
        let button = |key: &'static str, icon: IconName, tip: &'static str, act: Act| {
            let (acted, pixels) = (view.clone(), pixels);
            IconButton::new((self.id.clone(), key), icon)
                .size(ControlSize::Sm)
                .tooltip(tip)
                .disabled(pixels.is_none())
                .when_some(pixels, |button, pixels| {
                    button.on_click(move |_, _, cx| {
                        acted.update(cx, |view, cx| {
                            act(view, pixels);
                            cx.notify();
                        })
                    })
                })
        };
        let step = move |up: bool| {
            move |view: &mut View, pixels: (f32, f32)| {
                let now = view.zoom.unwrap_or_else(|| fit(pixels, view.size()));
                if let Some(next) = zoom_step(now, up) {
                    view.zoom_to(Some(next), (0.0, 0.0), pixels);
                }
            }
        };
        let turn = move |by: u8| {
            move |view: &mut View, _: (f32, f32)| {
                view.turns = (view.turns + by) % 4;
                (view.zoom, view.offset) = (None, (0.0, 0.0));
                log::info!("image viewer: turned to {} quarters", view.turns);
            }
        };
        let toolbar = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(button(
                        "out",
                        IconName::ZoomOut,
                        "Zoom out",
                        Box::new(step(false)),
                    ))
                    .child(
                        tabular(div())
                            .w(theme.label_width() * 0.3)
                            .text_size(theme.text_size(TextSize::Xs))
                            .text_color(colors.fg_muted)
                            .flex()
                            .justify_center()
                            .child(
                                zoom.map(|zoom| format!("{:.0}%", zoom * 100.0))
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(button(
                        "in",
                        IconName::ZoomIn,
                        "Zoom in",
                        Box::new(step(true)),
                    ))
                    .child(button(
                        "fit",
                        IconName::Maximize2,
                        "Fit",
                        Box::new(|view, pixels| view.zoom_to(None, (0.0, 0.0), pixels)),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(button(
                        "left",
                        IconName::RotateCcw,
                        "Turn left",
                        Box::new(turn(3)),
                    ))
                    .child(button(
                        "right",
                        IconName::RotateCw,
                        "Turn right",
                        Box::new(turn(1)),
                    )),
            );
        let shown = pixels.zip(zoom).map(|(pixels, zoom)| {
            let size = (pixels.0 * zoom, pixels.1 * zoom);
            let (left, top) = (
                (stage.0 - size.0) / 2.0 + offset.0,
                (stage.1 - size.1) / 2.0 + offset.1,
            );
            (size, left, top)
        });
        let body = match (picture, shown) {
            (Some(image), Some((size, left, top))) => img(ImageSource::Render(image))
                .id((self.id.clone(), "picture"))
                .debug_selector(|| "image-viewer-picture".into())
                .absolute()
                .left(Pixels::from(left))
                .top(Pixels::from(top))
                .w(Pixels::from(size.0))
                .h(Pixels::from(size.1))
                .into_any_element(),
            _ if failed => div()
                .debug_selector(|| "image-viewer-failed".into())
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .text_size(theme.text_size(TextSize::Sm))
                .text_color(colors.fg_muted)
                .child(
                    Icon::new(IconName::ImageOff)
                        .size(IconSize::Lg)
                        .color(colors.fg_subtle),
                )
                .child("This picture can't be opened")
                .into_any_element(),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new((self.id.clone(), "loading")).size(IconSize::Md))
                .into_any_element(),
        };
        let overhangs =
            shown.is_some_and(|(size, _, _)| size.0 > stage.0 + 0.5 || size.1 > stage.1 + 0.5);
        let owner = view.entity_id();
        let (measure, wheel, pressed, moved, keys) = (
            view.clone(),
            view.clone(),
            view.clone(),
            view.clone(),
            view.clone(),
        );
        let stage_box = div()
            .id(self.id.clone())
            .debug_selector(|| "image-viewer-stage".into())
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .bg(colors.sunken)
            .border_1()
            .border_color(transparent_black())
            .when(pixels.is_some(), |stage| {
                stage.track_focus(&focus).focus_ring(cx)
            })
            .when(overhangs, |stage| stage.cursor_grab())
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if measure.read(cx).stage != bounds {
                            measure.update(cx, |view, cx| {
                                view.stage = bounds;
                                cx.notify();
                            });
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(body)
            .when_some(pixels, |stage, pixels| {
                stage
                    .on_scroll(move |event, window, cx| {
                        let delta = event.delta.pixel_delta(window.line_height());
                        let zooming = event.modifiers.platform || event.modifiers.control;
                        wheel.update(cx, |view, cx| {
                            if zooming {
                                let now = view.zoom.unwrap_or_else(|| fit(pixels, view.size()));
                                let at = event.position - view.stage.center();
                                let next = now * (-f32::from(delta.y) * 0.01).exp();
                                view.zoom_to(
                                    Some(next),
                                    (f32::from(at.x), f32::from(at.y)),
                                    pixels,
                                );
                            } else {
                                let before = view.offset;
                                view.pan((f32::from(delta.x), f32::from(delta.y)), pixels);
                                if view.offset == before {
                                    return;
                                }
                            }
                            cx.stop_propagation();
                            cx.notify();
                        })
                    })
                    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                        pressed.update(cx, |view, cx| {
                            view.from = event.position;
                            if event.click_count == 2 {
                                let at = event.position - view.stage.center();
                                let to = view.zoom.is_none().then_some(1.0);
                                view.zoom_to(to, (f32::from(at.x), f32::from(at.y)), pixels);
                                cx.notify();
                            }
                        })
                    })
                    .on_drag(Pan(owner), |_, _, _, cx| cx.new(|_| EmptyView))
                    .on_drag_move(move |event: &DragMoveEvent<Pan>, _, cx| {
                        if event.drag(cx).0 != owner {
                            return;
                        }
                        moved.update(cx, |view, cx| {
                            let by = event.event.position - view.from;
                            view.from = event.event.position;
                            view.pan((f32::from(by.x), f32::from(by.y)), pixels);
                            cx.notify();
                        })
                    })
                    .on_key_down(move |event, _, cx| {
                        let key = event.keystroke.key.as_str();
                        let shift = event.keystroke.modifiers.shift;
                        keys.update(cx, |view, cx| {
                            let (w, h) = view.size();
                            let now = view.zoom.unwrap_or_else(|| fit(pixels, (w, h)));
                            match key {
                                "+" | "=" => {
                                    if let Some(next) = zoom_step(now, true) {
                                        view.zoom_to(Some(next), (0.0, 0.0), pixels);
                                    }
                                }
                                "-" => {
                                    if let Some(next) = zoom_step(now, false) {
                                        view.zoom_to(Some(next), (0.0, 0.0), pixels);
                                    }
                                }
                                "0" => view.zoom_to(None, (0.0, 0.0), pixels),
                                "1" => view.zoom_to(Some(1.0), (0.0, 0.0), pixels),
                                "r" => turn(if shift { 3 } else { 1 })(view, pixels),
                                "left" => view.pan((w * PAN, 0.0), pixels),
                                "right" => view.pan((-w * PAN, 0.0), pixels),
                                "up" => view.pan((0.0, h * PAN), pixels),
                                "down" => view.pan((0.0, -h * PAN), pixels),
                                _ => return,
                            }
                            cx.stop_propagation();
                            cx.notify();
                        })
                    })
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(toolbar)
            .child(stage_box)
    }
}

#[cfg(test)]
mod tests {
    use super::{fit, held, zoom_about};

    #[test]
    fn a_big_picture_fits_by_its_tighter_side_and_a_small_one_keeps_its_size() {
        assert_eq!(fit((800.0, 400.0), (400.0, 300.0)), 0.5);
        assert_eq!(fit((300.0, 600.0), (400.0, 300.0)), 0.5);
        assert_eq!(fit((100.0, 50.0), (400.0, 300.0)), 1.0);
    }

    #[test]
    fn an_overhanging_picture_pans_to_its_edges_and_one_that_fits_stays_centered() {
        assert_eq!(
            held((500.0, 90.0), (800.0, 400.0), (400.0, 300.0)),
            (200.0, 50.0)
        );
        assert_eq!(
            held((-500.0, 90.0), (300.0, 200.0), (400.0, 300.0)),
            (0.0, 0.0)
        );
    }

    #[test]
    fn zooming_about_a_place_keeps_the_spot_under_it() {
        let (offset, at) = ((10.0, -20.0), (100.0, 50.0));
        let after = zoom_about(offset, at, 1.0, 2.0);
        let spot =
            |offset: (f32, f32), zoom: f32| ((at.0 - offset.0) / zoom, (at.1 - offset.1) / zoom);
        assert_eq!(spot(offset, 1.0), spot(after, 2.0));
    }
}
