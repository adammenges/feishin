use crate::app::{Feishin, Route};
use feishin_core::{
    artists, genres,
    model::{Item, ItemKind},
    styles::Target,
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt};

impl Feishin {
    pub(crate) fn genre_metadata(
        &self,
        item: &Item,
        badges: bool,
        detail: bool,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let genres = genres::links(item);
        if !badges {
            return div()
                .text_color(cx.theme().muted_foreground)
                .child(self.linked_metadata(
                    artists::listed(genres, ", "),
                    item.server_id,
                    ItemKind::Genre,
                    id,
                    cx,
                ))
                .into_any_element();
        }
        let rem = cx.theme().font_size;
        let border = crate::styles::css_border_width(rem / 16., cx);
        div()
            .id(id)
            .debug_selector(move || id.into())
            .h_flex()
            .when(!detail, |div| div.flex_wrap())
            .gap_x(rems(0.25))
            .gap_y(rems(0.5))
            .py(rems(0.25))
            .overflow_hidden()
            .when(crate::appearance::rtl(cx), |div| {
                div.flex_row_reverse().justify_end()
            })
            .children(
                genres
                    .into_iter()
                    .take(4)
                    .enumerate()
                    .map(|(index, (genre_id, name))| {
                        let (color, light) = genres::badge_color(&name);
                        let background = rgb((u32::from(color[0]) << 16)
                            | (u32::from(color[1]) << 8)
                            | u32::from(color[2]));
                        let target = Item {
                            id: genre_id,
                            name: name.clone(),
                            kind: ItemKind::Genre,
                            server_id: item.server_id,
                            ..Default::default()
                        };
                        let key_target = target.clone();
                        let mut style_target =
                            Target::new("a", "", &["mantine-Badge-root", "fs-badge-module-root"]);
                        style_target
                            .attributes
                            .insert("data-variant".into(), "default".into());
                        style_target.inline_declarations.extend([
                            (
                                "background-color".into(),
                                format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
                            ),
                            ("color".into(), if light { "black" } else { "white" }.into()),
                        ]);
                        crate::styles::apply_in(
                            gpui::div()
                                .id(("genre-badge", index))
                                .debug_selector(move || format!("genre-badge-{index}"))
                                .role(Role::Link)
                                .aria_label(name.clone())
                                .focusable()
                                .tab_stop(true)
                                .cursor_pointer()
                                .grid()
                                .grid_cols(1)
                                .h(rem * 1.25)
                                .px(rems(0.5))
                                .py(rems(0.25))
                                .rounded(rems(5. / 16.))
                                .border(border)
                                .border_color(transparent_black())
                                .min_w(rem + border * 2.)
                                .max_w_full()
                                .overflow_hidden()
                                .justify_center()
                                .items_center()
                                .bg(background)
                                .text_color(if light { black() } else { white() })
                                .text_size(rem * 11. / 16.)
                                .line_height(rem * 18. / 16.)
                                .font_weight(FontWeight::MEDIUM)
                                .letter_spacing(rem / 64.)
                                .child(div().min_w_0().truncate().child(name.to_uppercase()))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.navigate(Route::Detail(Box::new(target.clone())), cx);
                                    cx.stop_propagation();
                                }))
                                .on_key_down(cx.listener(
                                    move |view, event: &KeyDownEvent, _, cx| {
                                        if event.keystroke.key == "enter" {
                                            view.navigate(
                                                Route::Detail(Box::new(key_target.clone())),
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                            style_target,
                            [Target::new(
                                "div",
                                "",
                                &["fs-genre-badge-column-module-group"],
                            )],
                            cx,
                        )
                    }),
            )
            .into_any_element()
    }
}
