use crate::scroll::VerticalScroll;
use crate::{
    app::{Feishin, Route},
    queue::Dragged,
};
use feishin_core::{
    home::Kind,
    model::{Item, Page},
};
use gpui::{prelude::*, *};
use gpui_component::animation::CssTransition;
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
};
use rand::seq::SliceRandom;
use std::collections::HashSet;

fn hover_transition(
    id: impl Into<ElementId>,
    milliseconds: u64,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<CssTransition>, f32) {
    let state = window.use_keyed_state(id, cx, |_, _| {
        CssTransition::new(
            0.,
            std::time::Duration::from_millis(milliseconds),
            [0.25, 0.1, 0.25, 1.],
        )
    });
    let (value, animating) = state.read(cx).value(cx.background_executor().now());
    if animating {
        window.request_animation_frame();
    }
    (state, value)
}
fn transition_hover(state: Entity<CssTransition>) -> impl Fn(&bool, &mut Window, &mut App) {
    move |hovered, _, cx| {
        state.update(cx, |state, cx| {
            state.retarget(f32::from(*hovered), cx.background_executor().now());
            cx.notify();
        })
    }
}
fn feature_shadows(progress: f32, single: bool, zoom: f32) -> Vec<gpui::BackdropFilterOperation> {
    let progress = if single { 0. } else { progress };
    [(6., 20., 0.5, 16., 40., 0.6), (2., 8., 0.4, 6., 16., 0.5)]
        .into_iter()
        .map(|(y, sigma, alpha, end_y, end_sigma, end_alpha)| {
            gpui::BackdropFilterOperation::DropShadow {
                offset: [0., (y + (end_y - y) * progress) * zoom],
                sigma: (sigma + (end_sigma - sigma) * progress) * zoom,
                color: Some([0., 0., 0., alpha + (end_alpha - alpha) * progress]),
            }
        })
        .collect()
}

fn feature_text_shadow(zoom: f32) -> Vec<gpui::TextShadow> {
    vec![gpui::TextShadow {
        offset: gpui::Point::default(),
        blur_radius: px(8. * zoom),
        color: Some(gpui::black().opacity(0.5)),
    }]
}

fn feature_target(tag: &str, class: &str) -> feishin_core::styles::Target {
    let mut target = feishin_core::styles::Target::new(
        tag,
        "",
        &[&format!("fs-feature-carousel-module-{class}")],
    );
    if class == "blurred-background" {
        target
            .inline_declarations
            .push(("filter".into(), "blur(3rem)".into()));
    }
    target
}

pub(crate) fn feature_stylesheet() -> &'static str {
    ".fs-feature-carousel-module-content :where(.mantine-Text-root) { font-size:var(--text-fz); line-height:var(--text-lh); }
     .fs-feature-carousel-module-content :where(.mantine-Title-root) { font-size:var(--title-fz); line-height:var(--title-lh); }
     .fs-feature-carousel-module-content :where(.mantine-Badge-root) { font-size:.625rem; line-height:1rem; letter-spacing:.015625rem; font-weight:500; color:var(--theme-colors-foreground); }
     .fs-badge-module-root[data-variant=transparent] { background:transparent; }
     .fs-feature-carousel-module-title,.fs-feature-carousel-module-artist,.fs-feature-carousel-module-badge { color:white; text-shadow:0 0 8px rgb(0 0 0 / 50%); }"
}

fn feature_text_target(
    tag: &str,
    class: &str,
    size: &str,
    weight: u16,
    line_height: &str,
) -> feishin_core::styles::Target {
    let mut target = feature_target(tag, class);
    let heading = tag.starts_with('h');
    target.classes.extend(
        if heading {
            ["mantine-Title-root", "fs-text-title-module-root"]
        } else {
            ["mantine-Text-root", "fs-text-module-root"]
        }
        .map(str::to_owned),
    );
    target
        .inline_declarations
        .push(("font-weight".into(), weight.to_string()));
    target.inline_declarations.push((
        if heading { "--title-fz" } else { "--text-fz" }.into(),
        size.into(),
    ));
    target.inline_declarations.push((
        if heading { "line-height" } else { "--text-lh" }.into(),
        line_height.into(),
    ));
    if heading {
        target
            .inline_declarations
            .push(("text-align".into(), "left".into()));
    } else if class == "artist" || class == "title" && weight == 700 {
        target
            .inline_declarations
            .push(("text-align".into(), "center".into()));
    }
    if tag == "h1" {
        target
            .inline_declarations
            .push(("font-size".into(), size.into()));
    }
    target
}

fn feature_path(single: bool) -> Vec<feishin_core::styles::Target> {
    let mut container = feature_target("div", "carousel-container");
    if single {
        container
            .classes
            .push("fs-feature-carousel-module-single-carousel-container".into());
    }
    vec![
        container,
        feature_target("div", "carousel"),
        feature_target("div", "carousel-item"),
        feature_target("a", "carousel-link"),
        feature_target("div", "content"),
    ]
}

fn feature_style<T: Styled>(
    element: T,
    parents: &[feishin_core::styles::Target],
    section: Option<feishin_core::styles::Target>,
    descendants: &[feishin_core::styles::Target],
    cx: &App,
) -> T {
    let mut path = parents.to_vec();
    if let Some(section) = section {
        path.push(section);
    }
    path.extend_from_slice(descendants);
    let target = path.pop().unwrap();
    crate::styles::apply_in(element, target, path, cx)
}

struct FeatureFade {
    from: f32,
    target: f32,
    started: Option<std::time::Instant>,
    duration: std::time::Duration,
    curve: [f32; 4],
    transition: CssTransition,
}
impl FeatureFade {
    fn settled(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            started: None,
            duration: std::time::Duration::ZERO,
            curve: [0., 0., 1., 1.],
            transition: CssTransition::new(value, std::time::Duration::ZERO, [0., 0., 1., 1.]),
        }
    }
    fn restart(&mut self, target: f32, now: std::time::Instant, reset: bool) {
        let from = if reset {
            0.
        } else {
            self.interrupted_value(now)
        };
        let entering = target == 1.;
        let duration = std::time::Duration::from_millis(if entering { 200 } else { 300 });
        let curve = if entering {
            [0., 0., 0.58, 1.]
        } else {
            [0.42, 0., 1., 1.]
        };
        let mut transition = CssTransition::new(from, duration, curve);
        transition.retarget(target, now);
        *self = Self {
            from,
            target,
            started: Some(now),
            duration,
            curve,
            transition,
        };
    }
    fn completed(&self, now: std::time::Instant) -> bool {
        self.started
            .is_none_or(|start| now.saturating_duration_since(start) >= self.duration)
    }
    fn removed(&self, now: std::time::Instant) -> bool {
        // Motion's finish callback removes exited pages after their final frame.
        self.started
            .is_none_or(|start| now.saturating_duration_since(start) > self.duration)
    }
    fn interrupted_value(&self, now: std::time::Instant) -> f32 {
        let Some(started) = self.started else {
            return self.target;
        };
        if self.completed(now) {
            return self.target;
        }
        // Motion 12.40 samples its JS animation when interrupting WAAPI. Its
        // cubic-bezier solver uses twelve binary subdivisions, including the
        // ten-millisecond minimum sampling interval in NativeAnimationExtended.
        let elapsed = now
            .saturating_duration_since(started)
            .as_secs_f64()
            .max(0.01);
        let progress = (elapsed / self.duration.as_secs_f64()).clamp(0., 1.);
        let [x1, y1, x2, y2] = self.curve.map(f64::from);
        let bezier = |t: f64, a: f64, b: f64| {
            (((1. - 3. * b + 3. * a) * t + (3. * b - 6. * a)) * t + 3. * a) * t
        };
        let mut lower = 0.;
        let mut upper = 1.;
        let mut t = progress;
        if progress > 0. && progress < 1. {
            for _ in 0..12 {
                t = (lower + upper) / 2.;
                let difference = bezier(t, x1, x2) - progress;
                if difference > 0. {
                    upper = t;
                } else {
                    lower = t;
                }
                if difference.abs() <= 0.0000001 {
                    break;
                }
            }
        }
        self.from + (self.target - self.from) * bezier(t, y1, y2) as f32
    }
}
struct FeaturePage {
    key: usize,
    cards: Vec<(usize, Item)>,
    single: bool,
    fade: FeatureFade,
}
#[derive(Default)]
struct FeaturePages {
    pages: Vec<FeaturePage>,
    current: usize,
}
impl FeaturePages {
    fn sync(
        &mut self,
        key: usize,
        cards: Vec<(usize, Item)>,
        single: bool,
        now: std::time::Instant,
    ) {
        if self.pages.is_empty() {
            self.current = key;
            self.pages.push(FeaturePage {
                key,
                cards,
                single,
                fade: FeatureFade::settled(1.),
            });
            return;
        }
        self.prune(now);
        if self.current == key {
            let page = self.pages.iter_mut().find(|page| page.key == key).unwrap();
            page.cards = cards;
            page.single = single;
            return;
        }
        self.pages
            .iter_mut()
            .find(|page| page.key == self.current)
            .unwrap()
            .fade
            .restart(0., now, false);
        self.current = key;
        if let Some(page) = self.pages.iter_mut().find(|page| page.key == key) {
            let reset = page.fade.completed(now);
            page.fade.restart(1., now, reset);
            page.cards = cards;
            page.single = single;
        } else {
            let mut fade = FeatureFade::settled(0.);
            fade.restart(1., now, false);
            self.pages.push(FeaturePage {
                key,
                cards,
                single,
                fade,
            });
        }
    }
    fn prune(&mut self, now: std::time::Instant) {
        if self
            .pages
            .iter()
            .all(|page| page.key == self.current || page.fade.removed(now))
        {
            self.pages.retain(|page| page.key == self.current);
        }
    }
    fn animating(&self, now: std::time::Instant) -> bool {
        self.pages.len() > 1
            || self
                .pages
                .iter()
                .any(|page| page.fade.transition.value(now).1)
    }
}

fn feature_metadata_stack(
    title: Option<(&str, Pixels)>,
    artist: &str,
    labels: &[String],
    index: usize,
    parents: &[feishin_core::styles::Target],
    scale: (Pixels, f32),
    cx: &App,
) -> AnyElement {
    let (rem, zoom) = scale;
    let single = parents[0]
        .classes
        .iter()
        .any(|class| class == "fs-feature-carousel-module-single-carousel-container");
    let stack_target = feishin_core::styles::Target::new("div", "", &["mantine-Stack-root"]);
    gpui::div()
        .id(("home-feature-stack", index))
        .debug_selector(move || format!("home-feature-stack-{index}"))
        .v_flex()
        .min_w_0()
        .max_w_full()
        .gap(rem * 0.5)
        .when_some(title, |stack, (title, size)| {
            stack.child(
                gpui::div()
                    .text_size(size)
                    .font_weight(FontWeight::BLACK)
                    .line_height(relative(1.1))
                    .text_shadows(feature_text_shadow(zoom))
                    .debug_selector(move || format!("home-feature-heading-{index}"))
                    .map(|title| {
                        let target = feature_text_target(
                            "h1",
                            "title",
                            &format!("{}px", f32::from(size) / zoom),
                            900,
                            "1.1",
                        );
                        feature_style(
                            title,
                            parents,
                            Some(feature_target("div", "metadata-section")),
                            &[stack_target.clone(), target],
                            cx,
                        )
                    })
                    .child(title.to_owned()),
            )
        })
        .when(!artist.is_empty(), |stack| {
            stack.child(
                gpui::div()
                    .id(("home-feature-artist", index))
                    .debug_selector(move || format!("home-feature-artist-{index}"))
                    .w_full()
                    .text_shadows(feature_text_shadow(zoom))
                    .text_size(rem * if single { 1. } else { 0.875 })
                    .line_height(if single {
                        relative(1.1)
                    } else {
                        (rem * 18. / 16.).into()
                    })
                    .font_weight(if single {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .when(!single, |artist| {
                        artist.line_clamp(1).text_align(TextAlign::Center)
                    })
                    .map(|artist| {
                        let target = feature_text_target(
                            if single { "h5" } else { "div" },
                            if single { "title" } else { "artist" },
                            if single { "1rem" } else { ".875rem" },
                            if single { 700 } else { 500 },
                            if single { "1.1" } else { "1.125rem" },
                        );
                        feature_style(
                            artist,
                            parents,
                            Some(feature_target("div", "metadata-section")),
                            &[stack_target.clone(), target],
                            cx,
                        )
                    })
                    .child(artist.to_owned()),
            )
        })
        .child(feature_metadata_labels(labels, index, parents, scale, cx))
        .map(|stack| {
            feature_style(
                stack,
                parents,
                Some(feature_target("div", "metadata-section")),
                &[stack_target],
                cx,
            )
        })
        .into_any_element()
}

fn feature_metadata_labels(
    labels: &[String],
    index: usize,
    parents: &[feishin_core::styles::Target],
    scale: (Pixels, f32),
    cx: &App,
) -> AnyElement {
    let (rem, zoom) = scale;
    let single = parents[0]
        .classes
        .iter()
        .any(|class| class == "fs-feature-carousel-module-single-carousel-container");
    let stack_target = feishin_core::styles::Target::new("div", "", &["mantine-Stack-root"]);
    let group_target = feishin_core::styles::Target::new("div", "", &["mantine-Group-root"]);
    gpui::div()
        .id(("home-feature-labels", index))
        .debug_selector(move || format!("home-feature-labels-{index}"))
        .h_flex()
        .w_full()
        .flex_wrap()
        .items_center()
        .gap(rem * 0.25)
        .when(!single, |div| div.justify_center())
        .children(labels.iter().enumerate().map(|(slot, label)| {
            let id = index * 10 + slot;
            if single {
                gpui::div()
                    .id(("home-feature-label", id))
                    .min_w_0()
                    .max_w_full()
                    .debug_selector(move || format!("home-feature-label-{id}"))
                    .text_size(rem * 13. / 16.)
                    .line_height(rem)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_shadows(feature_text_shadow(zoom))
                    .map(|label| {
                        let target = feature_text_target("div", "title", ".8125rem", 600, "1rem");
                        feature_style(
                            label,
                            parents,
                            Some(feature_target("div", "metadata-section")),
                            &[stack_target.clone(), group_target.clone(), target],
                            cx,
                        )
                    })
                    .child(format!(
                        "{label}{}",
                        if slot + 1 < labels.len() { " •" } else { "" }
                    ))
                    .into_any_element()
            } else {
                gpui::div()
                    .id(("home-feature-badge", id))
                    .debug_selector(move || format!("home-feature-badge-{id}"))
                    .h_flex()
                    .h(rem * 18. / 16.)
                    .px(rem * 0.5)
                    .py(rem * 0.25)
                    .border(gpui::px(1.))
                    .border_color(transparent_black())
                    .rounded(rem * 5. / 16.)
                    .min_w_0()
                    .max_w_full()
                    .overflow_hidden()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .id(("home-feature-badge-label", id))
                            .debug_selector(move || format!("home-feature-badge-label-{id}"))
                            .min_w_0()
                            .text_size(rem * 10. / 16.)
                            .line_height(rem)
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(rem / 64.)
                            .truncate()
                            .text_shadows(feature_text_shadow(zoom))
                            .map(|label| {
                                let mut badge = feishin_core::styles::Target::new(
                                    "div",
                                    "",
                                    &["mantine-Badge-root", "fs-badge-module-root"],
                                );
                                badge
                                    .attributes
                                    .insert("data-variant".into(), "transparent".into());
                                let mut target = feature_target("span", "badge");
                                target.classes.push("mantine-Badge-label".into());
                                feature_style(
                                    label,
                                    parents,
                                    Some(feature_target("div", "metadata-section")),
                                    &[stack_target.clone(), group_target.clone(), badge, target],
                                    cx,
                                )
                            })
                            .child(label.to_uppercase()),
                    )
                    .map(|badge| {
                        let mut target = feishin_core::styles::Target::new(
                            "div",
                            "",
                            &["mantine-Badge-root", "fs-badge-module-root"],
                        );
                        target
                            .attributes
                            .insert("data-variant".into(), "transparent".into());
                        feature_style(
                            badge,
                            parents,
                            Some(feature_target("div", "metadata-section")),
                            &[stack_target.clone(), group_target.clone(), target],
                            cx,
                        )
                    })
                    .into_any_element()
            }
        }))
        .map(|group| {
            feature_style(
                group,
                parents,
                Some(feature_target("div", "metadata-section")),
                &[stack_target, group_target],
                cx,
            )
        })
        .into_any_element()
}

#[derive(Default)]
pub struct Section {
    pub items: Vec<Item>,
    pub offset: usize,
    pub visible: usize,
    pub loading: bool,
    pub loading_more: bool,
    pub has_more: bool,
    pub error: String,
    pub generation: u64,
    pub pending_next: bool,
    pub grid: std::cell::RefCell<crate::grid_carousel::State>,
    pub first_page_empty: bool,
}

impl Section {
    pub(crate) fn hidden(&self) -> bool {
        self.first_page_empty || (self.items.is_empty() && !self.loading && self.error.is_empty())
    }

    pub(crate) fn receive(
        &mut self,
        pages: Vec<Page>,
        more: bool,
        kind: Kind,
        excluded: Option<&str>,
    ) {
        if !more {
            self.items.clear();
            self.offset = 0;
            self.first_page_empty = false;
        }
        for page in pages {
            if excluded.is_some() {
                self.append_excluding(page, kind, excluded);
            } else {
                self.append(page, kind);
            }
        }
    }
    pub(crate) fn append(&mut self, page: Page, kind: Kind) {
        self.append_excluding(page, kind, None);
    }
    pub(crate) fn append_excluding(&mut self, page: Page, kind: Kind, excluded: Option<&str>) {
        let previous_len = self.items.len();
        let first_page = self.offset == 0;
        self.offset += page.items.len();
        self.has_more = if matches!(kind, Kind::Feature | Kind::Genres) {
            page.has_more && !page.items.is_empty()
        } else {
            page.items.len() >= 20
        };
        let mut seen: HashSet<_> = self
            .items
            .iter()
            .map(|item| (item.server_id, item.id.clone()))
            .collect();
        self.items.extend(page.items.into_iter().filter(|item| {
            (kind != Kind::Feature || !item.cover_id.is_empty())
                && excluded != Some(item.id.as_str())
                && (kind != Kind::Feature || seen.insert((item.server_id, item.id.clone())))
        }));
        if first_page {
            self.first_page_empty = self.items.is_empty();
        }
        if kind == Kind::Feature && self.pending_next && self.items.len() > previous_len {
            self.visible = previous_len;
            self.pending_next = false;
        }
        if !self.has_more {
            self.pending_next = false;
        }
    }
}

pub(crate) fn load_pages(
    api: &feishin_core::api::Api,
    mut query: feishin_core::model::ListQuery,
    count: usize,
) -> anyhow::Result<Vec<Page>> {
    let mut pages = Vec::with_capacity(count);
    for _ in 0..count {
        let page = api.list(&query)?;
        let end = page.items.len() < query.limit;
        pages.push(page);
        if end {
            break;
        }
        query.offset += query.limit;
    }
    Ok(pages)
}
fn target(selector: String, element: impl IntoElement) -> AnyElement {
    gpui::div()
        .debug_selector(move || selector.clone())
        .child(element)
        .into_any_element()
}
impl Feishin {
    fn home_feature_source(&self, item: &Item) -> Option<(String, std::path::PathBuf)> {
        let placement = feishin_core::images::Placement::ItemCard;
        let source = self.artwork_path_at(item, placement)?.clone();
        let key = format!(
            "home-{}-{:x}-{}-animated-color-managed",
            self.cover_cache_key(item, placement),
            feishin_core::api::cover_hash(&source.to_string_lossy()),
            self.state.settings.blur_explicit_images && item.explicit()
        );
        Some((key, source))
    }

    pub fn sync_home_backgrounds(&mut self, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        let sources = if self.route == Route::Home && self.state.settings.home.feature {
            self.home_sections
                .get(&Kind::Feature)
                .into_iter()
                .flat_map(|section| {
                    section
                        .items
                        .iter()
                        .cycle()
                        .skip(section.visible)
                        .take(self.home_step(Kind::Feature))
                })
                .filter_map(|item| self.home_feature_source(item))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        self.home_backgrounds
            .retain(|key, _| sources.iter().any(|(current, _)| current == key));
        for (key, source) in sources {
            if self.home_backgrounds.contains_key(&key) {
                continue;
            }
            self.home_backgrounds.insert(key.clone(), None);
            let destination = self
                .data_dir
                .join("detail-backgrounds")
                .join(format!("{key}.png"));
            let stale_path = destination.clone();
            let revision = self.state.cache_revision;
            let task = cx.background_executor().spawn(async move {
                crate::player_display::prepare_background(&source, &destination, 0.)
            });
            cx.spawn(async move |view, cx| {
                let result = task.await;
                view.update(cx, |view, cx| {
                    if view.state.cache_revision != revision {
                        feishin_core::cache::discard_stale(&stale_path);
                        return;
                    }
                    let Some(cached) = view.home_backgrounds.get_mut(&key) else {
                        return;
                    };
                    match result {
                        Ok(background) => *cached = Some(background),
                        Err(error) => log::debug!("Home artwork color preparation failed: {error}"),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    pub fn load_home(&mut self, cx: &mut Context<Self>) {
        self.page_scroll.set_offset(point(px(0.), px(0.)));
        self.home_feature_wheel_at = None;
        self.home_backgrounds.clear();
        self.request_generation += 1;
        self.items.clear();
        self.has_more = false;
        self.loading = false;
        self.home_sections.clear();
        let enabled = self.state.settings.home.enabled().collect::<Vec<_>>();
        for kind in enabled {
            self.load_home_section(kind, false, cx);
        }
        cx.notify();
    }
    pub fn load_home_section(&mut self, kind: Kind, more: bool, cx: &mut Context<Self>) {
        let Some(api) = self.api() else { return };
        let section = self.home_sections.entry(kind).or_default();
        if more && (section.loading || !section.has_more) {
            return;
        }
        if !more && matches!(kind, Kind::Feature | Kind::Genres) {
            *section = Section::default();
        }
        self.home_generation = self.home_generation.wrapping_add(1);
        let generation = self.home_generation;
        let server = api.server.id;
        section.generation = generation;
        section.loading = true;
        section.loading_more = more;
        section.error.clear();
        let pages = if more {
            1
        } else {
            section.offset.div_ceil(20).max(1)
        };
        let offset = if more { section.offset } else { 0 };
        let query = kind.query(api.server.kind, offset);
        let task = cx.background_executor().spawn(smol::unblock(move || {
            if kind == Kind::Genres {
                let mut items = api.all_items(query)?;
                items.shuffle(&mut rand::thread_rng());
                items.truncate(18);
                Ok(vec![Page {
                    total: Some(items.len()),
                    items,
                    has_more: false,
                }])
            } else {
                load_pages(&api, query, pages)
            }
        }));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.route != Route::Home || this.state.active_server != Some(server) {
                    return;
                }
                let Some(section) = this
                    .home_sections
                    .get_mut(&kind)
                    .filter(|section| section.generation == generation)
                else {
                    return;
                };
                section.loading = false;
                section.loading_more = false;
                match result {
                    Ok(pages) => {
                        section.receive(pages, more, kind, None);
                        this.fetch_covers(cx);
                    }
                    Err(error) => {
                        log::warn!("Home section {} failed: {error}", kind.label());
                        section.error = error.to_string();
                    }
                }
                let fetch_more = this.home_sections.get(&kind).is_some_and(|section| {
                    kind == Kind::Feature && section.error.is_empty() && section.has_more
                        && (section.pending_next || section.items.len() < 40)
                        // Bound artwork filtering and repeated random pages on sparse libraries.
                        && section.offset < 1000
                });
                if fetch_more {
                    this.load_home_section(kind, true, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn home_step(&self, kind: Kind) -> usize {
        if kind == Kind::Feature {
            if self.state.settings.home.feature_style == "multiple" {
                feishin_core::home::feature_columns(
                    f32::from(self.home_feature_width) / self.state.settings.zoom,
                )
            } else {
                1
            }
        } else {
            20
        }
    }
    pub fn home_next(&mut self, kind: Kind, forward: bool, cx: &mut Context<Self>) {
        if kind != Kind::Feature {
            self.carousel_step(crate::grid_carousel::Source::Home(kind), forward, cx);
            return;
        }
        let step = self.home_step(kind);
        let Some(section) = self.home_sections.get_mut(&kind) else {
            return;
        };
        section.visible =
            feishin_core::home::feature_next(section.visible, section.items.len(), step, forward);
        self.prefetch_home_feature(cx);
        cx.notify();
    }
    fn prefetch_home_feature(&mut self, cx: &mut Context<Self>) {
        let threshold = if self.state.settings.home.feature_style == "multiple" {
            self.home_step(Kind::Feature) * 2
        } else {
            3
        };
        if self
            .home_sections
            .get(&Kind::Feature)
            .is_some_and(|section| {
                !section.loading
                    && section.has_more
                    && section.offset < 1000
                    && section.items.len().saturating_sub(section.visible) < threshold
            })
        {
            self.load_home_section(Kind::Feature, true, cx);
        }
    }
    fn home_feature_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let step = self.home_step(Kind::Feature);
        let delta = f32::from(event.delta.pixel_delta(px(1.)).y);
        let now = cx.background_executor().now();
        if !event.modifiers.shift
            || !delta.is_finite()
            || delta.abs() < 10.
            || self
                .home_sections
                .get(&Kind::Feature)
                .is_none_or(|section| section.items.len() <= step)
            || self.home_feature_wheel_at.is_some_and(|last| {
                now.saturating_duration_since(last) < std::time::Duration::from_millis(250)
            })
        {
            return;
        }
        self.home_feature_wheel_at = Some(now);
        self.home_next(Kind::Feature, delta < 0., cx);
    }
    pub(crate) fn home_feature_card(
        &self,
        item: &Item,
        index: usize,
        container: feishin_core::styles::Target,
        page: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use feishin_core::queue::Enqueue;
        let single = container
            .classes
            .iter()
            .any(|class| class == "fs-feature-carousel-module-single-carousel-container");
        let zoom = self.state.settings.zoom;
        let viewport = window.viewport_size().width;
        let (card_motion, shadow_progress) = hover_transition(
            SharedString::from(format!("home-feature-shadow-{page}-{index}")),
            300,
            window,
            cx,
        );
        let (image_motion, image_progress) = hover_transition(
            SharedString::from(format!("home-feature-image-motion-{page}-{index}")),
            300,
            window,
            cx,
        );
        let mut css_path = feature_path(single);
        css_path[0] = container;
        if card_motion.read(cx).target() != 0. {
            for target in &mut css_path[2..] {
                target.states.push("hover".into());
            }
        }
        let mut image_section = feature_target("div", "image-section");
        if image_motion.read(cx).target() != 0. {
            image_section.states.push("hover".into());
        }
        let rem = f32::from(cx.theme().font_size);
        let width = f32::from(self.home_feature_width);
        let tier = if width >= rem * 88. {
            4.
        } else if width >= rem * 62. {
            3.
        } else if width >= rem * 48. {
            2.
        } else if width >= rem * 36. {
            1.
        } else {
            0.
        };
        let image_height = if single { 200. } else { 160. + tier * 20. } * zoom;
        let cover_width = if single { 200. } else { 120. + tier * 20. } * zoom;
        let group: SharedString = format!("home-feature-image-{index}").into();
        let shadows = feature_shadows(shadow_progress, single, zoom);
        let open = item.clone();
        let genres = item
            .genre_names
            .iter()
            .take(2)
            .cloned()
            .chain((item.year > 0).then(|| item.year.to_string()))
            .collect::<Vec<_>>();
        let artist = if single {
            item.album_artist.as_str()
        } else {
            item.album_artist_names
                .first()
                .map_or(item.album_artist.as_str(), String::as_str)
        };
        let color = self
            .home_feature_source(item)
            .and_then(|(key, _)| self.home_backgrounds.get(&key))
            .and_then(Option::as_ref)
            .map_or_else(
                || {
                    if item.cover_id.is_empty() {
                        cx.theme().muted_foreground
                    } else {
                        cx.theme().background
                    }
                },
                |background| background.color,
            );
        let rgb = color.to_rgb();
        let shades = feishin_core::appearance::color_scale([rgb.r, rgb.g, rgb.b, rgb.a]);
        let gradient = feishin_core::appearance::Gradient {
            angle: 180.,
            corner: false,
            noise: false,
            stops: [shades[6], shades[9]]
                .map(|color| feishin_core::appearance::ColorStop {
                    color,
                    position: None,
                })
                .to_vec(),
        };
        let signature = format!("{gradient:?}");
        let gradient_key = format!("home-feature-gradient-{}", index % 6);
        let cache = self.theme_overlays.clone();
        let overlay = canvas(
            move |bounds, _, _| {
                cache.borrow_mut().get(
                    &gradient_key,
                    &signature,
                    &gradient,
                    (f32::from(bounds.size.width).ceil() as u32).clamp(1, 2048),
                    (f32::from(bounds.size.height).ceil() as u32).clamp(1, 2048),
                )
            },
            |bounds, image, window, _| {
                let _ = window.paint_image(bounds, Corners::default(), image, 0, false);
            },
        )
        .size_full();
        gpui::div()
            .id(("home-feature-card", index))
            .debug_selector(move || format!("home-feature-card-{index}"))
            .on_hover(transition_hover(card_motion))
            .relative()
            .v_flex()
            .flex_1()
            .min_w_0()
            .flex_basis(px(0.))
            .rounded(px(rem * 5. / 16.))
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(white())
            .cursor_pointer()
            .on_drag(
                Dragged {
                    items: vec![item.clone()],
                    keys: vec![],
                    sidebar_playlist: false,
                    playlist_source: None,
                },
                crate::context_preview::drag_constructor(cx),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.navigate(Route::Detail(Box::new(open.clone())), cx)
            }))
            .child(
                gpui::div()
                    .debug_selector(move || format!("home-feature-overlay-{index}"))
                    .absolute()
                    .inset_0()
                    .bg(color)
                    .child(overlay)
                    .opacity(0.7)
                    .map(|overlay| {
                        crate::styles::apply_in(
                            overlay,
                            feishin_core::styles::Target::new(
                                "div", "", &["fs-library-background-overlay-module-background-overlay"],
                            ),
                            css_path[..3].to_vec(),
                            cx,
                        )
                    }),
            )
            .when(single, |card| {
                card.children(
                    self.artwork_path_at(item, feishin_core::images::Placement::ItemCard)
                        .map(|path| {
                            gpui::div()
                                .debug_selector(move || format!("home-feature-background-{index}"))
                                .absolute()
                                .inset_0()
                                .size_full()
                                .v_flex()
                                .foreground_blur(px(rem * 3.))
                                .paint_scale(1.1)
                                .opacity(0.8)
                                .map(|background| {
                                    crate::styles::apply_in(
                                        background,
                                        feature_target("div", "blurred-background"),
                                        css_path[..3].to_vec(),
                                        cx,
                                    )
                                })
                                .child(
                                    gpui::img(path.clone())
                                        .debug_selector(move || {
                                            format!("home-feature-background-image-{index}")
                                        })
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                        }),
                )
            })
            .child(
                gpui::div()
                    .id(("home-feature-link", index))
                    .debug_selector(move || format!("home-feature-link-{index}"))
                    .block()
                    .w_full()
                    .h_full()
                    .child(
                        gpui::div()
                            .id(("home-feature-content", index))
                            .debug_selector(move || format!("home-feature-content-{index}"))
                            .relative()
                            .v_flex()
                            .w_full()
                            .gap(px(rem * 0.25))
                            .p(px(rem * 0.75))
                            .when(single, |div| {
                                div.flex_row()
                                    .gap(px(rem * 0.75))
                                    .p(px(rem * 1.5))
                                    .items_end()
                                    .min_h(px(240. * zoom))
                            })
                            .when(!single, |div| {
                                div.items_center().min_h(px((280. + tier * 20.) * zoom))
                            })
                            .when(!single, |div| {
                                div.child(
                                    gpui::div()
                                        .id(("home-feature-title", index))
                                        .debug_selector(move || {
                                            format!("home-feature-title-{index}")
                                        })
                                        .w_full()
                                        .h(px((40. + tier * 5.) * zoom))
                                        .flex_shrink_0()
                                        .flex()
                                        .justify_center()
                                        .child(
                                            gpui::div()
                                                .text_size(px(rem))
                                                .font_weight(FontWeight::BOLD)
                                                .line_height(px(rem * 1.375))
                                                .line_clamp(2)
                                                .text_align(TextAlign::Center)
                                                .text_shadows(feature_text_shadow(zoom))
                                                .debug_selector(move || {
                                                    format!("home-feature-title-text-{index}")
                                                })
                                                .map(|title| {
                                                    let target = feature_text_target(
                                                        "div", "title", "1rem", 700, "1.375rem",
                                                    );
                                                    feature_style(
                                                        title,
                                                        &css_path,
                                                        Some(feature_target(
                                                            "div",
                                                            "title-section",
                                                        )),
                                                        &[target],
                                                        cx,
                                                    )
                                                })
                                                .child(item.name.clone()),
                                        )
                                        .map(|title| {
                                            feature_style(
                                                title,
                                                &css_path,
                                                Some(feature_target("div", "title-section")),
                                                &[],
                                                cx,
                                            )
                                        }),
                                )
                            })
                            .child(
                                gpui::div()
                                    .id(("home-feature-image-section", index))
                                    .debug_selector(move || {
                                        format!("home-feature-image-section-{index}")
                                    })
                                    .relative()
                                    .group(group.clone())
                                    .on_hover(transition_hover(image_motion))
                                    .h(px(image_height))
                                    .w_full()
                                    .flex_shrink_0()
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .when(single, |div| div.w(px(200. * zoom)).justify_start())
                                    .child(
                                        gpui::div()
                                            .id(("home-feature-cover", index))
                                            .debug_selector(move || {
                                                format!("home-feature-cover-{index}")
                                            })
                                            .w(px(cover_width))
                                            .max_w_full()
                                            .h_full()
                                            .rounded(px(rem * 5. / 16.))
                                            .overflow_hidden()
                                            .foreground_filters(shadows)
                                            .map(|cover| {
                                                feature_style(
                                                    cover,
                                                    &css_path,
                                                    Some(image_section.clone()),
                                                    &[feature_target(
                                                        "div",
                                                        "album-image-container",
                                                    )],
                                                    cx,
                                                )
                                            })
                                            .child(self.image_content_in(
                                                item,
                                                feishin_core::images::Placement::ItemCard,
                                                px(rem * 5. / 16.),
                                                Some((feature_target("img", "album-image"), {
                                                    let mut path = css_path.clone();
                                                    path.push(image_section.clone());
                                                    path.push(feature_target(
                                                        "div",
                                                        "album-image-container",
                                                    ));
                                                    path
                                                })),
                                                cx,
                                            ))
                                            .child(
                                                gpui::div()
                                                    .absolute()
                                                    .inset_0()
                                                    .rounded(px(rem * 5. / 16.))
                                                    .bg(black().opacity(0.4 * image_progress)),
                                            ),
                                    )
                                    .child(
                                        gpui::div()
                                            .absolute()
                                            .left(relative(0.5))
                                            .top(relative(0.5))
                                            .w(px(112. * zoom + rem))
                                            .h(px(48. * zoom))
                                            .ml(px(-(112. * zoom + rem) / 2.))
                                            .mt(px(-24. * zoom))
                                            .opacity(image_progress)
                                            .child(gpui::div()
                                                .debug_selector(move || format!("home-feature-play-group-{index}"))
                                                .h_flex()
                                                .size_full()
                                                .gap(px(rem * 0.5))
                                                .items_center()
                                                .justify_center()
                                                .children(
                                                [Enqueue::Next, Enqueue::Now, Enqueue::End].map(
                                                    |mode| {
                                                        let play = item.clone();
                                                        let held = item.clone();
                                                        let suffix = match mode {
                                                            Enqueue::Next => "next",
                                                            Enqueue::Now => "now",
                                                            Enqueue::End => "last",
                                                        };
                                                        crate::styles::play_control_in(
                                                            SharedString::from(format!(
                                                                "home-feature-{suffix}-{index}"
                                                            )),
                                                            mode,
                                                            zoom,
                                                            {
                                                                let mut path = css_path.clone();
                                                                path.push(image_section.clone());
                                                                path.push(feature_target("div", "play-button-overlay"));
                                                                path
                                                            },
                                                            cx,
                                                        )
                                                        .disabled(!self.library_fetches.is_empty())
                                                        .on_click(cx.listener(
                                                            move |view, _, _, cx| {
                                                                view.enqueue_items(
                                                                    vec![play.clone()],
                                                                    mode,
                                                                    false,
                                                                    cx,
                                                                );
                                                                cx.stop_propagation();
                                                            },
                                                        ))
                                                        .on_long_press(cx.listener(
                                                            move |view, _, _, cx| {
                                                                view.enqueue_items(
                                                                    vec![held.clone()],
                                                                    mode,
                                                                    true,
                                                                    cx,
                                                                )
                                                            },
                                                        ))
                                                    },
                                                ),
                                            )
                                                .map(|group| feature_style(
                                                    group, &css_path, Some(image_section.clone()),
                                                    &[
                                                        feature_target("div", "play-button-overlay"),
                                                        feishin_core::styles::Target::new("div", "", &["fs-play-button-group-module-play-button-group"]),
                                                    ], cx,
                                                )),
                                            )
                                            .map(|overlay| {
                                                feature_style(
                                                    overlay,
                                                    &css_path,
                                                    Some(image_section.clone()),
                                                    &[feature_target("div", "play-button-overlay")],
                                                    cx,
                                                )
                                            }),
                                    )
                                    .map(|image| {
                                        feature_style(
                                            image,
                                            &css_path,
                                            Some(image_section.clone()),
                                            &[],
                                            cx,
                                        )
                                    }),
                            )
                            .when(!single || f32::from(viewport) / zoom >= 576., |div| {
                                div.child(
                                    gpui::div()
                                        .id(("home-feature-metadata", index))
                                        .debug_selector(move || {
                                            format!("home-feature-metadata-{index}")
                                        })
                                        .w_full()
                                        .h(px((60. + tier * 5.) * zoom))
                                        .flex_shrink_0()
                                        .v_flex()
                                        .items_center()
                                        .justify_center()
                                        .when(single, |div| {
                                            div.w_auto()
                                                .h_auto()
                                                .flex_1()
                                                .min_w_0()
                                                .items_start()
                                                .text_align(TextAlign::Left)
                                        })
                                        .child(feature_metadata_stack(
                                            single.then(|| {
                                                (
                                                    item.name.as_str(),
                                                    px(feishin_core::home::feature_title_size(
                                                        &item.name,
                                                        f32::from(viewport),
                                                        rem,
                                                    )),
                                                )
                                            }),
                                            artist,
                                            &genres,
                                            index,
                                            &css_path,
                                            (px(rem), zoom),
                                            cx,
                                        ))
                                        .map(|metadata| {
                                            feature_style(
                                                metadata,
                                                &css_path,
                                                Some(feature_target("div", "metadata-section")),
                                                &[],
                                                cx,
                                            )
                                        }),
                                )
                            })
                            .map(|content| feature_style(content, &css_path, None, &[], cx)),
                    )
                    .map(|link| {
                        let mut path = css_path.clone();
                        path.truncate(4);
                        let target = path.pop().unwrap();
                        crate::styles::apply_in(link, target, path, cx)
                    }),
            )
            .map(|card| {
                let mut path = css_path.clone();
                path.truncate(3);
                let target = path.pop().unwrap();
                crate::styles::apply_in(card, target, path, cx)
            })
            .into_any_element()
    }

    pub fn home_view(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.state.settings.home.enabled().collect::<Vec<_>>();
        let font = cx.theme().font_size;
        let page = gpui::div()
            .id("home-page")
            .debug_selector(|| "home-page".into())
            .v_flex()
            .size_full()
            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
            .native_vertical_scroll()
            .scroll_handle(&self.page_scroll)
            .child(
                gpui::div()
                    .id("home-stack")
                    .debug_selector(|| "home-stack".into())
                    .v_flex()
                    .flex_shrink_0()
                    .w_full()
                    .gap(font * 2.)
                    .px(font * 2.)
                    .pt(font
                        * if self.window_bar_style_active == "web" {
                            5.
                        } else {
                            3.
                        })
                    .pb(font * 5.)
                    .when(self.api().is_none(), |div| {
                        div.child(self.tr("Select a server and sign in to browse your library"))
                    })
                    .children(enabled.into_iter().enumerate().filter_map(
                        |(section_index, kind)| {
                            if kind == Kind::Genres {
                                return Some(self.home_genres_view(window, cx));
                            }
                            if !matches!(kind, Kind::Feature | Kind::Genres) {
                                return self.grid_carousel_view(
                                    crate::grid_carousel::Source::Home(kind),
                                    window,
                                    cx,
                                );
                            }
                            let section = self.home_sections.get(&kind);
                            let loading = section.is_some_and(|section| section.loading);
                            let visible = section.map_or(0, |section| section.visible);
                            let step = self.home_step(kind);
                            let items = section.map_or(&[][..], |section| section.items.as_slice());
                            let cards: Vec<_> = if kind == Kind::Feature {
                                items
                                    .iter()
                                    .cycle()
                                    .skip(visible)
                                    .take(step)
                                    .enumerate()
                                    .map(|(slot, item)| (visible + slot, item))
                                    .collect()
                            } else {
                                items.iter().enumerate().skip(visible).take(step).collect()
                            };
                            let nav_reveal = (kind == Kind::Feature).then(|| {
                                hover_transition(
                                    ("home-feature-nav-reveal", section_index),
                                    200,
                                    window,
                                    cx,
                                )
                            });
                            let container_hovered = nav_reveal
                                .as_ref()
                                .is_some_and(|(motion, _)| motion.read(cx).target() != 0.);
                            let feature_container = |single| {
                                let mut target = feature_path(single)[0].clone();
                                if container_hovered {
                                    target.states.push("hover".into());
                                }
                                target
                            };
                            let card_elements: Vec<AnyElement> = if kind == Kind::Feature
                                && !cards.is_empty()
                            {
                                let now = cx.background_executor().now();
                                let motion = window.use_keyed_state(
                                    ("home-feature-pages", section_index),
                                    cx,
                                    |_, _| FeaturePages::default(),
                                );
                                let (pages, current, animating) = motion.update(cx, |motion, _| {
                                    motion.sync(
                                        visible,
                                        cards
                                            .iter()
                                            .map(|(index, item)| (*index, (*item).clone()))
                                            .collect(),
                                        step == 1,
                                        now,
                                    );
                                    (
                                        motion
                                            .pages
                                            .iter()
                                            .map(|page| {
                                                (
                                                    page.key,
                                                    page.single,
                                                    page.fade.transition.value(now).0,
                                                    page.cards.clone(),
                                                )
                                            })
                                            .collect::<Vec<_>>(),
                                        motion.current,
                                        motion.animating(now),
                                    )
                                });
                                if animating {
                                    window.request_animation_frame();
                                }
                                vec![
                                    gpui::div()
                                        .relative()
                                        .w_full()
                                        .children(pages.into_iter().map(
                                            |(key, single, opacity, cards)| {
                                                gpui::div()
                                                    .id(("home-feature-page", key))
                                                    .block_mouse_except_scroll()
                                                    .debug_selector(move || {
                                                        format!("home-feature-page-{key}")
                                                    })
                                                    .h_flex()
                                                    .w_full()
                                                    .gap(
                                                        cx.theme().font_size
                                                            * if self.home_feature_width
                                                                >= cx.theme().font_size * 48.
                                                            {
                                                                0.75
                                                            } else {
                                                                0.5
                                                            },
                                                    )
                                                    .when(crate::appearance::rtl(cx), |div| {
                                                        div.flex_row_reverse()
                                                    })
                                                    .when(key != current, |div| {
                                                        div.absolute().inset_0()
                                                    })
                                                    .map(|page| {
                                                        crate::styles::apply_in(
                                                            page,
                                                            feature_target("div", "carousel"),
                                                            [feature_container(single)],
                                                            cx,
                                                        )
                                                    })
                                                    .children(cards.into_iter().map(
                                                        |(index, item)| {
                                                            gpui::div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .flex_basis(px(0.))
                                                                .opacity(opacity)
                                                                .child(self.home_feature_card(
                                                                    &item,
                                                                    section_index * 100_000 + index,
                                                                    feature_container(single),
                                                                    key,
                                                                    window,
                                                                    cx,
                                                                ))
                                                        },
                                                    ))
                                            },
                                        ))
                                        .into_any_element(),
                                ]
                            } else {
                                Vec::new()
                            };
                            let measured = cx.entity().downgrade();
                            Some(
                                gpui::div()
                                    .id(SharedString::from(format!("home-section-{kind:?}")))
                                    .debug_selector(move || format!("home-section-{kind:?}"))
                                    .v_flex()
                                    .gap_3()
                                    .when(loading, |div| div.child(self.tr("Loading...")))
                                    .when_some(
                                        section.filter(|section| !section.error.is_empty()),
                                        |div, section| {
                                            div.child(
                                                gpui::div()
                                                    .text_color(cx.theme().danger)
                                                    .child(section.error.clone()),
                                            )
                                        },
                                    )
                                    .when(!loading && items.is_empty(), |div| {
                                        div.child(self.tr("No items"))
                                    })
                                    .child(
                                        gpui::div()
                                            .id(("home-carousel", section_index))
                                            .debug_selector(move || {
                                                format!("home-carousel-{section_index}")
                                            })
                                            .relative()
                                            .h_flex()
                                            .when(crate::appearance::rtl(cx), |div| {
                                                div.flex_row_reverse()
                                            })
                                            .gap_3()
                                            .overflow_x_scroll()
                                            .when(kind == Kind::Feature, |div| {
                                                div.group("home-feature-carousel")
                                                    .w_full()
                                                    .px(cx.theme().font_size * 0.75)
                                                    .gap(
                                                        cx.theme().font_size
                                                            * if self.home_feature_width
                                                                >= cx.theme().font_size * 48.
                                                            {
                                                                0.75
                                                            } else {
                                                                0.5
                                                            },
                                                    )
                                                    .overflow_hidden()
                                                    .on_scroll_wheel(cx.listener(
                                                        |this, event, _, cx| {
                                                            this.home_feature_wheel(event, cx)
                                                        },
                                                    ))
                                                    .child(
                                                        canvas(
                                                            move |bounds, window, cx| {
                                                                let measured = measured.clone();
                                                                window.defer(cx, move |_, cx| {
                                                                    measured
                                                                .update(cx, |view, cx| {
                                                                    if view.route == Route::Home
                                                                        && view.home_feature_width
                                                                            != bounds.size.width
                                                                    {
                                                                        view.home_feature_width =
                                                                            bounds.size.width;
                                                                        view.prefetch_home_feature(
                                                                            cx,
                                                                        );
                                                                        cx.notify();
                                                                    }
                                                                })
                                                                .ok();
                                                                });
                                                            },
                                                            |_, _, _, _| {},
                                                        )
                                                        .absolute()
                                                        .size_full(),
                                                    )
                                            })
                                            .when(kind == Kind::Feature, |container| {
                                                crate::styles::apply_in(
                                                    container,
                                                    feature_container(step == 1),
                                                    [],
                                                    cx,
                                                )
                                            })
                                            .children(card_elements)
                                            .when(kind == Kind::Feature, |div| {
                                                // Keep container hover above page hitboxes that occlude
                                                // pointer interactions with overlapping pages.
                                                div.child(
                                                    gpui::div()
                                                        .id(("home-feature-hover", section_index))
                                                        .absolute()
                                                        .inset_0()
                                                        .on_hover(transition_hover(
                                                            nav_reveal.as_ref().unwrap().0.clone(),
                                                        )),
                                                )
                                            })
                                            .when(
                                                kind == Kind::Feature && items.len() > step,
                                                |div| {
                                                    div.children([false, true].map(|forward| {
                                                        let size = cx.theme().font_size * 2.625;
                                                        let suffix = if forward {
                                                            "next"
                                                        } else {
                                                            "previous"
                                                        };
                                                        let id: SharedString = format!(
                                                "home-feature-{suffix}-navigation-{section_index}"
                                            )
                                            .into();
                                                        let selector = id.to_string();
                                                        let (border_motion, border_progress) =
                                                            hover_transition(
                                                                (
                                                                    "home-feature-nav-border",
                                                                    section_index * 2
                                                                        + usize::from(forward),
                                                                ),
                                                                200,
                                                                window,
                                                                cx,
                                                            );
                                                        let mut arrow = feature_target(
                                                            "button",
                                                            if forward {
                                                                "nav-arrow-right"
                                                            } else {
                                                                "nav-arrow-left"
                                                            },
                                                        );
                                                        arrow.classes.extend(
                                                            [
                                                                "mantine-ActionIcon-root",
                                                                "fs-action-icon-module-root",
                                                            ]
                                                            .map(str::to_owned),
                                                        );
                                                        arrow.attributes.insert(
                                                            "data-variant".into(),
                                                            "subtle".into(),
                                                        );
                                                        Button::new(id)
                                                    .state_style(crate::styles::state_style_path(
                                                        arrow,
                                                        vec![feature_container(step == 1)],
                                                    ))
                                                    .on_hover(transition_hover(border_motion))
                                                    .interaction_transform(
                                                        1.1,
                                                        0.95,
                                                        std::time::Duration::from_millis(200),
                                                        [0.25, 0.1, 0.25, 1.],
                                                    )
                                                    .debug_selector(move || selector.clone())
                                                    .custom(
                                                        ButtonCustomVariant::new(cx)
                                                            .color(transparent_black())
                                                            .foreground(white())
                                                            .hover(transparent_black())
                                                            .active(transparent_black()),
                                                    )
                                                    .icon(
                                                        Icon::default()
                                                            .path(if forward {
                                                                "native-icons/arrow-right.svg"
                                                            } else {
                                                                "native-icons/arrow-left.svg"
                                                            })
                                                            .with_size(
                                                                cx.theme().font_size * 1.125,
                                                            ),
                                                    )
                                                    .tooltip(self.tr(if forward {
                                                        "Next"
                                                    } else {
                                                        "Previous"
                                                    }))
                                                    .absolute()
                                                    .top(relative(0.5))
                                                    .mt(-size / 2.)
                                                    .when(forward, |button| {
                                                        button.right(cx.theme().font_size * 0.25)
                                                    })
                                                    .when(!forward, |button| {
                                                        button.left(cx.theme().font_size * 0.25)
                                                    })
                                                    .with_size(size)
                                                    .size(size)
                                                    .p_0()
                                                    .rounded(size / 2.)
                                                    .border_1()
                                                    .border_color(
                                                        white()
                                                            .opacity(0.25 + 0.1 * border_progress),
                                                    )
                                                    .backdrop_blur(px(
                                                        10. * self.state.settings.zoom
                                                    ))
                                                    .when(step == 1, |button| {
                                                        button
                                                            .opacity(nav_reveal.as_ref().unwrap().1)
                                                    })
                                                    .on_click(cx.listener(move |view, _, _, cx| {
                                                        view.home_next(Kind::Feature, forward, cx);
                                                        cx.stop_propagation();
                                                    }))
                                                    }))
                                                },
                                            ),
                                    )
                                    .into_any_element(),
                            )
                        },
                    )),
            )
            .into_any_element();
        gpui::div()
            .relative()
            .size_full()
            .child(page)
            .when(
                self.page_scroll.offset().y < -px(200. * self.state.settings.zoom),
                |page| {
                    use feishin_core::styles::Target;
                    let mut container =
                        Target::new("div", "", &["fs-page-header-module-container"]);
                    container
                        .attributes
                        .insert("data-visible".into(), "true".into());
                    let parents = [
                        container.clone(),
                        Target::new("div", "", &["fs-page-header-module-header"]),
                        Target::new("div", "", &["fs-page-header-module-title-wrapper"]),
                        Target::new(
                            "div",
                            "",
                            &["fs-library-header-bar-module-header-container"],
                        ),
                    ];
                    let mut title = Target::new(
                        "h1",
                        "",
                        &["mantine-Title-root", "fs-text-title-module-root"],
                    );
                    title
                        .inline_declarations
                        .push(("font-weight".into(), "700".into()));
                    page.child(
                        gpui::div()
                            .id("home-scroll-header")
                            .debug_selector(|| "home-scroll-header".into())
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(65. * self.state.settings.zoom))
                            .h_flex()
                            .px(font * 0.75)
                            .bg(cx.theme().background)
                            .overflow_hidden()
                            .occlude()
                            .child(crate::player_display::LibraryBackground {
                                image_mode: false,
                                image: None,
                                color: cx.theme().background,
                                overlay: self.theme_overlay("overlay-subheader"),
                                blur: px(0.),
                                zoom: self.state.settings.zoom,
                            })
                            .child(
                                gpui::div()
                                    .id("home-scroll-title")
                                    .debug_selector(|| "home-scroll-title".into())
                                    .role(gpui::Role::Heading)
                                    .relative()
                                    .text_size(font * 2.25)
                                    .line_height(font * 2.75)
                                    .font_weight(FontWeight::BOLD)
                                    .map(|heading| {
                                        crate::styles::apply_in(heading, title, parents, cx)
                                    })
                                    .child(self.tr("page.home.title")),
                            )
                            .map(|header| crate::styles::apply_in(header, container, [], cx)),
                    )
                },
            )
            .into_any_element()
    }
    pub fn home_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.home;
        gpui::div()
            .id("home-settings-page")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(
                Checkbox::new("home-feature-enabled")
                    .label(self.tr("setting.homeFeature"))
                    .checked(config.feature)
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.state.settings.home.feature = *value;
                        this.save();
                        cx.notify();
                    })),
            )
            .child(target(
                "home-feature-style".into(),
                crate::styles::button("home-feature-style", cx)
                    .label(format!(
                        "{}: {}",
                        self.tr("setting.homeFeatureStyle"),
                        self.tr(if config.feature_style == "single" {
                            "setting.homeFeatureStyle_optionSingle"
                        } else {
                            "setting.homeFeatureStyle_optionMultiple"
                        })
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.home.feature_style =
                            if this.state.settings.home.feature_style == "single" {
                                "multiple"
                            } else {
                                "single"
                            }
                            .into();
                        this.save();
                        cx.notify();
                    })),
            ))
            .children(config.items.iter().enumerate().map(|(index, item)| {
                gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(target(
                        format!("home-visible-{index}"),
                        Checkbox::new(("home-visible", index))
                            .label(self.tr(item.id.title_key()))
                            .checked(!item.disabled)
                            .on_click(cx.listener(move |this, value: &bool, _, cx| {
                                this.state.settings.home.items[index].disabled = !*value;
                                this.save();
                                cx.notify();
                            })),
                    ))
                    .child(target(
                        format!("home-up-{index}"),
                        crate::styles::button(("home-up", index), cx)
                            .label(crate::appearance::text("↑", cx))
                            .disabled(index == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.home.move_section(index, true);
                                this.save();
                                cx.notify();
                            })),
                    ))
                    .child(
                        crate::styles::button(("home-down", index), cx)
                            .label(crate::appearance::text("↓", cx))
                            .disabled(index + 1 == config.items.len())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.home.move_section(index, false);
                                this.save();
                                cx.notify();
                            })),
                    )
            }))
            .child(
                crate::styles::button("home-reset", cx)
                    .label(self.tr("Reset to defaults"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.home = Default::default();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                crate::styles::button("home-return", cx)
                    .label(self.tr("Home"))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Home, cx))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, Kind, Page, Section};
    #[gpui::test]
    fn home_page_frame_matches_original_application(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use feishin_core::{
            api::Api,
            model::{Server, Session},
        };
        use gpui::{AppContext, px, size};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/home-page-frame-reference.json")).unwrap();
        let dir = std::env::temp_dir().join(format!("feishin-home-frame-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = feishin_core::storage::State::default();
            state.settings.follow_system_theme = false;
            state.settings.window_bar_style = "web".into();
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        let app = captured.borrow_mut().take().unwrap();
        app.update(cx, |view, cx| {
            let server = Server {
                id: uuid::Uuid::new_v4(),
                name: "Frame fixture".into(),
                url: "http://127.0.0.1:1".into(),
                username: "fixture".into(),
                kind: feishin_core::model::ServerType::Navidrome,
                legacy_auth: false,
                options: Default::default(),
            };
            view.state.active_server = Some(server.id);
            view.sessions
                .insert(server.id, Api::new(server, Session::default()).unwrap());
            view.route = Route::Home;
            view.message.clear();
            view.home_sections.insert(
                Kind::Feature,
                Section {
                    items: vec![Item {
                        name: "Northern Lights".into(),
                        album_artist: "Fixture Artist".into(),
                        genre_names: vec!["Ambient".into()],
                        year: 2026,
                        kind: feishin_core::model::ItemKind::Album,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            );
            cx.notify();
        });
        cx.run_until_parked();
        for (selector, original) in [
            ("sidebar", ".fs-sidebar-module-container"),
            ("player-bar", ".fs-playerbar-module-container"),
            (
                "home-feature-card-0",
                ".fs-feature-carousel-module-carousel-item",
            ),
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            let actual = [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ];
            for (actual, expected) in actual
                .into_iter()
                .zip(reference["frames"][original].as_array().unwrap())
            {
                assert!(
                    (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "{selector}: {bounds:?}, expected {}",
                    reference["frames"][original]
                );
            }
        }
        assert!(cx.debug_bounds("home-scroll-header").is_none());
        app.update(cx, |view, cx| {
            for kind in [Kind::Random, Kind::RecentlyAdded, Kind::MostPlayed] {
                view.state
                    .settings
                    .home
                    .items
                    .push(feishin_core::home::Section {
                        id: kind,
                        disabled: false,
                    });
                view.home_sections.insert(
                    kind,
                    Section {
                        items: vec![Item {
                            name: "Other album".into(),
                            kind: feishin_core::model::ItemKind::Album,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                );
            }
            cx.notify();
        });
        cx.run_until_parked();
        let scroll: serde_json::Value =
            serde_json::from_str(include_str!("../assets/home-page-scroll-reference.json"))
                .unwrap();
        for frame in scroll.as_array().unwrap() {
            app.update(cx, |view, cx| {
                view.page_scroll.set_offset(gpui::point(
                    px(0.),
                    -px(frame["offset"].as_f64().unwrap() as f32),
                ));
                cx.notify();
            });
            cx.run_until_parked();
            assert_eq!(
                cx.debug_bounds("home-scroll-header").is_some(),
                frame["visible"] == "true"
            );
            if frame["visible"] == "true" {
                for (selector, key) in [
                    ("home-scroll-header", "bounds"),
                    ("home-scroll-title", "title"),
                ] {
                    let bounds = cx.debug_bounds(selector).unwrap();
                    for (actual, expected) in [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .zip(frame[key].as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{selector}: {bounds:?}, expected {}",
                            frame[key]
                        );
                    }
                }
            }
        }
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: gpui::point(px(1200.), px(500.)),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-201.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("home-scroll-header").is_some());
        app.update(cx, |view, cx| {
            view.navigate(Route::Settings, cx);
            view.back(cx);
            assert_eq!(view.page_scroll.offset().y, px(0.));
        });
        std::fs::remove_dir_all(dir).ok();
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn featured_background_layers_match_original_pixels(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use gpui::{AppContext, Context, IntoElement, Render, Window, prelude::*, px};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        struct Probe(gpui::Entity<Feishin>, bool, f32);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui::div()
                    .w(px(600.))
                    .h(px(self.2))
                    .flex()
                    .flex_row()
                    .child(self.0.update(cx, |view, cx| {
                        let Some(item) = view
                            .home_sections
                            .get(&Kind::Feature)
                            .and_then(|section| section.items.first())
                        else {
                            return gpui::div().into_any_element();
                        };
                        view.home_feature_card(
                            item,
                            0,
                            super::feature_path(self.1)[0].clone(),
                            0,
                            window,
                            cx,
                        )
                    }))
            }
        }
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-feature-background-linux-reference.json")
        } else {
            include_str!("../assets/home-feature-background-darwin-reference.json")
        })
        .unwrap();
        for (case, title) in reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|case| ["Album", ""].map(|title| (case, title)))
        {
            let single = case["single"].as_bool().unwrap();
            let height = case["height"].as_f64().unwrap() as f32;
            let dir = std::env::temp_dir()
                .join(format!("feishin-feature-layers-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let source = dir.join("cover.png");
            let color = u32::from_str_radix(&case["color"].as_str().unwrap()[1..], 16).unwrap();
            image::RgbaImage::from_pixel(
                32,
                32,
                image::Rgba([(color >> 16) as u8, (color >> 8) as u8, color as u8, 255]),
            )
            .save(&source)
            .unwrap();
            let captured = Rc::new(RefCell::new(None));
            let (_, cx) = cx.add_window_view(|window, cx| {
                let mut state = feishin_core::storage::State::default();
                state.settings.follow_system_theme = false;
                let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
                let probe = cx.new(|_| Probe(app.clone(), single, height));
                captured.replace(Some((app, probe.clone())));
                Root::new(probe, window, cx)
            });
            cx.simulate_resize(gpui::size(px(600.), px(height)));
            cx.run_until_parked();
            let (app, probe) = captured.borrow_mut().take().unwrap();
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.route = Route::Home;
                    view.home_feature_width = px(600.);
                    let item = Item {
                        id: "background".into(),
                        name: title.into(),
                        kind: feishin_core::model::ItemKind::Album,
                        cover_id: "cover".into(),
                        ..Default::default()
                    };
                    view.covers.insert(
                        view.cover_cache_key(&item, feishin_core::images::Placement::ItemCard),
                        source.clone(),
                    );
                    let (key, _) = view.home_feature_source(&item).unwrap();
                    let background = crate::player_display::prepare_background(
                        &source,
                        &dir.join("background.png"),
                        0.,
                    )
                    .unwrap();
                    view.home_backgrounds.insert(key, Some(background));
                    view.home_sections.insert(
                        Kind::Feature,
                        Section {
                            items: vec![item],
                            ..Default::default()
                        },
                    );
                    gpui_component::Theme::global_mut(cx).background = gpui::rgb(0x0a0a0a).into();
                    gpui_component::Theme::global_mut(cx).font_size = px(16.);
                });
                probe.update(cx, |_, cx| cx.notify());
                window.refresh();
            });
            cx.run_until_parked();
            cx.update(|window, cx| {
                assert!(matches!(
                    window.use_asset::<gpui::ImgResourceLoader>(
                        &gpui::Resource::from(source.clone()),
                        cx
                    ),
                    Some(Ok(_))
                ));
                window.refresh();
            });
            cx.run_until_parked();
            let scale = cx.update(|window, _| window.scale_factor());
            let pixels = cx.update(|window, _| {
                window
                    .rasterized_scene_for_test(gpui::size(px(600.), px(height)))
                    .unwrap()
            });
            let width = (600. * scale) as usize;
            for sample in case["samples"].as_array().unwrap() {
                let x = (sample["x"].as_f64().unwrap() as f32 * scale) as usize;
                let y = (sample["y"].as_f64().unwrap() as f32 * scale) as usize;
                let actual = &pixels[(y * width + x) * 4..(y * width + x) * 4 + 3];
                let expected = sample["rgb"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>();
                assert!(
                    actual
                        .iter()
                        .zip(&expected)
                        .all(|(a, b)| a.abs_diff(*b) <= 3),
                    "single={single}, title={title:?}, color={}, sample={sample}, actual={actual:?}",
                    case["color"]
                );
            }
            std::fs::remove_dir_all(dir).ok();
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn feature_text_shadows_match_original_glyph_layers(cx: &mut gpui::TestAppContext) {
        use gpui::{Context, IntoElement, Render, prelude::*, px};
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe(serde_json::Value, bool);
        impl Render for Probe {
            fn render(
                &mut self,
                window: &mut gpui::Window,
                _: &mut Context<Self>,
            ) -> impl IntoElement {
                let case = &self.0;
                let zoom = case["zoom"].as_f64().unwrap() as f32;
                window.text_system().set_optical_size_scale(zoom);
                let rem = case["rem"].as_f64().unwrap() as f32;
                gpui::div()
                    .relative()
                    .w(px(256.))
                    .h(px(160.))
                    .bg(gpui::rgb(0x777777))
                    .child(
                        gpui::div()
                            .absolute()
                            .left(px(32. * zoom))
                            .top(px(32. * zoom))
                            .w(px(160. * zoom))
                            .font_family("Inter")
                            .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![(
                                "tnum".into(),
                                1,
                            )])))
                            .text_size(px(case["size"].as_f64().unwrap() as f32 * rem / 16. * zoom))
                            .font_weight(gpui::FontWeight(case["weight"].as_f64().unwrap() as f32))
                            .line_height(gpui::relative(1.1))
                            .letter_spacing(px(case["tracking"].as_f64().unwrap_or(0.) as f32
                                * rem
                                / 16.
                                * zoom))
                            .text_color(gpui::white())
                            .when(case["wrap"] != true, |div| div.whitespace_nowrap())
                            .when(self.1, |div| {
                                div.text_shadows(super::feature_text_shadow(zoom))
                            })
                            .child(case["text"].as_str().unwrap().to_owned()),
                    )
            }
        }
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-text-shadow-linux-reference.json")
        } else {
            include_str!("../assets/home-text-shadow-darwin-reference.json")
        })
        .unwrap();
        let mut differences = Vec::new();
        for case in reference["cases"].as_array().unwrap() {
            let mut native = Vec::new();
            let mut original = Vec::new();
            for (shadow, key) in [(false, "plain"), (true, "shadow")] {
                let (_, window_cx) = cx.add_window_view(|_, _| Probe(case.clone(), shadow));
                window_cx.run_until_parked();
                native.push(window_cx.update(|window, _| {
                    window
                        .rasterized_scene_for_test(gpui::size(px(256.), px(160.)))
                        .unwrap()
                }));
                let filename = case[key].as_str().unwrap();
                let image = image::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(filename),
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(image.dimensions(), (512, 320));
                original.push(image.into_raw());
                if let Some(directory) = std::env::var_os("FEISHIN_TEXT_SHADOW_ACTUAL") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        std::path::Path::new(&directory).join(filename),
                        native.last().unwrap(),
                        512,
                        320,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
            let mut squared = 0_f64;
            let mut maximum = 0_i16;
            let mut signal = 0_usize;
            let mut samples = 0_usize;
            let ink_bounds = |pixels: &[u8]| {
                let mut bounds = [u32::MAX, u32::MAX, 0, 0];
                for (index, pixel) in pixels.chunks_exact(4).enumerate() {
                    if pixel[0] <= 121 {
                        continue;
                    }
                    let x = index as u32 % 512;
                    let y = index as u32 / 512;
                    bounds[0] = bounds[0].min(x);
                    bounds[1] = bounds[1].min(y);
                    bounds[2] = bounds[2].max(x);
                    bounds[3] = bounds[3].max(y);
                }
                bounds
            };
            for (actual, expected) in ink_bounds(&native[0])
                .into_iter()
                .zip(ink_bounds(&original[0]))
            {
                assert!(
                    actual.abs_diff(expected) <= 2,
                    "Zoomed glyph bounds differ: {case}"
                );
            }
            for index in (0..native[0].len()).filter(|index| index % 4 != 3) {
                // Compare the shadow independently of the native/browser glyph
                // antialiasing change caused by an offscreen filter layer.
                if original[0][index] != 119 || native[0][index] != 119 {
                    continue;
                }
                let expected = i16::from(original[0][index]) - i16::from(original[1][index]);
                let actual = i16::from(native[0][index]) - i16::from(native[1][index]);
                let error = (actual - expected).abs();
                squared += f64::from(error).powi(2);
                maximum = maximum.max(error);
                signal += usize::from(expected > 2);
                samples += 1;
            }
            let rms = (squared / samples as f64).sqrt();
            assert!(signal > 100, "Original shadow is missing: {case}");
            if rms > 0.7 || maximum > 8 {
                differences.push(format!("RMS {rms}, maximum {maximum}: {case}"));
            }
        }
        assert!(
            differences.is_empty(),
            "Text shadow differences: {differences:#?}"
        );
    }

    #[gpui::test]
    fn feature_custom_css_matches_original_wrappers(cx: &mut gpui::TestAppContext) {
        use feishin_core::styles::{Stylesheet, Target};
        use gpui::{prelude::*, *};
        cx.update(gpui_component::init);
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-feature-custom-css-linux-reference.json")
        } else {
            include_str!("../assets/home-feature-custom-css-darwin-reference.json")
        })
        .unwrap();
        let palette = feishin_core::appearance::Palette::default();
        for case in reference["cases"].as_array().unwrap() {
            let single = case["single"].as_bool().unwrap();
            for zoom in [1., 1.5] {
                cx.update(|cx| {
                    gpui_component::Theme::global_mut(cx).font_size = px(16. * zoom);
                    cx.set_global(
                        crate::styles::NativeStyles::new(
                            Stylesheet::parse([
                                super::feature_stylesheet(),
                                case["css"].as_str().unwrap(),
                            ])
                            .unwrap(),
                            palette.clone(),
                        )
                        .with_variables(
                            [("--theme-colors-foreground".into(), "white".into())].into(),
                        )
                        .with_zoom(zoom),
                    );
                    let mut path = super::feature_path(single);
                    if case["name"] == "hover" {
                        for target in &mut path[2..] {
                            target.states.push("hover".into());
                        }
                    }
                    let stack = Target::new("div", "", &["mantine-Stack-root"]);
                    let group = Target::new("div", "", &["mantine-Group-root"]);
                    let mut badge =
                        Target::new("div", "", &["mantine-Badge-root", "fs-badge-module-root"]);
                    badge
                        .attributes
                        .insert("data-variant".into(), "transparent".into());
                    for expected in case["elements"].as_array().unwrap() {
                        let classes = expected["classes"].as_array().unwrap();
                        let has = |name: &str| classes.iter().any(|class| class == name);
                        let (section, descendants) = if has("mantine-Badge-label") {
                            let mut label = super::feature_target("span", "badge");
                            label.classes.push("mantine-Badge-label".into());
                            (
                                "metadata-section",
                                vec![stack.clone(), group.clone(), badge.clone(), label],
                            )
                        } else if has("mantine-Badge-root") {
                            (
                                "metadata-section",
                                vec![stack.clone(), group.clone(), badge.clone()],
                            )
                        } else if has("fs-feature-carousel-module-artist") {
                            (
                                "metadata-section",
                                vec![
                                    stack.clone(),
                                    super::feature_text_target(
                                        "div", "artist", ".875rem", 500, "1.125rem",
                                    ),
                                ],
                            )
                        } else if expected["tag"] == "h1" {
                            (
                                "metadata-section",
                                vec![
                                    stack.clone(),
                                    super::feature_text_target("h1", "title", "28px", 900, "1.1"),
                                ],
                            )
                        } else if expected["tag"] == "h5" {
                            (
                                "metadata-section",
                                vec![
                                    stack.clone(),
                                    super::feature_text_target("h5", "title", "1rem", 700, "1.1"),
                                ],
                            )
                        } else if single {
                            (
                                "metadata-section",
                                vec![
                                    stack.clone(),
                                    group.clone(),
                                    super::feature_text_target(
                                        "div", "title", ".8125rem", 600, "1rem",
                                    ),
                                ],
                            )
                        } else {
                            (
                                "title-section",
                                vec![super::feature_text_target(
                                    "div", "title", "1rem", 700, "1.375rem",
                                )],
                            )
                        };
                        let mut element = super::feature_style(
                            div(),
                            &path,
                            Some(super::feature_target("div", section)),
                            &descendants,
                            cx,
                        );
                        let text = element.style().text.clone().unwrap();
                        let size = text.font_size.unwrap_or_else(|| panic!("Missing font size: {case}, {expected}, {descendants:?}, {text:?}")).to_pixels(px(16. * zoom));
                        let height = text
                            .line_height
                            .unwrap()
                            .to_pixels(size.into(), px(16. * zoom));
                        assert!(
                            (f32::from(size)
                                - expected["fontSize"].as_f64().unwrap() as f32 * zoom)
                                .abs()
                                < 0.001,
                            "Font size: {case}, {expected}, {text:?}"
                        );
                        assert!(
                            (f32::from(height)
                                - expected["lineHeight"].as_f64().unwrap() as f32 * zoom)
                                .abs()
                                < 0.001,
                            "Line height: {case}, {expected}, {text:?}"
                        );
                        assert_eq!(
                            text.font_weight.unwrap().0,
                            expected["fontWeight"].as_f64().unwrap() as f32,
                            "Weight: {case}, {expected}"
                        );
                        let color =
                            feishin_core::appearance::color(expected["color"].as_str().unwrap())
                                .unwrap();
                        let actual = text.color.unwrap().to_rgb();
                        for (actual, expected) in [actual.r, actual.g, actual.b, actual.a]
                            .into_iter()
                            .zip(color)
                        {
                            assert!(
                                (actual - expected).abs() < 0.001,
                                "Color: {case}, {expected}"
                            );
                        }
                        let spacing = expected["letterSpacing"].as_str().unwrap();
                        let spacing = if spacing == "normal" {
                            0.
                        } else {
                            spacing.trim_end_matches("px").parse::<f32>().unwrap()
                        };
                        assert!(
                            (f32::from(text.letter_spacing.unwrap_or(px(0.))) - spacing * zoom)
                                .abs()
                                < 0.001,
                            "Tracking: {case}, {expected}, {text:?}"
                        );
                        if !has("mantine-Badge-root") {
                            let shadow = text.text_shadows.unwrap();
                            assert_eq!(
                                shadow.len(),
                                usize::from(expected["shadow"] != "none"),
                                "Shadow: {case}, {expected}"
                            );
                            if let Some(shadow) = shadow.first() {
                                assert_eq!(shadow.blur_radius, px(8. * zoom));
                            }
                        }
                    }
                });
            }
        }
    }

    #[gpui::test]
    fn feature_custom_css_updates_live_geometry_artwork_and_pointer_styles(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::app::{Feishin, Route};
        use feishin_core::images::Placement;
        use gpui::{AppContext, Modifiers, point, px};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let dir = std::env::temp_dir().join(format!("feishin-home-css-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("cover.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([80, 120, 160, 255]))
            .save(&source)
            .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(Default::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(px(1024.), px(900.)));
        cx.run_until_parked();
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-feature-custom-css-linux-reference.json")
        } else {
            include_str!("../assets/home-feature-custom-css-darwin-reference.json")
        })
        .unwrap();
        for single in [false, true] {
            for zoom in [1., 1.5] {
                // Keep enough content width for the fixed CSS dimensions being tested,
                // independently of the navigation sidebar's default width and zoom.
                cx.simulate_resize(gpui::size(px(1024. + 400. * zoom), px(900.)));

                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.route = Route::Home;
                        view.state.settings.home.items.clear();
                        view.state.settings.home.feature_style =
                            if single { "single" } else { "multiple" }.into();
                        view.state.settings.zoom = zoom;
                        view.state.settings.css = Default::default();
                        let item = Item {
                            id: "css-album".into(),
                            name: "Northern Lights".into(),
                            album_artist: "Artist".into(),
                            album_artist_names: vec!["Artist".into()],
                            genre_names: vec!["Rock".into()],
                            year: 1999,
                            cover_id: "cover".into(),
                            ..Default::default()
                        };
                        view.covers.insert(
                            view.cover_cache_key(&item, Placement::ItemCard),
                            source.clone(),
                        );
                        view.home_sections.insert(
                            Kind::Feature,
                            Section {
                                items: vec![item; 4],
                                ..Default::default()
                            },
                        );
                        view.apply_theme(window, cx);
                        cx.notify();
                    })
                });
                cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
                cx.run_until_parked();
                let baseline_image = cx.debug_bounds("home-feature-image-section-0").unwrap();
                let baseline_labels = cx.debug_bounds("home-feature-labels-0").unwrap();
                let baseline_artist = cx.debug_bounds("home-feature-artist-0").unwrap();
                for name in ["layout", "artwork", "hover", "controls"] {
                    let css = if name == "controls" {
                        ".fs-feature-carousel-module-carousel-container:hover > button.fs-feature-carousel-module-nav-arrow-right { width:56px; height:44px; backdrop-filter:blur(4px); }
                         .fs-feature-carousel-module-carousel-item:hover .fs-feature-carousel-module-image-section:hover .fs-play-button-group-module-play-button-group { gap:14px; }
                         .fs-feature-carousel-module-image-section:hover .fs-play-button-group-module-play-button-group > button.fs-play-button-module-fill { background:rgb(12,150,120); width:54px; }".to_owned()
                    } else {
                        reference["cases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|case| case["single"] == single && case["name"] == name)
                            .unwrap()["css"]
                            .as_str()
                            .unwrap()
                            .to_owned()
                    };
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.state.settings.css.enabled = true;
                            view.state.settings.css.content = css;
                            view.apply_theme(window, cx);
                        })
                    });
                    cx.run_until_parked();
                    if name == "layout" {
                        assert_eq!(
                            cx.debug_bounds("home-feature-image-section-0")
                                .unwrap()
                                .size
                                .height,
                            px(180. * zoom)
                        );
                        assert_eq!(
                            cx.debug_bounds("home-feature-metadata-0")
                                .unwrap()
                                .size
                                .height,
                            px(88. * zoom)
                        );
                        let link = cx.debug_bounds("home-feature-link-0").unwrap();
                        let content = cx.debug_bounds("home-feature-content-0").unwrap();
                        assert!((content.left() - link.left() - px(6. * zoom)).abs() <= px(1.));
                    } else if name == "artwork" {
                        let cover = cx.debug_bounds("home-feature-cover-0").unwrap();
                        assert_eq!(cover.size.width, px(100. * zoom));
                        let operations =
                            cx.update(|window, _| window.painted_foreground_operations_for_test());
                        assert!(
                            operations
                                .contains(&vec![gpui::BackdropFilterOperation::Brightness(0.75)])
                        );
                        let images = cx.update(|window, _| window.painted_images_for_test());
                        assert!(
                            images
                                .iter()
                                .any(|(bounds, corners, opacity)| *bounds == cover
                                    && corners.top_left == px(9. * zoom)
                                    && (*opacity - 0.4).abs() < 0.001),
                            "Styled images: {images:?}, cover: {cover:?}"
                        );
                        if single {
                            assert!(
                                images
                                    .iter()
                                    .any(|(_, _, opacity)| (*opacity - 0.35).abs() < 0.001)
                            );
                        }
                    } else if name == "controls" {
                        let hover = cx
                            .debug_bounds("home-feature-image-section-0")
                            .unwrap()
                            .center();
                        cx.simulate_mouse_move(hover, None, Modifiers::none());
                        cx.run_until_parked();
                        cx.executor().advance_clock(Duration::from_millis(350));
                        cx.update(|window, _| window.refresh());
                        cx.run_until_parked();
                        let arrow = cx.debug_bounds("home-feature-next-navigation-0").unwrap();
                        assert_eq!(arrow.size, gpui::size(px(56. * zoom), px(44. * zoom)));
                        let backdrops = cx.update(|window, _| window.painted_backdrops_for_test());
                        assert!(
                            backdrops
                                .iter()
                                .any(|(bounds, _, _, radius, _)| *bounds == arrow
                                    && *radius == px(4. * zoom))
                        );
                        let play = cx.debug_bounds("home-feature-now-0").unwrap();
                        let next = cx.debug_bounds("home-feature-next-0").unwrap();
                        assert_eq!(play.size.width, px(54. * zoom));
                        assert!((play.left() - next.right() - px(14. * zoom)).abs() <= px(1.));
                        let quads =
                            cx.update(|window, _| window.painted_lines_and_quads_for_test());
                        assert!(quads.iter().any(|(_, color, _, _)| {
                            let rgb = color.to_rgb();
                            (rgb.r - 12. / 255.).abs() < 0.001
                                && (rgb.g - 150. / 255.).abs() < 0.001
                                && (rgb.b - 120. / 255.).abs() < 0.001
                        }));
                        cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
                        cx.run_until_parked();
                    } else {
                        let count_shadows = |cx: &mut gpui::VisualTestContext| {
                            cx.update(|window, _| window.painted_foreground_operations_for_test())
                                .iter()
                                .filter(|operations| {
                                    operations.iter().any(|operation| {
                                        matches!(
                                            operation,
                                            gpui::BackdropFilterOperation::DropShadow { .. }
                                        )
                                    })
                                })
                                .count()
                        };
                        let unhovered_shadows = count_shadows(cx);
                        let hover = cx
                            .debug_bounds("home-feature-image-section-0")
                            .unwrap()
                            .center();
                        cx.simulate_mouse_move(hover, None, Modifiers::none());
                        cx.run_until_parked();
                        let operations =
                            cx.update(|window, _| window.painted_foreground_operations_for_test());
                        assert_eq!(
                            count_shadows(cx),
                            unhovered_shadows - 1,
                            "Hover must clear the hovered card's custom artwork shadows: {operations:?}"
                        );
                        if !single {
                            let artist = cx.debug_bounds("home-feature-artist-0").unwrap();
                            let glyphs = cx.update(|window, _| window.painted_glyphs_for_test());
                            assert!(
                                glyphs
                                    .iter()
                                    .any(|(bounds, _, color)| artist.intersects(bounds) && {
                                        let rgb = color.to_rgb();
                                        (rgb.g - 200. / 255.).abs() < 0.001
                                    }),
                                "Hovered artist glyphs must paint inside {artist:?}"
                            );
                        }
                        cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
                        cx.run_until_parked();
                        // :hover stops matching immediately, even during the leave fade.
                        assert_eq!(count_shadows(cx), unhovered_shadows);
                    }
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.state.settings.css = Default::default();
                            view.apply_theme(window, cx);
                        })
                    });
                    cx.executor().advance_clock(Duration::from_millis(500));
                    cx.update(|window, _| window.refresh());
                    cx.run_until_parked();
                    assert_eq!(
                        cx.debug_bounds("home-feature-image-section-0").unwrap(),
                        baseline_image
                    );
                    assert_eq!(
                        cx.debug_bounds("home-feature-labels-0").unwrap(),
                        baseline_labels
                    );
                    assert_eq!(
                        cx.debug_bounds("home-feature-artist-0").unwrap(),
                        baseline_artist
                    );
                    assert_eq!(
                        app.read_with(cx, |view, _| view.home_sections[&Kind::Feature].items[0]
                            .id
                            .clone()),
                        "css-album"
                    );
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn feature_metadata_layout_matches_original_mantine(cx: &mut gpui::TestAppContext) {
        use gpui::{AppContext, Context, IntoElement, Render, prelude::*, px};
        use gpui_component::{Root, StyledExt};
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe(serde_json::Value);
        impl Render for Probe {
            fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
                let rem = px(self.0["rem"].as_f64().unwrap() as f32);
                let labels = self.0["labels"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let container = gpui::div()
                    .id("metadata-stack")
                    .debug_selector(|| "metadata-stack".into())
                    .v_flex()
                    .w(px(self.0["width"].as_f64().unwrap() as f32))
                    .font_family("Inter")
                    .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![(
                        "tnum".into(),
                        1,
                    )])))
                    .text_size(rem * 14. / 16.)
                    .line_height(rem * 18. / 16.)
                    .text_color(gpui::white());
                if self.0["card"] == true {
                    return container
                        .h(px(60.))
                        .items_center()
                        .justify_center()
                        .child(super::feature_metadata_stack(
                            None,
                            "Artist",
                            &labels,
                            0,
                            &super::feature_path(false),
                            (rem, 1.),
                            cx,
                        ))
                        .into_any_element();
                }
                container
                    .gap(rem * 0.5)
                    .child(
                        gpui::div()
                            .id("metadata-artist")
                            .debug_selector(|| "metadata-artist".into())
                            .w_full()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Artist"),
                    )
                    .child(super::feature_metadata_labels(
                        &labels,
                        0,
                        &super::feature_path(self.0["single"].as_bool().unwrap()),
                        (rem, 1.),
                        cx,
                    ))
                    .into_any_element()
            }
        }
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-feature-metadata-linux-reference.json")
        } else {
            include_str!("../assets/home-feature-metadata-darwin-reference.json")
        })
        .unwrap();
        let (root, cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe(reference["cases"][0].clone()));
            Root::new(probe, window, cx)
        });
        let probe = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Probe>().unwrap()
        });
        let card_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "linux") {
                include_str!("../assets/home-feature-metadata-card-linux-reference.json")
            } else {
                include_str!("../assets/home-feature-metadata-card-darwin-reference.json")
            })
            .unwrap();
        let cases = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .chain(
                card_reference["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned()
                    .map(|mut case| {
                        case["card"] = true.into();
                        case
                    }),
            );
        for case in cases {
            cx.update(|_, cx| {
                probe.update(cx, |probe, cx| {
                    probe.0 = case.clone();
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let mut label_slot = 0;
            for original in case["elements"].as_array().unwrap() {
                let selector = match original["type"].as_str().unwrap() {
                    "container" => "metadata-stack".to_owned(),
                    "stack" => if case["card"] == true {
                        "home-feature-stack-0"
                    } else {
                        "metadata-stack"
                    }
                    .to_owned(),
                    "artist" => if case["card"] == true {
                        "home-feature-artist-0"
                    } else {
                        "metadata-artist"
                    }
                    .to_owned(),
                    "group" => "home-feature-labels-0".to_owned(),
                    "root" => format!(
                        "home-feature-{}-{}",
                        if case["single"] == true {
                            "label"
                        } else {
                            "badge"
                        },
                        original["index"].as_u64().unwrap()
                    ),
                    "label" => {
                        let selector = format!("home-feature-badge-label-{label_slot}");
                        label_slot += 1;
                        selector
                    }
                    _ => unreachable!(),
                };
                let bounds = cx.debug_bounds(&selector).unwrap();
                for (index, value) in [
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ]
                .into_iter()
                .enumerate()
                {
                    assert!(
                        (f64::from(value) - original["bounds"][index].as_f64().unwrap()).abs()
                            <= 1.,
                        "{selector} {index}: {bounds:?} original {original:?}, {case:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn feature_page_fades_match_original_motion_presence() {
        use std::time::{Duration, Instant};
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/home-feature-pages-reference.json"))
                .unwrap();
        let start = Instant::now();
        for scenario in reference["scenarios"].as_array().unwrap() {
            let count = scenario["count"].as_u64().unwrap() as usize;
            let name = scenario["name"].as_str().unwrap();
            let mut motion = super::FeaturePages::default();
            let cards = |key: usize| {
                (0..count)
                    .map(|index| {
                        (
                            index,
                            Item {
                                id: format!("{key}-{index}"),
                                ..Default::default()
                            },
                        )
                    })
                    .collect()
            };
            motion.sync(0, cards(0), count == 1, start);
            assert_eq!(motion.pages.len(), 1);
            assert_eq!(motion.pages[0].fade.transition.value(start), (1., false));
            motion.sync(1, cards(1), count == 1, start);
            let offset = if name == "return-rest" {
                motion.sync(2, cards(2), count == 1, start + Duration::from_millis(50));
                motion.sync(0, cards(0), count == 1, start + Duration::from_millis(320));
                320
            } else if name != "complete" {
                let offset = scenario["interruptAt"].as_u64().unwrap();
                let key = if name == "return" { 0 } else { 2 };
                motion.sync(
                    key,
                    cards(key),
                    count == 1,
                    start + Duration::from_millis(offset),
                );
                offset
            } else {
                0
            };
            for frame in scenario["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                let now = start + Duration::from_millis(offset + milliseconds);
                motion.prune(now);
                let expected = frame["pages"].as_array().unwrap();
                assert_eq!(
                    motion.pages.len(),
                    expected.len(),
                    "{name} {count} {milliseconds}"
                );
                for (page, expected) in motion.pages.iter().zip(expected) {
                    assert_eq!(page.key, expected["page"].as_u64().unwrap() as usize);
                    assert_eq!(
                        page.cards.len(),
                        expected["items"].as_array().unwrap().len()
                    );
                    assert_eq!(
                        page.key != motion.current,
                        expected["position"] == "absolute"
                    );
                    assert_eq!(
                        expected["hitPage"].as_u64().unwrap() as usize,
                        motion.pages.last().unwrap().key
                    );
                    let opacity = page.fade.transition.value(now).0;
                    for item in expected["items"].as_array().unwrap() {
                        assert!(
                            (f64::from(opacity) - item["opacity"].as_f64().unwrap()).abs()
                                < 0.000002,
                            "{name} {count} {milliseconds}: {opacity} != {}",
                            item["opacity"]
                        );
                        assert_eq!(item["scale"], 1.);
                        assert_eq!(item["y"], 0.);
                    }
                }
            }
            assert_eq!(motion.pages.len(), 1);
            assert_eq!(
                motion.pages[0].key,
                scenario["finished"][0]["page"].as_u64().unwrap() as usize
            );
        }
    }

    #[test]
    fn feature_css_motion_matches_original_frames_and_reversals() {
        use gpui_component::animation::CssTransition;
        use std::time::{Duration, Instant};
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/home-feature-motion-reference.json"))
                .unwrap();
        let check = |name: &str, value: f32, expected: &serde_json::Value| {
            if name == "artwork-shadow" {
                let feishin_core::styles::BackdropFilter::Chain(operations) =
                    feishin_core::styles::backdrop_filter(expected.as_str().unwrap(), 16.).unwrap()
                else {
                    panic!("{expected}");
                };
                for (actual, expected) in super::feature_shadows(value, false, 1.)
                    .into_iter()
                    .zip(operations)
                {
                    let gpui::BackdropFilterOperation::DropShadow {
                        offset: a,
                        sigma: sa,
                        color: ca,
                    } = actual
                    else {
                        unreachable!()
                    };
                    let feishin_core::styles::FilterOperation::DropShadow {
                        offset: b,
                        sigma: sb,
                        color: cb,
                    } = expected
                    else {
                        unreachable!()
                    };
                    assert!(
                        a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 0.0001)
                            && (sa - sb).abs() < 0.0001,
                        "{name}: {actual:?} != {expected:?}"
                    );
                    assert!(
                        ca.unwrap()
                            .into_iter()
                            .zip(cb.unwrap())
                            .all(|(a, b)| (a - b).abs() <= 1. / 255.),
                        "{name}: {actual:?} != {expected:?}"
                    );
                }
            } else {
                let (actual, expected) = match name {
                    "navigation-border" => (
                        0.25 + 0.1 * value,
                        feishin_core::appearance::color(expected.as_str().unwrap()).unwrap()[3],
                    ),
                    "artwork-dimming" => (
                        0.4 * value,
                        feishin_core::appearance::color(expected.as_str().unwrap()).unwrap()[3],
                    ),
                    _ => (value, expected.as_f64().unwrap() as f32),
                };
                // Computed CSS colors serialize alpha with limited precision.
                let tolerance = if matches!(name, "navigation-border" | "artwork-dimming") {
                    1. / 255.
                } else {
                    0.00001
                };
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{name}: {actual} != {expected}"
                );
            }
        };
        for name in [
            "navigation-scale",
            "navigation-border",
            "navigation-opacity",
            "artwork-shadow",
            "artwork-dimming",
            "play-overlay-opacity",
        ] {
            let duration = Duration::from_millis(if name.starts_with("navigation") {
                200
            } else {
                300
            });
            let initial = if name == "navigation-scale" { 1. } else { 0. };
            let hovered = if name == "navigation-scale" { 1.1 } else { 1. };
            let mut transition = CssTransition::new(initial, duration, [0.25, 0.1, 0.25, 1.]);
            let mut now = Instant::now();
            for frame in reference["frames"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|frame| frame["name"] == name)
            {
                assert_eq!(
                    frame["duration"].as_f64().unwrap(),
                    duration.as_millis() as f64
                );
                let target = match frame["state"].as_str().unwrap() {
                    "active" => 0.95,
                    "leave" => initial,
                    _ => hovered,
                };
                transition.retarget(target, now);
                for sample in frame["samples"].as_array().unwrap() {
                    check(
                        name,
                        transition
                            .value(
                                now + Duration::from_millis(
                                    sample["milliseconds"].as_u64().unwrap(),
                                ),
                            )
                            .0,
                        &sample["value"],
                    );
                }
                now += Duration::from_millis(400);
            }
            for frame in reference["interruptions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|frame| frame["name"] == name)
            {
                let mut transition = CssTransition::new(initial, duration, [0.25, 0.1, 0.25, 1.]);
                transition.retarget(hovered, now);
                let interrupt = now + Duration::from_millis(frame["interruptAt"].as_u64().unwrap());
                transition.retarget(initial, interrupt);
                for sample in frame["samples"].as_array().unwrap() {
                    check(
                        name,
                        transition
                            .value(
                                interrupt
                                    + Duration::from_millis(
                                        sample["milliseconds"].as_u64().unwrap(),
                                    ),
                            )
                            .0,
                        &sample["value"],
                    );
                }
            }
        }
    }
    #[gpui::test]
    fn feature_arrow_transform_matches_original_pointer_frames(cx: &mut gpui::TestAppContext) {
        use gpui::{prelude::*, *};
        use gpui_component::{
            Icon, Root, Sizable,
            button::{Button, ButtonCustomVariant, ButtonVariants},
        };
        use std::time::Duration;
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().size_full().bg(black()).p_4().child(
                    Button::new("feature-motion-arrow")
                        .debug_selector(|| "feature-motion-arrow".into())
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .color(white())
                                .hover(white())
                                .active(white())
                                .foreground(black()),
                        )
                        .size(px(42.))
                        .rounded(px(21.))
                        .p_0()
                        .opacity(0.7)
                        .icon(
                            Icon::default()
                                .path("native-icons/arrow-right.svg")
                                .with_size(px(18.)),
                        )
                        .tooltip("Next")
                        .interaction_transform(
                            1.1,
                            0.95,
                            Duration::from_millis(200),
                            [0.25, 0.1, 0.25, 1.],
                        ),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let view = cx.new(|_| Probe);
            Root::new(view, window, cx)
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("feature-motion-arrow").unwrap();
        let display_scale = cx.update(|window, _| window.scale_factor());
        let outside = point(px(400.), px(300.));
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/home-feature-motion-reference.json"))
                .unwrap();
        let check = |cx: &mut gpui::VisualTestContext, samples: &serde_json::Value| {
            let mut previous = 0;
            for sample in samples.as_array().unwrap() {
                let ms = sample["milliseconds"].as_u64().unwrap();
                cx.executor()
                    .advance_clock(Duration::from_millis(ms - previous));
                previous = ms;
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                assert_eq!(cx.debug_bounds("feature-motion-arrow").unwrap(), bounds);
                let quads = cx.update(|window, _| window.painted_lines_and_quads_for_test());
                let quad = quads
                    .iter()
                    .find(|(_, color, rounded, _)| *rounded && color.l == 1.)
                    .unwrap();
                let scale = (f64::from(quad.0.size.width) as f32)
                    / display_scale
                    / f32::from(bounds.size.width);
                assert!(
                    (scale - sample["value"].as_f64().unwrap() as f32).abs() < 0.00001,
                    "{sample}: {scale}"
                );
                let tree = cx
                    .update(|window, _| window.accessibility_tree_for_test().cloned())
                    .unwrap();
                let ax = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == gpui::accesskit::Role::Button && node.label() == Some("Next")
                    })
                    .unwrap()
                    .1
                    .bounds()
                    .unwrap();
                assert!(
                    ((ax.x1 - ax.x0) as f32 / display_scale - f32::from(bounds.size.width) * scale)
                        .abs()
                        < 0.001
                );
            }
        };
        for frame in reference["frames"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|frame| frame["name"] == "navigation-scale")
        {
            match frame["state"].as_str().unwrap() {
                "hover" => cx.simulate_mouse_move(bounds.center(), None, Modifiers::none()),
                "active" => {
                    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none())
                }
                "release" => {
                    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none())
                }
                "leave" => cx.simulate_mouse_move(outside, None, Modifiers::none()),
                _ => unreachable!(),
            }
            cx.run_until_parked();
            check(cx, &frame["samples"]);
        }
        for frame in reference["interruptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|frame| frame["name"] == "navigation-scale")
        {
            cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(
                frame["interruptAt"].as_u64().unwrap(),
            ));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            cx.simulate_mouse_move(outside, None, Modifiers::none());
            cx.run_until_parked();
            check(cx, &frame["samples"]);
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        cx.update(|window, _| {
            let pixels = window
                .rasterized_scene_for_test(size(px(256.), px(256.)))
                .unwrap();
            let x = (f32::from(bounds.center().x) * display_scale) as usize;
            let y = ((f32::from(bounds.center().y) + 13.) * display_scale) as usize;
            let width = (256. * display_scale) as usize;
            assert!(
                pixels[(y * width + x) * 4..(y * width + x) * 4 + 3]
                    .iter()
                    .all(|n| n.abs_diff(179) <= 1),
                "Transform animation must preserve explicit opacity"
            );
        });
    }

    #[gpui::test]
    fn feature_page_pointer_frames_paint_original_opacity_and_stacking(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::app::{Feishin, Route};
        use feishin_core::images::Placement;
        use gpui::{AppContext, Modifiers, px};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-home-page-fades-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("cover.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([80, 120, 160, 255]))
            .save(&source)
            .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(Default::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Home;
                view.state.settings.home.items.clear();
                view.state.settings.home.feature_style = "single".into();
                view.state.settings.confirm_queue_changes = false;
                view.state.settings.lyric_fetch = false;
                let items = (0..4)
                    .map(|index| Item {
                        id: format!("page-{index}"),
                        name: format!("Page {index}"),
                        cover_id: format!("cover-{index}"),
                        kind: feishin_core::model::ItemKind::Album,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();
                for item in &items {
                    view.covers.insert(
                        view.cover_cache_key(item, Placement::ItemCard),
                        source.clone(),
                    );
                }
                view.home_sections.insert(
                    Kind::Feature,
                    Section {
                        items,
                        ..Default::default()
                    },
                );
                cx.notify();
            })
        });
        cx.run_until_parked();
        let cover = cx.debug_bounds("home-feature-cover-0").unwrap();
        let opacities = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, _| {
                window
                    .painted_images_for_test()
                    .into_iter()
                    .filter(|(bounds, _, _)| bounds.size == cover.size)
                    .map(|(_, _, opacity)| opacity)
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(opacities(cx), vec![1.]);
        let navigation = cx
            .debug_bounds("home-feature-next-navigation-0")
            .unwrap()
            .center();
        cx.simulate_click(navigation, Modifiers::none());
        cx.run_until_parked();
        for (elapsed, expected) in [(20, [0.99218, 0.160572]), (50, [0.955502, 0.378138])] {
            cx.executor()
                .advance_clock(Duration::from_millis(if elapsed == 20 { 20 } else { 30 }));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let actual = opacities(cx);
            assert_eq!(actual.len(), 2, "{actual:?}");
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 0.000002,
                    "{actual} != {expected}"
                );
            }
            let first = cx.debug_bounds("home-feature-page-0").unwrap();
            let second = cx.debug_bounds("home-feature-page-1").unwrap();
            assert_eq!(
                first, second,
                "Outgoing pages must retain the same layout without occupying another row"
            );
        }
        let navigation = cx
            .debug_bounds("home-feature-previous-navigation-0")
            .unwrap()
            .center();
        cx.simulate_click(navigation, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let actual = opacities(cx);
        assert_eq!(actual.len(), 2);
        for (actual, expected) in actual.iter().zip([0.972376, 0.361575]) {
            assert!(
                (actual - expected).abs() < 0.000002,
                "{actual} != {expected}"
            );
        }
        // The original DOM preserves page insertion order on an interrupted
        // return, so the outgoing page remains above the returning first page.
        cx.simulate_click(
            cover.origin + gpui::point(px(4.), px(4.)),
            Modifiers::none(),
        );
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(
                matches!(&view.route,Route::Detail(item) if item.id=="page-1"),
                "{:?}",
                view.route
            )
        });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn feature_carousel_artwork_actions_and_accessibility(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use feishin_core::{images::Placement, queue::Enqueue};
        use gpui::{
            AppContext, Modifiers, MouseButton,
            accesskit::{Action, ActionRequest, Role, TreeId},
            px,
        };
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-home-artwork-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("cover.png");
        let censored = dir.join("censored.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([96, 32, 16, 255]))
            .save(&source)
            .unwrap();
        image::RgbaImage::from_pixel(32, 32, image::Rgba([20, 100, 60, 255]))
            .save(&censored)
            .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(Default::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Home;
                view.state.settings.home.items.clear();
                view.state.settings.home.feature_style = "single".into();
                view.state.settings.confirm_queue_changes = false;
                view.state.settings.lyric_fetch = false;
                view.state.settings.scrobble = false;
                view.state.settings.play_button_behavior = feishin_core::queue::PlayBehavior::Last;
                let item = Item {
                    id: "featured".into(),
                    name: "Featured".into(),
                    cover_id: "cover".into(),
                    dominant_color: "#0000ff".into(),
                    column_data: [("explicitStatus".into(), "e".into())].into(),
                    ..Default::default()
                };
                view.covers.insert(
                    view.cover_cache_key(&item, Placement::ItemCard),
                    source.clone(),
                );
                view.home_sections.insert(
                    Kind::Feature,
                    Section {
                        items: vec![item],
                        ..Default::default()
                    },
                );
                cx.notify();
            })
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            let color = view
                .home_backgrounds
                .values()
                .next()
                .unwrap()
                .as_ref()
                .unwrap()
                .color
                .to_rgb();
            assert!((color.r - 96. / 255.).abs() < 0.001);
            assert!((color.g - 32. / 255.).abs() < 0.001);
            assert!((color.b - 16. / 255.).abs() < 0.001);
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.blur_explicit_images = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| view.home_backgrounds.is_empty()));
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                let item = &view.home_sections[&Kind::Feature].items[0];
                view.censored_covers.insert(
                    view.cover_cache_key(item, Placement::ItemCard),
                    censored.clone(),
                );
                cx.notify();
            })
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            let color = view
                .home_backgrounds
                .values()
                .next()
                .unwrap()
                .as_ref()
                .unwrap()
                .color
                .to_rgb();
            assert!((color.g - 100. / 255.).abs() < 0.001);
        });
        let cover = cx.debug_bounds("home-feature-cover-0").unwrap();
        let default_shadows = vec![
            gpui::BackdropFilterOperation::DropShadow {
                offset: [0., 6.],
                sigma: 20.,
                color: Some([0., 0., 0., 0.5]),
            },
            gpui::BackdropFilterOperation::DropShadow {
                offset: [0., 2.],
                sigma: 8.,
                color: Some([0., 0., 0., 0.4]),
            },
        ];
        assert!(
            cx.update(|window, _| window.painted_foreground_operations_for_test())
                .contains(&default_shadows)
        );
        let card = cx.debug_bounds("home-feature-card-0").unwrap();
        let background = cx.debug_bounds("home-feature-background-0").unwrap();
        assert_eq!(background, card);
        assert_eq!(
            cx.debug_bounds("home-feature-background-image-0").unwrap(),
            card
        );
        let filters = cx.update(|window, _| window.painted_foreground_filters_for_test());
        assert!(
            filters.iter().any(|(mask, sigma)| {
                *mask == card && (*sigma - px(48. * 1.1)).abs() < px(0.001)
            }),
            "Card {card:?}, foreground layers {filters:?}"
        );
        cx.update(|window, cx| {
            let asset = window
                .use_asset::<gpui::ImgResourceLoader>(&gpui::Resource::from(censored.clone()), cx);
            assert!(
                matches!(asset, Some(Ok(_))),
                "Background image resource was not ready: {asset:?}"
            );
            window.refresh();
        });
        cx.run_until_parked();
        let images = cx.update(|window, _| window.painted_images_for_test());
        assert!(
            images
                .iter()
                .any(|(bounds, corners, _)| *bounds == cover && corners.top_left == px(5.))
        );
        assert!(
            images.iter().any(|(bounds, _, opacity)| {
                (bounds.center().x - card.center().x).abs() <= px(0.5)
                    && (bounds.center().y - card.center().y).abs() <= px(0.5)
                    && (bounds.size.width - card.size.width * 1.1).abs() <= px(0.5)
                    && (*opacity - 0.8).abs() < 0.001
            }),
            "Card {card:?}, painted images {images:?}"
        );
        let dimmed = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, _| window.painted_lines_and_quads_for_test())
                .iter()
                .any(|(_, color, rounded, _)| {
                    *rounded && color.l == 0. && (color.a - 0.4).abs() < 0.001
                })
        };
        assert!(!dimmed(cx));
        cx.simulate_mouse_move(
            cover.origin + gpui::point(px(4.), px(4.)),
            None,
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(!dimmed(cx));
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(dimmed(cx));
        cx.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(!dimmed(cx));
        for (label, expected) in [
            ("Next", vec!["before", "featured", "after"]),
            ("Last", vec!["before", "after", "featured"]),
            ("Play", vec!["featured"]),
        ] {
            cx.update(|_, cx| {
                app.update(cx, |view, cx| {
                    view.state.queue = Default::default();
                    view.state.queue.add(
                        ["before", "after"]
                            .map(|id| Item {
                                id: id.into(),
                                ..Default::default()
                            })
                            .to_vec(),
                        Enqueue::End,
                    );
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| {
                    node.role() == Role::Button
                        && node
                            .label()
                            .is_some_and(|name| name.starts_with(&format!("{label}\n")))
                })
                .unwrap()
                .0;
            cx.update(|window, cx| {
                window.dispatch_accessibility_action_for_test(
                    ActionRequest {
                        action: Action::Click,
                        target_tree: TreeId::ROOT,
                        target_node: node,
                        data: None,
                    },
                    cx,
                )
            });
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                assert_eq!(view.route, Route::Home);
                assert_eq!(
                    view.state
                        .queue
                        .ordered()
                        .iter()
                        .map(|entry| entry.item.id.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
            });
        }
        let next = cx.debug_bounds("home-feature-next-0").unwrap().center();
        cx.simulate_mouse_move(next, None, Modifiers::none());
        cx.run_until_parked();
        assert!(
            cx.update(|window, _| window.painted_foreground_operations_for_test())
                .contains(&default_shadows)
        );
        cx.simulate_mouse_down(next, MouseButton::Left, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            2
        );
        cx.simulate_mouse_up(next, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            2
        );
        let card = cx.debug_bounds("home-feature-card-0").unwrap();
        cx.simulate_click(card.origin + gpui::point(px(4.), px(4.)), Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(matches!(view.route, Route::Detail(_)));
            assert!(view.home_backgrounds.is_empty());
        });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn feature_carousel_wheel_matches_original_callbacks(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use gpui::{AppContext, Modifiers, ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-home-wheel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(Default::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Home;
                view.state.settings.home.items.clear();
                view.state.settings.css.enabled = true;
                view.state.settings.css.content =
                    ".fs-main-content-module-main-content-container { max-width: 10000px; }".into();
                view.apply_theme(window, cx);
                cx.notify();
            })
        });
        cx.simulate_resize(gpui::size(px(1400.), px(800.)));
        cx.run_until_parked();
        let width = app.read_with(cx, |view, _| view.home_feature_width);
        cx.simulate_resize(gpui::size(px(1400.) + px(900.) - width, px(800.)));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.home_feature_width),
            px(900.)
        );
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/home-feature-carousel-reference.json"
        ))
        .unwrap();
        for style in ["single", "multiple"] {
            for count in [1, 2, 17] {
                cx.update(|_, cx| {
                    app.update(cx, |view, cx| {
                        view.state.settings.home.feature_style = style.into();
                        view.home_feature_wheel_at = None;
                        view.home_sections.insert(
                            Kind::Feature,
                            Section {
                                items: (0..count)
                                    .map(|index| Item {
                                        id: format!("album-{index}"),
                                        name: format!("Album {index}"),
                                        ..Default::default()
                                    })
                                    .collect(),
                                ..Default::default()
                            },
                        );
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let mut previous = 0;
                for case in reference["wheel"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|case| case["style"] == style && case["count"] == count)
                {
                    let milliseconds = case["milliseconds"].as_u64().unwrap();
                    cx.executor()
                        .advance_clock(Duration::from_millis(milliseconds - previous));
                    previous = milliseconds;
                    let position = cx.debug_bounds("home-carousel-0").unwrap().center();
                    cx.simulate_event(ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(
                            px(0.),
                            px(-case["delta"].as_f64().unwrap() as f32),
                        )),
                        modifiers: Modifiers {
                            shift: case["shift"].as_bool().unwrap(),
                            ..Modifiers::none()
                        },
                        touch_phase: TouchPhase::Moved,
                    });
                    cx.run_until_parked();
                    assert_eq!(
                        app.read_with(cx, |view, _| view.home_sections[&Kind::Feature].visible),
                        case["next"].as_u64().unwrap() as usize,
                        "{case}"
                    );
                }
                if count == 17 {
                    let start =
                        app.read_with(cx, |view, _| view.home_sections[&Kind::Feature].visible);
                    let step = app.read_with(cx, |view, _| view.home_step(Kind::Feature));
                    for (selector, expected) in [
                        ("home-feature-next-navigation-0", (start + step) % count),
                        ("home-feature-previous-navigation-0", start),
                    ] {
                        let position = cx.debug_bounds(selector).unwrap().center();
                        cx.simulate_mouse_move(position, None, Modifiers::none());
                        cx.run_until_parked();
                        cx.simulate_click(position, Modifiers::none());
                        cx.run_until_parked();
                        assert_eq!(
                            app.read_with(cx, |view, _| view.home_sections[&Kind::Feature].visible),
                            expected
                        );
                        assert_eq!(app.read_with(cx, |view, _| view.route.clone()), Route::Home);
                    }
                    cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
                    cx.run_until_parked();
                }
            }
        }
        let layouts: Vec<serde_json::Value> = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/home-feature-layout-linux-reference.json")
        } else {
            include_str!("../assets/home-feature-layout-reference.json")
        })
        .unwrap();
        for reference in layouts {
            let single = reference["single"].as_bool().unwrap();
            cx.update(|_, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.home.feature_style =
                        if single { "single" } else { "multiple" }.into();
                    view.home_sections.insert(
                        Kind::Feature,
                        Section {
                            items: (0..17)
                                .map(|index| Item {
                                    id: format!("layout-{index}"),
                                    name: "Album".into(),
                                    album_artist: "Artist".into(),
                                    album_artist_names: vec!["Artist".into()],
                                    ..Default::default()
                                })
                                .collect(),
                            ..Default::default()
                        },
                    );
                    cx.notify();
                })
            });
            let width = px(reference["width"].as_f64().unwrap() as f32);
            cx.simulate_resize(gpui::size(
                px(reference["viewport"].as_f64().unwrap() as f32),
                px(800.),
            ));
            cx.run_until_parked();
            let measured = cx.debug_bounds("home-carousel-0").unwrap();
            let viewport = cx.update(|window, _| window.viewport_size().width);
            cx.simulate_resize(gpui::size(viewport + width - measured.size.width, px(800.)));
            cx.run_until_parked();
            // This fixture compares settled CSS geometry, after responsive page transitions.
            cx.executor().advance_clock(Duration::from_millis(600));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let carousel = cx.debug_bounds("home-carousel-0").unwrap();
            assert_eq!(carousel.size.width, width);
            assert_eq!(
                app.read_with(cx, |view, _| view.home_feature_width),
                width,
                "Container measurement must include its padding"
            );
            for (original, native) in [
                ("carousel-item", "home-feature-card-0"),
                ("content", "home-feature-content-0"),
                ("title-section", "home-feature-title-0"),
                ("image-section", "home-feature-image-section-0"),
                ("album-image-container", "home-feature-cover-0"),
                ("nav-arrow-left", "home-feature-previous-navigation-0"),
                ("nav-arrow-right", "home-feature-next-navigation-0"),
                ("play-next", "home-feature-next-0"),
                ("play-now", "home-feature-now-0"),
                ("play-last", "home-feature-last-0"),
            ] {
                let expected = &reference["elements"][original];
                if expected.is_null() {
                    continue;
                }
                let actual = cx.debug_bounds(native);
                let actual = actual.unwrap_or_else(|| panic!("{native}: {reference}"));
                for (name, value) in [
                    ("x", actual.left() - carousel.left()),
                    ("y", actual.top() - carousel.top()),
                    ("width", actual.size.width),
                    ("height", actual.size.height),
                ] {
                    assert!(
                        // Taffy rounds both edges to physical pixels.
                        (f64::from(value) - expected[name].as_f64().unwrap()).abs() <= 1.,
                        "{native} {name}: {value:?}, bounds {actual:?}, parent {carousel:?}, original {expected}: {reference}"
                    );
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn sparse_artwork_pages_and_duplicates_do_not_skip_carousel_items() {
        let item = |id: &str, art: bool| Item {
            id: id.into(),
            cover_id: if art { id.into() } else { String::new() },
            ..Default::default()
        };
        let page = |items| Page {
            items,
            total: None,
            has_more: true,
        };
        let mut section = Section::default();
        section.append(
            page(vec![item("first", true), item("no-art", false)]),
            Kind::Feature,
        );
        assert_eq!(section.items.len(), 1);
        assert_eq!(section.offset, 2);
        section.pending_next = true;
        section.append(
            page(vec![item("first", true), item("no-art-2", false)]),
            Kind::Feature,
        );
        assert!(section.pending_next);
        assert_eq!(section.visible, 0);
        section.append(
            page(vec![item("second", true), item("third", true)]),
            Kind::Feature,
        );
        assert_eq!(section.visible, 1);
        assert!(!section.pending_next);
        assert_eq!(section.items[section.visible].id, "second");
        section.pending_next = true;
        section.append(
            Page {
                items: vec![],
                total: Some(6),
                has_more: false,
            },
            Kind::Feature,
        );
        assert!(!section.pending_next && !section.has_more);
        assert_eq!(section.visible, 1);
    }
}
