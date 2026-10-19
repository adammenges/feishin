use feishin_core::appearance;
use gpui::{
    App, Axis, Background, Bounds, Corners, DispatchPhase, Element, ElementId, Global,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, RenderOnce, ScrollHandle, StyleRefinement, Styled, Window,
    canvas, div, fill, point, prelude::*, px, size,
};
use gpui_component::{ActiveTheme, StyledExt, scroll::ScrollbarHandle};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub struct ScrollTheme {
    size: Pixels,
    thumb_radius: Pixels,
    track_radius: Pixels,
    thumb: [Background; 3],
    track: [Background; 3],
}
impl Global for ScrollTheme {}
impl ScrollTheme {
    pub fn new(palette: &appearance::Palette, font: f32, zoom: f32, cx: &App) -> Self {
        let color = |key: &str, fallback: Background| {
            palette
                .app
                .get(key)
                .and_then(|value| appearance::theme_color(value, palette).ok())
                .map(|[r, g, b, a]| gpui::Rgba { r, g, b, a }.into())
                .unwrap_or(fallback)
        };
        let length = |key: &str, fallback: f32| {
            palette
                .app
                .get(key)
                .and_then(|value| {
                    let (value, factor) = if let Some(value) = value.strip_suffix("rem") {
                        (value, font)
                    } else {
                        (value.trim_end_matches("px"), zoom)
                    };
                    value.parse::<f32>().ok().map(|n| n * factor)
                })
                .filter(|n| n.is_finite() && (0. ..=100.).contains(n))
                .unwrap_or(fallback)
        };
        let thumb = color(
            "scrollbar-handle-background",
            cx.theme().scrollbar_thumb.into(),
        );
        let thumb_hover = color(
            "scrollbar-handle-hover-background",
            cx.theme().scrollbar_thumb_hover.into(),
        );
        let track = color("scrollbar-track-background", cx.theme().scrollbar.into());
        let track_hover = color("scrollbar-track-hover-background", track);
        Self {
            size: px(length("scrollbar-size", 8. * zoom).clamp(0., 64.)),
            thumb_radius: px(length("scrollbar-handle-border-radius", 4. * zoom)),
            track_radius: px(length("scrollbar-track-border-radius", 4. * zoom)),
            thumb: [
                thumb,
                thumb_hover,
                color("scrollbar-handle-active-background", thumb_hover),
            ],
            track: [
                track,
                track_hover,
                color("scrollbar-track-active-background", track_hover),
            ],
        }
    }
}
pub trait Handle: ScrollbarHandle {
    fn viewport(&self) -> Bounds<Pixels>;
}
impl Handle for ScrollHandle {
    fn viewport(&self) -> Bounds<Pixels> {
        self.bounds()
    }
}
impl Handle for gpui::UniformListScrollHandle {
    fn viewport(&self) -> Bounds<Pixels> {
        self.0.borrow().base_handle.bounds()
    }
}
impl Handle for gpui::ListState {
    fn viewport(&self) -> Bounds<Pixels> {
        self.viewport_bounds()
    }
}

#[derive(Default)]
struct State {
    hovered: bool,
    bounds: Option<Bounds<Pixels>>,
    drag: Option<(Pixels, Pixels)>,
}
#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    handle: Rc<dyn Handle>,
    parent_viewport: Option<Rc<dyn Handle>>,
    axis: Axis,
}
impl Scrollbar {
    pub fn vertical<H: Handle + Clone>(id: impl Into<ElementId>, handle: &H) -> Self {
        Self {
            id: id.into(),
            handle: Rc::new(handle.clone()),
            parent_viewport: None,
            axis: Axis::Vertical,
        }
    }
    pub fn horizontal<H: Handle + Clone>(id: impl Into<ElementId>, handle: &H) -> Self {
        Self {
            id: id.into(),
            handle: Rc::new(handle.clone()),
            parent_viewport: None,
            axis: Axis::Horizontal,
        }
    }
    pub fn in_viewport<H: Handle + Clone>(mut self, handle: &H) -> Self {
        self.parent_viewport = Some(Rc::new(handle.clone()));
        self
    }
}
fn coordinate(point: Point<Pixels>, axis: Axis) -> Pixels {
    match axis {
        Axis::Vertical => point.y,
        Axis::Horizontal => point.x,
    }
}
fn extent(size: gpui::Size<Pixels>, axis: Axis) -> Pixels {
    match axis {
        Axis::Vertical => size.height,
        Axis::Horizontal => size.width,
    }
}
fn set_offset(handle: &dyn Handle, axis: Axis, offset: Pixels) {
    let mut position = handle.offset();
    match axis {
        Axis::Vertical => position.y = offset,
        Axis::Horizontal => position.x = offset,
    }
    handle.set_offset(position);
}
fn thumb_geometry(length: f32, content: f32, offset: f32) -> Option<(f32, f32, f32)> {
    if !length.is_finite() || !content.is_finite() || length <= 0. || content <= length + 0.5 {
        return None;
    }
    let thumb = (length * length / content).clamp(16_f32.min(length), length);
    let travel = length - thumb;
    let max = content - length;
    Some(((-offset / max).clamp(0., 1.) * travel, thumb, max))
}
impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window
            .use_keyed_state(self.id.clone(), cx, |_, _| {
                Rc::new(RefCell::new(State::default()))
            })
            .read(cx)
            .clone();
        let theme = cx
            .try_global::<ScrollTheme>()
            .cloned()
            .unwrap_or_else(|| ScrollTheme::new(&appearance::Palette::default(), 16., 1., cx));
        let axis = self.axis;
        let name = self.id.to_string();
        let bounds_state = state.clone();
        let paint_state = state.clone();
        let paint_handle = self.handle.clone();
        let layout_handle = self.handle.clone();
        let parent_viewport = self.parent_viewport;
        div()
            .id(self.id)
            .debug_selector(move || name.clone())
            .absolute()
            .when(axis == Axis::Vertical, |div| {
                div.right_0().top_0().bottom_0().w(theme.size)
            })
            .when(axis == Axis::Horizontal, |div| {
                div.left_0().right_0().bottom_0().h(theme.size)
            })
            .child(
                canvas(
                    move |_, window, _| {
                        let viewport = layout_handle.viewport();
                        let bounds = match axis {
                            Axis::Vertical => Bounds::new(
                                point(viewport.right() - theme.size, viewport.top()),
                                size(theme.size, viewport.size.height),
                            ),
                            Axis::Horizontal => Bounds::new(
                                point(
                                    viewport.left(),
                                    parent_viewport
                                        .as_ref()
                                        .map_or(viewport.bottom(), |parent| {
                                            parent.viewport().bottom()
                                        })
                                        - theme.size,
                                ),
                                size(viewport.size.width, theme.size),
                            ),
                        };
                        bounds_state.borrow_mut().bounds = Some(bounds);
                        window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal)
                    },
                    move |_, hitbox, window, _| {
                        let bounds = hitbox.bounds;
                        let hitbox = hitbox.id;
                        let length = f32::from(extent(bounds.size, axis));
                        let content = f32::from(extent(paint_handle.content_size(), axis));
                        if let Some((start, thumb, _)) = thumb_geometry(
                            length,
                            content,
                            f32::from(coordinate(paint_handle.offset(), axis)),
                        ) {
                            let interaction = if paint_state.borrow().drag.is_some() {
                                2
                            } else if hitbox.is_hovered(window) {
                                1
                            } else {
                                0
                            };
                            let mut track = fill(bounds, theme.track[interaction]);
                            track.corner_radii = Corners::all(theme.track_radius);
                            window.paint_quad(track);
                            let thumb_bounds = match axis {
                                Axis::Vertical => Bounds::new(
                                    point(bounds.origin.x, bounds.origin.y + px(start)),
                                    size(bounds.size.width, px(thumb)),
                                ),
                                Axis::Horizontal => Bounds::new(
                                    point(bounds.origin.x + px(start), bounds.origin.y),
                                    size(px(thumb), bounds.size.height),
                                ),
                            };
                            let mut thumb = fill(thumb_bounds, theme.thumb[interaction]);
                            thumb.corner_radii = Corners::all(theme.thumb_radius);
                            window.paint_quad(thumb);
                        }
                        let down = paint_state.clone();
                        let down_handle = paint_handle.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture
                                || event.button != MouseButton::Left
                                || !hitbox.is_hovered(window)
                            {
                                return;
                            }
                            let Some(bounds) = down.borrow().bounds else {
                                return;
                            };
                            if !bounds.contains(&event.position) {
                                return;
                            }
                            let length = f32::from(extent(bounds.size, axis));
                            let content = f32::from(extent(down_handle.content_size(), axis));
                            let offset = coordinate(down_handle.offset(), axis);
                            let Some((start, thumb, max)) =
                                thumb_geometry(length, content, f32::from(offset))
                            else {
                                return;
                            };
                            let position =
                                f32::from(coordinate(event.position - bounds.origin, axis));
                            down_handle.start_drag();
                            if position < start || position > start + thumb {
                                let fraction = ((position - thumb / 2.) / (length - thumb).max(1.))
                                    .clamp(0., 1.);
                                set_offset(down_handle.as_ref(), axis, px(-fraction * max));
                            }
                            down.borrow_mut().drag = Some((
                                coordinate(event.position, axis),
                                coordinate(down_handle.offset(), axis),
                            ));
                            cx.stop_propagation();
                            cx.refresh_windows();
                        });
                        let moved = paint_state.clone();
                        let moved_handle = paint_handle.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let hovered = hitbox.is_hovered(window);
                            if moved.borrow().hovered != hovered {
                                moved.borrow_mut().hovered = hovered;
                                cx.refresh_windows();
                            }
                            let Some((origin, offset)) = moved.borrow().drag else {
                                return;
                            };
                            let Some(bounds) = moved.borrow().bounds else {
                                return;
                            };
                            if event.pressed_button != Some(MouseButton::Left) {
                                return;
                            }
                            let length = f32::from(extent(bounds.size, axis));
                            let content = f32::from(extent(moved_handle.content_size(), axis));
                            if let Some((_, thumb, max)) =
                                thumb_geometry(length, content, f32::from(offset))
                            {
                                let delta = f32::from(coordinate(event.position, axis) - origin);
                                set_offset(
                                    moved_handle.as_ref(),
                                    axis,
                                    px((f32::from(offset)
                                        - delta / (length - thumb).max(1.) * max)
                                        .clamp(-max, 0.)),
                                );
                                cx.stop_propagation();
                                cx.refresh_windows();
                            }
                        });
                        let released = paint_state.clone();
                        let released_handle = paint_handle.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture
                                && event.button == MouseButton::Left
                                && released.borrow_mut().drag.take().is_some()
                            {
                                released_handle.end_drag();
                                cx.refresh_windows();
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}
/// Scrollbars for ordinary panels reuse the framework's persistent scroll state.
#[derive(IntoElement)]
pub struct Vertical<E: InteractiveElement + Styled + ParentElement + Element> {
    id: ElementId,
    element: E,
    handle: Option<ScrollHandle>,
}
impl<E: InteractiveElement + Styled + ParentElement + Element> Vertical<E> {
    pub fn scroll_handle(mut self, handle: &ScrollHandle) -> Self {
        self.handle = Some(handle.clone());
        self
    }
}
pub trait VerticalScroll: InteractiveElement + Styled + ParentElement + Element + Sized {
    #[track_caller]
    fn native_vertical_scroll(self) -> Vertical<Self> {
        Vertical {
            id: ElementId::CodeLocation(*std::panic::Location::caller()),
            element: self,
            handle: None,
        }
    }
}
impl VerticalScroll for gpui::Div {}
impl VerticalScroll for gpui::Stateful<gpui::Div> {}
impl<E: InteractiveElement + Styled + ParentElement + Element> Styled for Vertical<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}
impl<E: InteractiveElement + Styled + ParentElement + Element> ParentElement for Vertical<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.element.extend(elements);
    }
}
impl<E: InteractiveElement + Styled + ParentElement + Element> InteractiveElement for Vertical<E> {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.element.interactivity()
    }
}
impl<E: InteractiveElement + Styled + ParentElement + Element + 'static> RenderOnce
    for Vertical<E>
{
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = self.handle.unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, _| ScrollHandle::default())
                .read(cx)
                .clone()
        });
        let style = std::mem::take(self.element.style());
        let mut outer = div();
        *outer.interactivity() = std::mem::take(self.element.interactivity());
        if outer.interactivity().element_id.is_none() {
            outer.interactivity().element_id = Some(self.id);
        }
        *self.element.style() = StyleRefinement {
            display: style.display,
            flex_direction: style.flex_direction,
            flex_wrap: style.flex_wrap,
            align_items: style.align_items,
            justify_content: style.justify_content,
            gap: style.gap.clone(),
            ..Default::default()
        };
        outer
            .refine_style(&style)
            .relative()
            .child(
                div()
                    .id("native-scroll-area")
                    .v_flex()
                    .size_full()
                    .track_scroll(&handle)
                    .overflow_y_scroll()
                    .child(self.element.flex_shrink_0().w_full().min_h_full()),
            )
            .child(Scrollbar::vertical("native-scrollbar", &handle))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext};

    struct Fixture(ScrollHandle, bool);
    impl Render for Fixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .relative()
                .size(px(200.))
                .p(px(10.))
                .child(
                    div()
                        .id("scroll-fixture")
                        .size_full()
                        .overflow_scroll()
                        .track_scroll(&self.0)
                        .child(div().size(px(2000.)).flex_shrink_0()),
                )
                .child(Scrollbar::vertical("fixture-vertical", &self.0))
                .child(Scrollbar::horizontal("fixture-horizontal", &self.0))
                .when(self.1, |div| {
                    div.child(gpui::div().absolute().inset_0().occlude())
                })
        }
    }
    #[gpui::test]
    fn covered_native_scrollbars_leave_overlay_clicks_and_scroll_position_untouched(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_component::init);
        let handle = ScrollHandle::default();
        let (view, cx) = cx.add_window_view(|_, _| Fixture(handle.clone(), true));
        cx.run_until_parked();
        let viewport = handle.bounds();
        let point = point(viewport.right() - px(4.), viewport.center().y);
        cx.simulate_click(point, Modifiers::none());
        assert_eq!(handle.offset(), gpui::point(px(0.), px(0.)));
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.1 = false;
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.simulate_click(point, Modifiers::none());
        assert!(handle.offset().y < px(-800.));
    }

    #[gpui::test]
    fn native_scrollbars_click_drag_outside_and_preserve_the_other_axis(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let handle = ScrollHandle::default();
        let (_, cx) = cx.add_window_view(|_, _| Fixture(handle.clone(), false));
        cx.run_until_parked();
        let viewport = handle.bounds();
        assert_eq!(viewport.size, size(px(180.), px(180.)));
        let vertical = point(viewport.right() - px(4.), viewport.center().y);
        cx.simulate_mouse_down(vertical, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(vertical, MouseButton::Left, Modifiers::none());
        assert!(handle.offset().y < px(-800.));
        assert_eq!(handle.offset().x, px(0.));
        let y = handle.offset().y;
        let horizontal = point(viewport.center().x, viewport.bottom() - px(4.));
        cx.simulate_mouse_down(horizontal, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            horizontal + point(px(1000.), px(1000.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            horizontal + point(px(1000.), px(1000.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert_eq!(handle.offset().x, px(-1820.));
        assert_eq!(handle.offset().y, y);
        // Releasing outside the track ends the drag; later moves cannot scroll it.
        cx.simulate_mouse_move(horizontal, MouseButton::Left, Modifiers::none());
        assert_eq!(handle.offset().x, px(-1820.));
        let vertical = point(viewport.right() - px(4.), viewport.top() + px(5.));
        cx.simulate_mouse_down(vertical, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            vertical - point(px(1000.), px(1000.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            vertical - point(px(1000.), px(1000.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert_eq!(handle.offset(), point(px(-1820.), px(0.)));
    }
    #[test]
    fn thumbs_keep_extent_offsets_clamping_and_empty_scroll_areas() {
        assert_eq!(thumb_geometry(100., 400., -150.), Some((37.5, 25., 300.)));
        assert_eq!(thumb_geometry(100., 400., -300.), Some((75., 25., 300.)));
        assert_eq!(thumb_geometry(100., 400., 10.), Some((0., 25., 300.)));
        assert!(thumb_geometry(100., 50., 0.).is_none());
        assert!(thumb_geometry(0., 400., 0.).is_none());
        assert!(thumb_geometry(100., f32::NAN, 0.).is_none());
    }
}
