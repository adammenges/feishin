use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::{ArtistTab, ItemKind},
    releases,
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, StyledExt};

pub struct AlbumRow {
    pub group: Option<String>,
    pub indexes: Vec<usize>,
}
impl Feishin {
    pub fn album_group_rows(&self, columns: usize) -> Option<Vec<AlbumRow>> {
        let Route::Detail(artist) = &self.route else {
            return None;
        };
        if !matches!(artist.kind, ItemKind::Artist | ItemKind::AlbumArtist)
            || self.artist_tab != ArtistTab::Albums
        {
            return None;
        }
        let groups = releases::groups(
            &self.items,
            &artist.id,
            self.state.settings.artist_grouping == "all",
            &self.state.settings.artist_release_types,
        );
        Some(
            groups
                .into_iter()
                .flat_map(|(key, indexes)| {
                    indexes
                        .chunks(columns.max(1))
                        .enumerate()
                        .map(|(row, indexes)| AlbumRow {
                            group: (row == 0).then(|| releases::label(&key)),
                            indexes: indexes.to_vec(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )
    }
    pub fn album_group_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("album-group-settings")
            .v_flex()
            .size_full()
            .gap_3()
            .native_vertical_scroll()
            .child(crate::appearance::text("Artist overview sections", cx))
            .children(
                self.state
                    .settings
                    .artist_items
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                div().flex_1().child(
                                    self.tr(feishin_core::artist_sections::label(&field.id)),
                                ),
                            )
                            .child(
                                crate::styles::button(("artist-section-visible", index), cx)
                                    .label(if field.disabled { "Hidden" } else { "Visible" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let value =
                                            &mut this.state.settings.artist_items[index].disabled;
                                        *value = !*value;
                                        this.save();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("artist-section-up", index), cx)
                                    .label("↑")
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index > 0 {
                                            this.state.settings.artist_items.swap(index, index - 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                crate::styles::button(("artist-section-down", index), cx)
                                    .label("↓")
                                    .disabled(index + 1 == self.state.settings.artist_items.len())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index + 1 < this.state.settings.artist_items.len() {
                                            this.state.settings.artist_items.swap(index, index + 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                    }),
            )
            .child(crate::appearance::text("Album group metadata", cx))
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(crate::styles::number_input(&self.album_group_size, cx))
                    .child(
                        crate::styles::button("album-group-size", cx)
                            .label(self.tr("common.save"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let value = this.album_group_size.read(cx).value().parse::<f32>();
                                match value {
                                    Ok(size)
                                        if size.is_finite() && (0. ..=2000.).contains(&size) =>
                                    {
                                        this.state.settings.album_group.image_size = size;
                                        this.save();
                                    }
                                    _ => {
                                        this.message =
                                            "Enter an artwork size from 0 to 2000 pixels".into()
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
            .children(
                [
                    (true, "Vertical artwork layout"),
                    (false, "Album favorite/rating controls"),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, (vertical, label))| {
                    let checked = if vertical {
                        self.state.settings.album_group.vertical_layout
                    } else {
                        self.state.settings.album_group.show_favorite_rating
                    };
                    crate::styles::button(("album-group-option", i), cx)
                        .label(format!("{}: {}", self.tr(label), checked))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let value = if vertical {
                                &mut this.state.settings.album_group.vertical_layout
                            } else {
                                &mut this.state.settings.album_group.show_favorite_rating
                            };
                            *value = !*value;
                            this.save();
                            cx.notify();
                        }))
                }),
            )
            .children(
                self.state
                    .settings
                    .album_group
                    .items
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let label = feishin_core::album_metadata::FIELDS
                            .iter()
                            .find(|(id, _)| *id == field.id)
                            .map_or(field.id.as_str(), |(_, label)| *label);
                        div()
                            .h_flex()
                            .gap_2()
                            .child(div().flex_1().child(self.tr(label)))
                            .child(
                                crate::styles::button(("album-metadata-visible", index), cx)
                                    .label(if field.disabled { "Hidden" } else { "Visible" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let value = &mut this.state.settings.album_group.items
                                            [index]
                                            .disabled;
                                        *value = !*value;
                                        this.save();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("album-metadata-up", index), cx)
                                    .label("↑")
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index > 0 {
                                            this.state
                                                .settings
                                                .album_group
                                                .items
                                                .swap(index, index - 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                crate::styles::button(("album-metadata-down", index), cx)
                                    .label("↓")
                                    .disabled(
                                        index + 1 == self.state.settings.album_group.items.len(),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index + 1 < this.state.settings.album_group.items.len() {
                                            this.state
                                                .settings
                                                .album_group
                                                .items
                                                .swap(index, index + 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                    }),
            )
            .child(
                crate::styles::button("album-grouping", cx)
                    .label(format!("Grouping: {}", self.state.settings.artist_grouping))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.artist_grouping =
                            if this.state.settings.artist_grouping == "primary" {
                                "all"
                            } else {
                                "primary"
                            }
                            .into();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(crate::appearance::text(
                "Order groups and choose which release types to show",
                cx,
            ))
            .children(
                self.state
                    .settings
                    .artist_release_types
                    .iter()
                    .enumerate()
                    .map(|(index, setting)| {
                        let id = setting.id.clone();
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_3()
                            .items_center()
                            .p_2()
                            .bg(cx.theme().secondary)
                            .rounded_md()
                            .child(div().flex_1().child(releases::label(&id)))
                            .child(
                                crate::styles::button(("release-visible", index), cx)
                                    .label(if setting.disabled {
                                        "Hidden"
                                    } else {
                                        "Visible"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(setting) = this
                                            .state
                                            .settings
                                            .artist_release_types
                                            .iter_mut()
                                            .find(|setting| setting.id == id)
                                        {
                                            setting.disabled = !setting.disabled;
                                        }
                                        this.save();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("release-up", index), cx)
                                    .label(crate::appearance::text("↑", cx))
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index > 0
                                            && index
                                                < this.state.settings.artist_release_types.len()
                                        {
                                            this.state
                                                .settings
                                                .artist_release_types
                                                .swap(index, index - 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                crate::styles::button(("release-down", index), cx)
                                    .label(crate::appearance::text("↓", cx))
                                    .disabled(
                                        index + 1 == self.state.settings.artist_release_types.len(),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index + 1
                                            < this.state.settings.artist_release_types.len()
                                        {
                                            this.state
                                                .settings
                                                .artist_release_types
                                                .swap(index, index + 1);
                                            this.save();
                                            cx.notify();
                                        }
                                    })),
                            )
                    }),
            )
            .child(
                crate::styles::button("release-reset", cx)
                    .label(crate::appearance::text("Restore group defaults", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.artist_release_types = releases::defaults();
                        this.save();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
