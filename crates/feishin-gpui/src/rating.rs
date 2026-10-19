use crate::app::Feishin;
use feishin_core::model::Item;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt, animation::CssTransition};
use std::time::Duration;

#[derive(IntoElement)]
pub struct Rating {
    id: SharedString,
    item: Item,
    app: WeakEntity<Feishin>,
    label: String,
    read_only: bool,
    size: Option<Pixels>,
    horizontal_padding: Option<Pixels>,
}
impl Feishin {
    pub fn rating(&self, id: impl Into<SharedString>, item: &Item, cx: &Context<Self>) -> Rating {
        Rating {
            id: id.into(),
            item: item.clone(),
            app: cx.weak_entity(),
            label: self.tr("Change rating"),
            read_only: false,
            size: None,
            horizontal_padding: None,
        }
    }
}
impl Rating {
    pub fn horizontal_padding(mut self, padding: Pixels) -> Self {
        self.horizontal_padding = Some(padding);
        self
    }
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = Some(size);
        self
    }
    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }
}
impl RenderOnce for Rating {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Include the item identity so recycled table rows cannot retain another song's edit.
        let key: SharedString = format!(
            "{}-{}-{:?}-{}",
            self.id, self.item.server_id, self.item.kind, self.item.id
        )
        .into();
        let state =
            window.use_keyed_state(key, cx, |window, cx| RatingState::new(&self, window, cx));
        state.update(cx, |state, cx| {
            if state.item.rating != self.item.rating
                || state.label != self.label
                || state.read_only != self.read_only
                || state.size != self.size
                || state.horizontal_padding != self.horizontal_padding
            {
                cx.notify();
            }
            state.item = self.item;
            state.label = self.label;
            state.read_only = self.read_only;
            state.size = self.size;
            state.horizontal_padding = self.horizontal_padding;
        });
        state
    }
}
struct RatingState {
    id: SharedString,
    item: Item,
    app: WeakEntity<Feishin>,
    label: String,
    read_only: bool,
    size: Option<Pixels>,
    horizontal_padding: Option<Pixels>,
    hovered: Option<u8>,
    pointer_inside: bool,
    group_focus: FocusHandle,
    scale: [CssTransition; 5],
    _blur: Subscription,
    focus: [FocusHandle; 5],
    pending: Option<Task<()>>,
}
impl RatingState {
    fn new(props: &Rating, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let group_focus = cx.focus_handle();
        let blur = cx.on_focus_out(&group_focus, window, |this: &mut RatingState, _, _, cx| {
            if !this.pointer_inside {
                this.hovered = None;
                cx.notify();
            }
        });
        Self {
            id: props.id.clone(),
            item: props.item.clone(),
            app: props.app.clone(),
            label: props.label.clone(),
            read_only: props.read_only,
            size: props.size,
            horizontal_padding: props.horizontal_padding,
            hovered: None,
            pointer_inside: false,
            group_focus,
            focus: std::array::from_fn(|_| cx.focus_handle()),
            scale: std::array::from_fn(|_| {
                CssTransition::new(1., Duration::from_millis(100), [0.25, 0.1, 0.25, 1.])
            }),
            _blur: blur,
            pending: None,
        }
    }
    fn select(&mut self, value: u8, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let rating = if value == self.item.rating { 0 } else { value };
        let item = self.item.clone();
        let app = self.app.clone();
        self.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            this.update(cx, |this, cx| {
                if !this.read_only {
                    app.update(cx, |app, cx| {
                        app.set_library_values(vec![item], None, Some(rating), cx)
                    })
                    .ok();
                }
            })
            .ok();
        }));
    }
}
impl Render for RatingState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rtl = crate::appearance::rtl(cx);
        let id = self.id.clone();
        let size = self.size.unwrap_or(cx.theme().font_size * 0.75);
        div()
            .id("rating")
            .debug_selector(move || id.to_string())
            .key_context("FormControl")
            .track_focus(&self.group_focus)
            .tab_index(-1)
            .role(Role::RadioGroup)
            .aria_label(self.label.clone())
            .h_flex()
            .flex_shrink_0()
            .px_2p5()
            .when(self.size.is_some(), |rating| {
                rating.px(cx.theme().font_size * 0.75)
            })
            .when_some(self.horizontal_padding, |div, padding| div.px(padding))
            .when(rtl, |div| div.flex_row_reverse())
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if !this.read_only {
                    this.pointer_inside = *hovered;
                    if !hovered {
                        this.hovered = None;
                    }
                    cx.notify();
                }
            }))
            .children((1..=5u8).map(|value| {
                let id = format!("{}-{value}", self.id);
                let selected = self.item.rating == value;
                let filled = value <= self.hovered.unwrap_or(self.item.rating);
                let now = cx.background_executor().now();
                let scale = &mut self.scale[value as usize - 1];
                scale.retarget(
                    if !self.read_only && self.hovered == Some(value) {
                        1.1
                    } else {
                        1.
                    },
                    now,
                );
                let (scale, animating) = scale.value(now);
                if animating {
                    cx.on_next_frame(window, |_, _, cx| cx.notify());
                }
                div()
                    .id(value as usize)
                    .debug_selector(move || id.clone())
                    .role(Role::RadioButton)
                    .aria_label(format!("{} {value}/5", self.label))
                    .aria_disabled(self.read_only)
                    .aria_toggled(if selected {
                        accesskit::Toggled::True
                    } else {
                        accesskit::Toggled::False
                    })
                    .track_focus(&self.focus[value as usize - 1])
                    .tab_index(
                        if !self.read_only && (selected || (self.item.rating == 0 && value == 1)) {
                            0
                        } else {
                            -1
                        },
                    )
                    .size(size)
                    .paint_scale(scale)
                    .relative()
                    .when(!self.read_only, |div| div.cursor_pointer())
                    .focus(|style| style.border_1().border_color(cx.theme().ring))
                    .when(filled, |div| {
                        div.child(
                            svg()
                                .absolute()
                                .size_full()
                                .path("native-icons/rating-fill.svg")
                                .text_color(rgb(0xfab005)),
                        )
                    })
                    .child(
                        svg()
                            .absolute()
                            .size_full()
                            .path("native-icons/rating-outline.svg")
                            .text_color(cx.theme().foreground),
                    )
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        if *hovered && !this.read_only {
                            this.hovered = Some(value);
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !this.read_only {
                            this.focus[value as usize - 1].focus(window);
                            this.select(value, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if this.read_only {
                            return;
                        }
                        let delta = match event.keystroke.key.as_str() {
                            "right" => Some(if rtl { -1 } else { 1 }),
                            "left" => Some(if rtl { 1 } else { -1 }),
                            "up" => Some(-1),
                            "down" => Some(1),
                            _ => None,
                        };
                        if let Some(delta) = delta {
                            let next = (i32::from(value) - 1 + delta).rem_euclid(5) as u8 + 1;
                            this.focus[next as usize - 1].focus(window);
                            this.hovered = Some(next);
                            cx.notify();
                        } else if matches!(event.keystroke.key.as_str(), "space" | "enter") {
                            this.select(value, cx);
                        } else {
                            return;
                        }
                        cx.stop_propagation();
                    }))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{Feishin, Item, Rating, RatingState};
    use feishin_core::storage::State;
    use gpui::{
        AppContext, Context, Entity, FocusHandle, Modifiers, TestAppContext, Window, div, point,
        prelude::*, px,
    };
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc, time::Duration};
    struct Host {
        _app: Entity<Feishin>,
        rating: Entity<RatingState>,
        outside: FocusHandle,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p_4().child(self.rating.clone()).child(
                div()
                    .track_focus(&self.outside)
                    .size(px(30.))
                    .child("Outside"),
            )
        }
    }
    #[gpui::test]
    fn hover_transition_read_only_and_keyboard_blur_match_rating(cx: &mut TestAppContext) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-rating-hover-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let item = Item {
            id: "song".into(),
            rating: 2,
            ..Default::default()
        };
        let (_, cx) = cx.add_window_view(|window, cx| {
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
            let rating = cx.new(|cx| {
                RatingState::new(
                    &Rating {
                        id: "test-rating".into(),
                        item,
                        app: app.downgrade(),
                        label: "Rating".into(),
                        read_only: false,
                        size: None,
                        horizontal_padding: None,
                    },
                    window,
                    cx,
                )
            });
            let host = cx.new(|cx| Host {
                _app: app,
                rating,
                outside: cx.focus_handle(),
            });
            captured.replace(Some(host.clone()));
            Root::new(host, window, cx)
        });
        let host = captured.borrow_mut().take().unwrap();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let rating = host.read_with(cx, |host, _| host.rating.clone());
        let star = cx.debug_bounds("test-rating-4").unwrap().center();
        cx.simulate_mouse_move(star, None, Modifiers::none());
        cx.run_until_parked();
        rating.read_with(cx, |state, _| {
            assert_eq!(state.hovered, Some(4));
            assert!(state.pointer_inside);
        });
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        rating.read_with(cx, |state, cx| {
            let (scale, animating) = state.scale[3].value(cx.background_executor().now());
            assert!((1.07..1.09).contains(&scale), "{scale}");
            assert!(animating);
        });
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        rating.read_with(cx, |state, cx| {
            assert_eq!(
                state.scale[3].value(cx.background_executor().now()),
                (1.1, false)
            )
        });
        cx.simulate_mouse_move(point(px(500.), px(500.)), None, Modifiers::none());
        cx.run_until_parked();
        assert!(!rating.read_with(cx, |state, _| state.pointer_inside));
        cx.update(|window, cx| rating.read(cx).focus[1].focus(window));
        cx.simulate_keystrokes("right");
        cx.run_until_parked();
        assert_eq!(rating.read_with(cx, |state, _| state.hovered), Some(3));
        cx.update(|window, cx| host.read(cx).outside.focus(window));
        cx.run_until_parked();
        assert!(cx.update(|window, cx| host.read(cx).outside.is_focused(window)));
        assert_eq!(rating.read_with(cx, |state, _| state.hovered), None);
        cx.update(|_, cx| {
            rating.update(cx, |state, cx| {
                state.read_only = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.simulate_mouse_move(star, None, Modifiers::none());
        cx.run_until_parked();
        rating.read_with(cx, |state, _| {
            assert_eq!(state.hovered, None);
            assert!(!state.pointer_inside);
            assert!(state.pending.is_none());
        });
        cx.update(|window, cx| rating.read(cx).focus[1].focus(window));
        cx.simulate_keystrokes("right enter");
        cx.run_until_parked();
        rating.read_with(cx, |state, _| {
            assert_eq!(state.hovered, None);
            assert!(state.pending.is_none());
        });
        if dir.exists() {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
