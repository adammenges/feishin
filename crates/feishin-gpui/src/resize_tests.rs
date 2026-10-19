use super::{Feishin, NativeStyles, Target};
use crate::app::Route;
use feishin_core::{
    appearance,
    model::ItemKind,
    storage::State,
    styles::{MediaEnvironment, Stylesheet},
};
use gpui::{AppContext, TestAppContext, px, size};
use gpui_component::Root;
use std::{collections::HashSet, time::Instant};

#[test]
fn inherited_variable_cache_shares_parents_and_refreshes_media_rules() {
    let mut styles = NativeStyles::new(
        Stylesheet::parse([r#"
            body { --base: 10px; --alias: var(--base); }
            .parent { --base: 20px; }
            .parent:hover { --base: 30px; }
            .leaf { width: var(--alias); height: var(--base); }
            @media (width <= 800px) { body { --base: 8px; } }
        "#])
        .unwrap(),
        appearance::Palette::default(),
    );
    let mut parent = Target::new("div", "parent", &["parent"]);
    for index in 0..40 {
        let leaf = Target::new("div", &format!("leaf-{index}"), &["leaf"]);
        let style = styles.style(leaf, Some(parent.clone()), 16.);
        assert_eq!(style.size.width, Some(px(10.).into()));
        assert_eq!(style.size.height, Some(px(20.).into()));
    }
    assert_eq!(
        styles.variable_cache.borrow().len(),
        2,
        "Resolve the root and parent once, not once per leaf"
    );
    let root = styles
        .variable_cache
        .borrow()
        .get(&styles.context)
        .unwrap()
        .clone();
    parent.states.push("hover".into());
    let leaf = Target::new("div", "hover-leaf", &["leaf"]);
    let style = styles.style(leaf.clone(), Some(parent.clone()), 16.);
    assert_eq!(style.size.width, Some(px(10.).into()));
    assert_eq!(style.size.height, Some(px(30.).into()));
    assert!(std::sync::Arc::ptr_eq(
        &root,
        styles.variable_cache.borrow().get(&styles.context).unwrap()
    ));
    styles.update_environment(
        MediaEnvironment {
            width: 800.,
            ..Default::default()
        },
        1.,
    );
    assert!(styles.variable_cache.borrow().is_empty());
    assert_eq!(
        styles.style(leaf, Some(parent), 16.).size.width,
        Some(px(8.).into())
    );
    styles = styles.with_variables(Default::default());
    assert!(styles.variable_cache.borrow().is_empty());
}

#[test]
fn resize_cache_preserves_static_styles_and_updates_responsive_values() {
    let mut styles = NativeStyles::new(
        Stylesheet::parse([r#"
            body { --gap: calc(2vw + 1px); }
            .fixed { width: 42px; transition: opacity 120ms; }
            .fluid { width: var(--gap); height: 10dvh; }
            @media (width <= 800px) {
                .fixed { width: 24px; transition: opacity 20ms; }
            }
            @media (prefers-color-scheme: light) { .fixed { height: 18px; } }
            @media (min-resolution: 2dppx) { .fixed { padding-left: 2px; } }
        "#])
        .unwrap(),
        appearance::Palette::default(),
    );
    let mut media = MediaEnvironment {
        width: 1000.,
        height: 800.,
        ..Default::default()
    };
    styles.update_environment(media, 1.);
    let fixed = Target::new("button", "fixed", &["fixed"]);
    let fluid = Target::new("div", "fluid", &["fluid"]);
    let mut inline = Target::new("div", "inline", &[]);
    inline
        .inline_declarations
        .push(("width".into(), "3vw".into()));
    assert_eq!(
        styles.style(fixed.clone(), None, 16.).size.width,
        Some(px(42.).into())
    );
    assert_eq!(
        styles.style(fluid.clone(), None, 16.).size.width,
        Some(px(21.).into())
    );
    assert_eq!(
        styles.style(inline.clone(), None, 16.).size.width,
        Some(px(30.).into())
    );
    assert!(
        (styles.button_motion(fixed.clone()).timing[3]
            .duration
            .as_secs_f32()
            - 0.12)
            .abs()
            < 1e-6
    );
    assert_eq!(styles.cache.borrow().len(), 3);

    media.width = 1100.;
    media.height = 900.;
    styles.update_environment(media, 1.);
    assert_eq!(
        styles.cache.borrow().len(),
        1,
        "Only viewport-dependent styles need recomputing"
    );
    assert_eq!(styles.motion_cache.borrow().len(), 1);
    assert_eq!(
        styles.style(fixed.clone(), None, 16.).size.width,
        Some(px(42.).into())
    );
    let resized = styles.style(fluid, None, 16.);
    assert_eq!(resized.size.width, Some(px(23.).into()));
    assert_eq!(resized.size.height, Some(px(90.).into()));
    assert_eq!(
        styles.style(inline, None, 16.).size.width,
        Some(px(33.).into())
    );

    media.width = 800.;
    styles.update_environment(media, 1.);
    assert!(styles.cache.borrow().is_empty());
    assert!(styles.motion_cache.borrow().is_empty());
    assert_eq!(
        styles.style(fixed.clone(), None, 16.).size.width,
        Some(px(24.).into())
    );
    assert!(
        (styles.button_motion(fixed.clone()).timing[3]
            .duration
            .as_secs_f32()
            - 0.02)
            .abs()
            < 1e-6
    );

    media.dark = false;
    styles.update_environment(media, 1.);
    assert!(styles.cache.borrow().is_empty());
    assert_eq!(
        styles.style(fixed.clone(), None, 16.).size.height,
        Some(px(18.).into())
    );
    media.resolution = 2.;
    styles.update_environment(media, 1.);
    assert!(styles.cache.borrow().is_empty());
    assert_eq!(
        styles.style(fixed.clone(), None, 16.).padding.left,
        Some(px(2.).into())
    );
    styles.update_environment(media, 2.);
    assert!(
        styles.cache.borrow().is_empty(),
        "Display changes must recompute pixel-snapped borders"
    );

    let mut bounded = Target::new("div", "bounded", &[]);
    bounded
        .inline_declarations
        .push(("width".into(), "1000vw".into()));
    media.width = 1100.;
    styles.update_environment(media, 2.);
    assert_eq!(styles.style(bounded.clone(), None, 16.).size.width, None);
    media.width = 500.;
    styles.update_environment(media, 2.);
    assert_eq!(
        styles.style(bounded, None, 16.).size.width,
        Some(px(5000.).into())
    );
}

#[gpui::test]
fn window_resize_reuses_resolved_styles(cx: &mut TestAppContext) {
    let started = Instant::now();
    cx.use_platform_text_system();
    cx.set_assets(crate::icons::Assets);
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::fonts::register_builtins(cx).unwrap();
    });
    eprintln!(
        "Builtin font registration: {:.2} ms",
        started.elapsed().as_secs_f64() * 1000.
    );
    let dir = std::env::temp_dir().join(format!("feishin-resize-{}", uuid::Uuid::new_v4()));
    let mut app = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut state = State::default();
        state.settings.home.feature = false;
        state.settings.home.items.clear();
        state.settings.sidebar.playlist_list = false;
        let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        app = Some(view.clone());
        Root::new(view, window, cx)
    });
    let app = app.unwrap();
    let mut lost = vec![];
    for (name, route) in [
        ("library", Route::Library(ItemKind::Album)),
        ("settings", Route::Settings),
    ] {
        let started = Instant::now();
        app.update(cx, |view, cx| {
            view.route = route;
            view.items = (0..40)
                .map(|index| feishin_core::model::Item {
                    id: index.to_string(),
                    name: format!("Album {index}"),
                    album_artist: "Fixture Artist".into(),
                    kind: ItemKind::Album,
                    ..Default::default()
                })
                .collect();
            cx.notify();
        });
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        eprintln!(
            "Open {name}: {:.2} ms",
            started.elapsed().as_secs_f64() * 1000.
        );
        let keys = cx.update(|_, cx| {
            cx.global::<NativeStyles>()
                .cache
                .borrow()
                .keys()
                .cloned()
                .collect::<HashSet<_>>()
        });
        assert!(!keys.is_empty());
        let mut frames = vec![];
        for frame in 1..=8 {
            let start = Instant::now();
            cx.simulate_resize(size(px(1320. + frame as f32 * 7.), px(840.)));
            cx.run_until_parked();
            frames.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let retained = cx.update(|_, cx| {
            let cache = cx.global::<NativeStyles>().cache.borrow();
            keys.iter().filter(|key| cache.contains_key(*key)).count()
        });
        frames.sort_by(f64::total_cmp);
        eprintln!(
            "Resize {name}: median {:.2} ms, max {:.2} ms; retained {retained}/{} styles",
            frames[frames.len() / 2],
            frames.last().unwrap(),
            keys.len()
        );
        if retained != keys.len() {
            lost.push(name);
        }
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.window.as_ref().unwrap().width, 1376.);
        });
    }
    std::fs::remove_dir_all(dir).ok();
    assert!(
        lost.is_empty(),
        "Resize discarded static styles in {lost:?}"
    );
}
