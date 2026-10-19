use smallvec::SmallVec;

use crate::{
    AnyElement, App, Axis, Bounds, Corner, Display, Edges, Element, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Point, Position, Size, Style,
    Window, point, px,
};

/// The state that the anchored element element uses to track its children.
pub struct AnchoredState {
    child_layout_ids: SmallVec<[LayoutId; 4]>,
}

/// An anchored element that can be used to display UI that
/// will avoid overflowing the window bounds.
pub struct Anchored {
    children: SmallVec<[AnyElement; 2]>,
    anchor_corner: Corner,
    center_axis: Option<Axis>,
    fit_mode: AnchoredFitMode,
    anchor_position: Option<Point<Pixels>>,
    position_mode: AnchoredPositionMode,
    offset: Option<Point<Pixels>>,
    floating_target: Option<(Bounds<Pixels>, Pixels, Pixels, Option<Corner>)>,
}

/// anchored gives you an element that will avoid overflowing the window bounds.
/// Its children should have no margin to avoid measurement issues.
pub fn anchored() -> Anchored {
    Anchored {
        children: SmallVec::new(),
        anchor_corner: Corner::TopLeft,
        center_axis: None,
        fit_mode: AnchoredFitMode::SwitchAnchor,
        anchor_position: None,
        position_mode: AnchoredPositionMode::Window,
        offset: None,
        floating_target: None,
    }
}

impl Anchored {
    /// Sets which corner of the anchored element should be anchored to the current position.
    pub fn anchor(mut self, anchor: Corner) -> Self {
        self.anchor_corner = anchor;
        self
    }

    /// Center on the anchor point along one axis before fitting to the viewport.
    pub fn center_on(mut self, axis: Axis) -> Self {
        self.center_axis = Some(axis);
        self
    }

    /// Place beside a trigger, flip to the roomier side, and shift along the cross axis.
    pub fn floating_on(mut self, target: Bounds<Pixels>, gap: Pixels, margin: Pixels) -> Self {
        self.floating_target = Some((target, gap, margin, None));
        self.fit_mode = AnchoredFitMode::AllowOverflow;
        self
    }

    /// Align beside a trigger, flipping side/alignment when space is limited.
    pub fn floating_side_on(
        mut self,
        target: Bounds<Pixels>,
        gap: Pixels,
        margin: Pixels,
        anchor: Corner,
    ) -> Self {
        self.floating_target = Some((target, gap, margin, Some(anchor)));
        self.fit_mode = AnchoredFitMode::AllowOverflow;
        self
    }

    /// Sets the position in window coordinates
    /// (otherwise the location the anchored element is rendered is used)
    pub fn position(mut self, anchor: Point<Pixels>) -> Self {
        self.anchor_position = Some(anchor);
        self
    }

    /// Offset the final position by this amount.
    /// Useful when you want to anchor to an element but offset from it, such as in PopoverMenu.
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Sets the position mode for this anchored element. Local will have this
    /// interpret its [`Anchored::position`] as relative to the parent element.
    /// While Window will have it interpret the position as relative to the window.
    pub fn position_mode(mut self, mode: AnchoredPositionMode) -> Self {
        self.position_mode = mode;
        self
    }

    /// Snap to window edge instead of switching anchor corner when an overflow would occur.
    pub fn snap_to_window(mut self) -> Self {
        self.fit_mode = AnchoredFitMode::SnapToWindow;
        self
    }

    /// Preserve the requested position, including content beyond the viewport.
    pub fn allow_overflow(mut self) -> Self {
        self.fit_mode = AnchoredFitMode::AllowOverflow;
        self
    }

    /// Snap to window edge and leave some margins.
    pub fn snap_to_window_with_margin(mut self, edges: impl Into<Edges<Pixels>>) -> Self {
        self.fit_mode = AnchoredFitMode::SnapToWindowWithMargin(edges.into());
        self
    }
}

impl ParentElement for Anchored {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl Element for Anchored {
    type RequestLayoutState = AnchoredState;
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let child_layout_ids = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window, cx))
            .collect::<SmallVec<_>>();

        let anchored_style = Style {
            position: Position::Absolute,
            display: Display::Flex,
            ..Style::default()
        };

        let layout_id = window.request_layout(anchored_style, child_layout_ids.iter().copied(), cx);

        (layout_id, AnchoredState { child_layout_ids })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if request_layout.child_layout_ids.is_empty() {
            return;
        }

        let mut child_min = point(Pixels::MAX, Pixels::MAX);
        let mut child_max = Point::default();
        for child_layout_id in &request_layout.child_layout_ids {
            let child_bounds = window.layout_bounds(*child_layout_id);
            child_min = child_min.min(&child_bounds.origin);
            child_max = child_max.max(&child_bounds.bottom_right());
        }
        let size: Size<Pixels> = (child_max - child_min).into();

        let (origin, mut desired) = self.position_mode.get_position_and_bounds(
            self.anchor_position,
            self.anchor_corner,
            size,
            bounds,
            self.offset,
        );

        match self.center_axis {
            Some(Axis::Horizontal) => {
                desired.origin.x += match self.anchor_corner {
                    Corner::TopLeft | Corner::BottomLeft => -size.width / 2.,
                    _ => size.width / 2.,
                }
            }
            Some(Axis::Vertical) => {
                desired.origin.y += match self.anchor_corner {
                    Corner::TopLeft | Corner::TopRight => -size.height / 2.,
                    _ => size.height / 2.,
                }
            }
            None => {}
        }

        let limits = Bounds {
            origin: Point::default(),
            size: window.viewport_size(),
        };

        if let Some((target, gap, margin, side_alignment)) = self.floating_target {
            let horizontal = side_alignment.is_some() || self.center_axis == Some(Axis::Vertical);
            let prefer_before = if let Some(anchor) = side_alignment {
                matches!(anchor, Corner::TopRight | Corner::BottomRight)
            } else if horizontal {
                matches!(self.anchor_corner, Corner::TopRight | Corner::BottomRight)
            } else {
                matches!(self.anchor_corner, Corner::BottomLeft | Corner::BottomRight)
            };
            let (before, after, length) = if horizontal {
                (
                    target.left() - gap - margin,
                    limits.right() - target.right() - gap - margin,
                    size.width,
                )
            } else {
                (
                    target.top() - gap - margin,
                    limits.bottom() - target.bottom() - gap - margin,
                    size.height,
                )
            };
            let preferred_space = if prefer_before { before } else { after };
            let before_side = if preferred_space >= length {
                prefer_before
            } else {
                before >= after
            };
            desired.origin = if horizontal {
                point(
                    if before_side {
                        target.left() - gap - size.width
                    } else {
                        target.right() + gap
                    },
                    {
                        let lower = (limits.bottom() - size.height - margin).max(margin);
                        let position = if let Some(anchor) = side_alignment {
                            let align_end =
                                matches!(anchor, Corner::BottomLeft | Corner::BottomRight);
                            let start = target.top();
                            let end = target.bottom() - size.height;
                            let (preferred, alternate) = if align_end {
                                (end, start)
                            } else {
                                (start, end)
                            };
                            if !(margin..=lower).contains(&preferred)
                                && (margin..=lower).contains(&alternate)
                            {
                                alternate
                            } else {
                                preferred
                            }
                        } else {
                            target.center().y - size.height / 2.
                        };
                        position.clamp(margin, lower)
                    },
                )
            } else {
                point(
                    (target.center().x - size.width / 2.)
                        .clamp(margin, (limits.right() - size.width - margin).max(margin)),
                    if before_side {
                        target.top() - gap - size.height
                    } else {
                        target.bottom() + gap
                    },
                )
            };
        }

        if self.fit_mode == AnchoredFitMode::SwitchAnchor {
            let mut anchor_corner = self.anchor_corner;

            if self.center_axis != Some(Axis::Horizontal)
                && (desired.left() < limits.left() || desired.right() > limits.right())
            {
                let switched = Bounds::from_corner_and_size(
                    anchor_corner.other_side_corner_along(Axis::Horizontal),
                    origin,
                    size,
                );
                if !(switched.left() < limits.left() || switched.right() > limits.right()) {
                    anchor_corner = anchor_corner.other_side_corner_along(Axis::Horizontal);
                    desired = switched
                }
            }

            if self.center_axis != Some(Axis::Vertical)
                && (desired.top() < limits.top() || desired.bottom() > limits.bottom())
            {
                let switched = Bounds::from_corner_and_size(
                    anchor_corner.other_side_corner_along(Axis::Vertical),
                    origin,
                    size,
                );
                if !(switched.top() < limits.top() || switched.bottom() > limits.bottom()) {
                    desired = switched;
                }
            }
        }

        let client_inset = window.client_inset.unwrap_or(px(0.));
        let edges = match self.fit_mode {
            AnchoredFitMode::SnapToWindowWithMargin(edges) => edges,
            _ => Edges::default(),
        }
        .map(|edge| *edge + client_inset);

        // Snap the horizontal edges of the anchored element to the horizontal edges of the window if
        // its horizontal bounds overflow, aligning to the left if it is wider than the limits.
        if self.fit_mode != AnchoredFitMode::AllowOverflow && desired.right() > limits.right() {
            desired.origin.x -= desired.right() - limits.right() + edges.right;
        }
        if self.fit_mode != AnchoredFitMode::AllowOverflow && desired.left() < limits.left() {
            desired.origin.x = limits.origin.x + edges.left;
        }

        // Snap the vertical edges of the anchored element to the vertical edges of the window if
        // its vertical bounds overflow, aligning to the top if it is taller than the limits.
        if self.fit_mode != AnchoredFitMode::AllowOverflow && desired.bottom() > limits.bottom() {
            desired.origin.y -= desired.bottom() - limits.bottom() + edges.bottom;
        }
        if self.fit_mode != AnchoredFitMode::AllowOverflow && desired.top() < limits.top() {
            desired.origin.y = limits.origin.y + edges.top;
        }

        let offset = desired.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());

        window.with_element_offset(offset, |window| {
            for child in &mut self.children {
                child.prepaint(window, cx);
            }
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: crate::Bounds<crate::Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for child in &mut self.children {
            child.paint(window, cx);
        }
    }
}

impl IntoElement for Anchored {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Which algorithm to use when fitting the anchored element to be inside the window.
#[derive(Copy, Clone, PartialEq)]
pub enum AnchoredFitMode {
    /// Preserve the requested position without flipping or clamping.
    AllowOverflow,
    /// Snap the anchored element to the window edge.
    SnapToWindow,
    /// Snap to window edge and leave some margins.
    SnapToWindowWithMargin(Edges<Pixels>),
    /// Switch which corner anchor this anchored element is attached to.
    SwitchAnchor,
}

/// Which algorithm to use when positioning the anchored element.
#[derive(Copy, Clone, PartialEq)]
pub enum AnchoredPositionMode {
    /// Position the anchored element relative to the window.
    Window,
    /// Position the anchored element relative to its parent.
    Local,
}

impl AnchoredPositionMode {
    fn get_position_and_bounds(
        &self,
        anchor_position: Option<Point<Pixels>>,
        anchor_corner: Corner,
        size: Size<Pixels>,
        bounds: Bounds<Pixels>,
        offset: Option<Point<Pixels>>,
    ) -> (Point<Pixels>, Bounds<Pixels>) {
        let offset = offset.unwrap_or_default();

        match self {
            AnchoredPositionMode::Window => {
                let anchor_position = anchor_position.unwrap_or(bounds.origin);
                let bounds =
                    Bounds::from_corner_and_size(anchor_corner, anchor_position + offset, size);
                (anchor_position, bounds)
            }
            AnchoredPositionMode::Local => {
                let anchor_position = anchor_position.unwrap_or_default();
                let bounds = Bounds::from_corner_and_size(
                    anchor_corner,
                    bounds.origin + anchor_position + offset,
                    size,
                );
                (anchor_position, bounds)
            }
        }
    }
}
