use std::{collections::HashSet, f64::consts::LOG2_E, rc::Rc};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, ElementId, EmptyView, Entity, EntityId, FocusHandle,
    ImageSource, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div,
};

use super::{
    MapViewport, Tile,
    cluster::Laying,
    layer::{MapLayer, painted},
    marker::{MapMarker, MapPopup, OnMarker},
    tile::laid_tile,
};
use crate::{
    buttons::{ButtonVariant, IconButton},
    layout::seeded::use_seeded,
    primitives::{FocusRing, IconName, Takeover, give_back, tab_stop, take_focus},
    theme::{ActiveTheme, Radius, TextSize},
};

type TileSource = Rc<dyn Fn(Tile) -> ImageSource>;
pub(crate) type OnMapViewport = Rc<dyn Fn(MapViewport, &mut Window, &mut App)>;
/// The viewport and the view's size as they stand now.
type Now = Rc<dyn Fn(&App) -> (MapViewport, (f32, f32))>;

/// A drag that pans a map, and whose map it is.
struct Pan {
    owner: EntityId,
}

/// The map's box, the pointer and viewport at the press of a pan under way, the focus a shown popup took, and the focus of each pin and cluster a zoom may regroup.
#[derive(Default)]
struct Stage {
    bounds: Bounds<Pixels>,
    grip: Option<(Point<Pixels>, MapViewport)>,
    takeover: Option<Entity<Takeover>>,
    gathered: Vec<FocusHandle>,
}

/// A map of tiles the host supplies by zoom, column and row, with pins and a popup over them; a pin that holds focus keeps it off view, but takes Tab and presses only while its place is in view. A drag or the wheel pans it; Command or Control with the wheel zooms about the pointer; focused, the arrows pan and + and - zoom, as its buttons do. The owner keeps the viewport; a new one from the owner shows at once. It fills its box; the host gives it a height.
#[derive(IntoElement)]
pub struct MapView {
    id: ElementId,
    pub(crate) viewport: MapViewport,
    tiles: Option<TileSource>,
    zooms: (u8, u8),
    pub(crate) attribution: Option<SharedString>,
    layers: Vec<MapLayer>,
    markers: Vec<MapMarker>,
    gathered: bool,
    on_marker: Option<OnMarker>,
    popup: Option<MapPopup>,
    pub(crate) on_viewport: Option<OnMapViewport>,
}

impl MapView {
    pub fn new(id: impl Into<ElementId>, viewport: MapViewport) -> Self {
        Self {
            id: id.into(),
            viewport,
            tiles: None,
            zooms: (0, MapViewport::ZOOMS.1 as u8),
            attribution: None,
            layers: Vec::new(),
            markers: Vec::new(),
            gathered: false,
            on_marker: None,
            popup: None,
            on_viewport: None,
        }
    }

    /// The picture of each tile: a web address, a file, or a picture drawn in code.
    pub fn tiles(mut self, source: impl Fn(Tile) -> ImageSource + 'static) -> Self {
        self.tiles = Some(Rc::new(source));
        self
    }

    /// The zooms the host cuts tiles at; nearer in, the nearest cut scales up.
    pub fn tile_zooms(mut self, least: u8, most: u8) -> Self {
        assert!(
            least <= most && f64::from(most) <= MapViewport::ZOOMS.1,
            "tile zooms {least} to {most}"
        );
        self.zooms = (least, most);
        self
    }

    /// Whose tiles and data these are, in the corner, as their licenses ask.
    pub fn attribution(mut self, text: impl Into<SharedString>) -> Self {
        self.attribution = Some(text.into());
        self
    }

    /// A route, GeoJSON features or heat over the tiles and under the pins, drawn in the order added.
    pub fn layer(mut self, layer: impl Into<MapLayer>) -> Self {
        self.layers.push(layer.into());
        self
    }

    /// Gathers pins that crowd one square of the world into a cluster that shows their count; a cluster's press shows its members whole. A zoom that regroups them hands a focused pin's or cluster's focus to the map.
    pub fn cluster_markers(mut self) -> Self {
        self.gathered = true;
        self
    }

    /// Pins keyed by their places; two with one key fail, as they would share a focus.
    pub fn markers(mut self, markers: impl IntoIterator<Item = MapMarker>) -> Self {
        self.markers = markers.into_iter().collect();
        let mut keys = HashSet::new();
        for marker in &self.markers {
            assert!(
                keys.insert(marker.key.clone()),
                "map {:?}: two markers keyed {}",
                self.id,
                marker.key
            );
        }
        self
    }

    /// Hears a pressed marker by its key; without it, markers take no press.
    pub fn on_marker(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_marker = Some(Rc::new(handler));
        self
    }

    /// A card at a place; Escape on the focused map closes it too.
    pub fn popup(mut self, popup: MapPopup) -> Self {
        self.popup = Some(popup);
        self
    }

    /// Gets the viewport after each pan and zoom.
    pub fn on_viewport(
        mut self,
        handler: impl Fn(MapViewport, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_viewport = Some(Rc::new(handler));
        self
    }
}

fn sized(bounds: Bounds<Pixels>) -> (f32, f32) {
    (f32::from(bounds.size.width), f32::from(bounds.size.height))
}

/// Whether a view point lies in a view of `size`.
fn inside((x, y): (f32, f32), (w, h): (f32, f32)) -> bool {
    (0.0..=w).contains(&x) && (0.0..=h).contains(&y)
}

impl RenderOnce for MapView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let local = use_seeded((id.clone(), "view"), self.viewport, window, cx);
        let stage: Entity<Stage> =
            window.use_keyed_state((id.clone(), "stage"), cx, |_, _| Stage::default());
        let focus = tab_stop((id.clone(), "focus").into(), true, window, cx);
        let view = local.read(cx).value;
        let size = sized(stage.read(cx).bounds);
        let tile = f32::from(cx.theme().maps().tile);
        let set: OnMapViewport = {
            let (local, on_viewport) = (local.clone(), self.on_viewport.clone());
            let (regrouped, map) = (stage.clone(), focus.clone());
            Rc::new(move |next, window, cx| {
                log::debug!(
                    "map: {:.4}, {:.4} at zoom {:.2}",
                    next.center.lat,
                    next.center.lon,
                    next.zoom
                );
                let zoomed = next.zoom != local.read(cx).value.zoom;
                let held = regrouped
                    .read(cx)
                    .gathered
                    .iter()
                    .any(|pin| pin.is_focused(window));
                if zoomed && held {
                    log::info!("map: a zoom regroups the pins; focus goes to the map");
                    window.focus(&map, cx);
                }
                local.update(cx, |local, cx| {
                    local.value = next;
                    cx.notify();
                });
                if let Some(on_viewport) = &on_viewport {
                    on_viewport(next, window, cx);
                }
            })
        };
        let tiles: Vec<AnyElement> = match &self.tiles {
            Some(source) => view
                .tiles(size, tile, self.zooms)
                .iter()
                .map(|laid| laid_tile(laid, source(laid.tile), window, cx))
                .collect(),
            None => Vec::new(),
        };
        let layers = painted(self.layers, view, size, tile, window, cx);
        let pins = Laying {
            markers: self.markers,
            on_marker: self.on_marker,
            gathered: self.gathered,
        }
        .lay(&id, (view, size, tile), &set, window, cx);
        stage.update(cx, |stage, _| stage.gathered = pins.gathered);
        let shown = self
            .popup
            .map(|popup| (view.to_view(popup.at, size, tile), popup))
            .filter(|(at, _)| inside(*at, size));
        let closer = shown.as_ref().and_then(|(_, popup)| popup.closer());
        let popup = match shown {
            Some((at, popup)) => {
                let takeover = take_focus((id.clone(), "popup"), window, cx);
                stage.update(cx, |stage, _| stage.takeover = Some(takeover.clone()));
                let held = takeover.read(cx).focus.clone();
                popup.place(&id, at, size, &held, window, cx)
            }
            None => {
                if let Some(taken) = stage.update(cx, |stage, _| stage.takeover.take()) {
                    give_back(&taken, window, cx);
                }
                None
            }
        };
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let now: Now = {
            let (local, stage) = (local.clone(), stage.clone());
            Rc::new(move |cx| (local.read(cx).value, sized(stage.read(cx).bounds)))
        };
        let zoom = |levels: f64, key: &'static str, icon: IconName, words: &'static str| {
            let (now, set) = (now.clone(), set.clone());
            let (least, most) = MapViewport::ZOOMS;
            let spent = if levels > 0.0 {
                view.zoom >= most
            } else {
                view.zoom <= least
            };
            IconButton::new((id.clone(), key), icon)
                .variant(ButtonVariant::Secondary)
                .tooltip(words)
                .disabled(spent)
                .on_click(move |_, window, cx| {
                    let (view, size) = now(cx);
                    set(
                        view.zoomed(levels, (size.0 / 2.0, size.1 / 2.0), size, tile),
                        window,
                        cx,
                    );
                })
        };
        let controls = div()
            .absolute()
            .top_2()
            .right_2()
            .occlude()
            .flex()
            .flex_col()
            .gap_1()
            .child(zoom(1.0, "zoom-in", IconName::Plus, "Zoom in"))
            .child(zoom(-1.0, "zoom-out", IconName::Minus, "Zoom out"));
        let attribution = self.attribution.map(|text| {
            div()
                .absolute()
                .bottom_0()
                .right_0()
                .max_w_full()
                .px_1()
                .rounded_tl(theme.radius(Radius::Sm))
                .bg(colors.bg.alpha(0.85))
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(colors.fg_muted)
                .child(text)
        });
        let measured = {
            let stage = stage.clone();
            canvas(
                move |bounds, window, cx| {
                    if stage.read(cx).bounds != bounds {
                        stage.update(cx, |stage, cx| {
                            stage.bounds = bounds;
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
            .size_full()
        };
        let owner = stage.entity_id();
        let (keyed, wheeled, held) = (now.clone(), now.clone(), now);
        let (by_key, by_wheel, by_drag) = (set.clone(), set.clone(), set);
        let (scrolled, pressed, dragged, dropped) =
            (stage.clone(), stage.clone(), stage.clone(), stage);
        div()
            .id(id)
            .track_focus(&focus)
            .relative()
            .h_full()
            .overflow_hidden()
            .bg(colors.sunken)
            .border_1()
            .border_color(gpui::transparent_black())
            .focus_ring(cx)
            .cursor_grab()
            .on_key_down(move |event, window, cx| {
                let held = &event.keystroke.modifiers;
                if held.platform || held.control || held.alt {
                    return;
                }
                let (view, size) = keyed(cx);
                let step = size.0.min(size.1) / 4.0;
                let middle = (size.0 / 2.0, size.1 / 2.0);
                let next = match event.keystroke.key.as_str() {
                    "left" => view.panned((step, 0.0), size, tile),
                    "right" => view.panned((-step, 0.0), size, tile),
                    "up" => view.panned((0.0, step), size, tile),
                    "down" => view.panned((0.0, -step), size, tile),
                    "+" | "=" => view.zoomed(1.0, middle, size, tile),
                    "-" => view.zoomed(-1.0, middle, size, tile),
                    "escape" => {
                        if let Some(close) = &closer {
                            cx.stop_propagation();
                            close(window, cx);
                        }
                        return;
                    }
                    _ => return,
                };
                cx.stop_propagation();
                by_key(next, window, cx);
            })
            .on_scroll(move |event, window, cx| {
                let delta = event.delta.pixel_delta(window.line_height());
                let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
                if (dx, dy) == (0.0, 0.0) {
                    return;
                }
                let (view, size) = wheeled(cx);
                let pointer = event.position - scrolled.read(cx).bounds.origin;
                let about = (f32::from(pointer.x), f32::from(pointer.y));
                let next = if event.modifiers.platform || event.modifiers.control {
                    view.zoomed(-f64::from(dy) / 240.0 * LOG2_E, about, size, tile)
                } else {
                    view.panned((dx, dy), size, tile)
                };
                cx.stop_propagation();
                by_wheel(next, window, cx);
            })
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                let (view, _) = held(cx);
                pressed.update(cx, |stage, _| stage.grip = Some((event.position, view)))
            })
            .on_drag(Pan { owner }, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_drag_move::<Pan>(move |event, window, cx| {
                if event.drag(cx).owner != owner {
                    return;
                }
                let stage = dragged.read(cx);
                let Some((press, start)) = stage.grip else {
                    return;
                };
                let size = sized(stage.bounds);
                let way = event.event.position - press;
                by_drag(
                    start.panned((f32::from(way.x), f32::from(way.y)), size, tile),
                    window,
                    cx,
                );
            })
            .on_drop(move |_: &Pan, _, cx| dropped.update(cx, |stage, _| stage.grip = None))
            .children(tiles)
            .children(layers)
            .children(pins.elements)
            .child(controls)
            .children(attribution)
            .children(popup)
            .child(measured)
    }
}
