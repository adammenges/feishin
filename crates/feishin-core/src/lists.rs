use crate::model::{Item, duration_label};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::OnceLock,
};

pub const COLUMNS: &[(&str, &str)] = &[
    ("name", "Title"),
    ("titleArtist", "Title and artist"),
    ("titleCombined", "Title and album"),
    ("artists", "Artists"),
    ("albumArtists", "Album artists"),
    ("album", "Album"),
    ("trackNumber", "Track"),
    ("discNumber", "Disc"),
    ("duration", "Duration"),
    ("year", "Year"),
    ("date", "Date"),
    ("releaseYear", "Release year"),
    ("releaseDate", "Release date"),
    ("genres", "Genres"),
    ("genreBadge", "Genre badges"),
    ("userFavorite", "Favorite"),
    ("userRating", "Rating"),
    ("playCount", "Play count"),
    ("lastPlayedAt", "Last played"),
    ("createdAt", "Added"),
    ("imageUrl", "Artwork"),
    ("rowIndex", "Index"),
    ("albumCount", "Albums"),
    ("songCount", "Songs"),
    ("albumGroup", "Album group"),
    ("releaseType", "Release type"),
    ("username", "Owner"),
    ("comment", "Comment"),
    ("biography", "Biography"),
    ("composer", "Composer"),
    ("bpm", "BPM"),
    ("bitRate", "Bitrate"),
    ("bitDepth", "Bit depth"),
    ("sampleRate", "Sample rate"),
    ("channels", "Channels"),
    ("container", "Codec"),
    ("size", "File size"),
    ("path", "Path"),
    ("id", "ID"),
    ("actions", "Actions"),
    ("playlistReorder", "Reorder"),
    ("skip", "Skip"),
    ("__layoutFill", "Space"),
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Compact,
    #[default]
    Default,
    Large,
}
impl Size {
    pub fn row_height(self) -> f32 {
        match self {
            Self::Compact => 40.,
            Self::Default => 66.,
            Self::Large => 88.,
        }
    }
    pub fn detail_row_height(self) -> f32 {
        match self {
            Self::Compact => 32.,
            Self::Default => 40.,
            Self::Large => 48.,
        }
    }
    pub fn card_height(self) -> f32 {
        match self {
            Self::Compact => 210.,
            Self::Default => 260.,
            Self::Large => 340.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Column {
    pub id: String,
    pub is_enabled: bool,
    pub width: f32,
    pub align: Align,
    pub pinned: Option<String>,
    pub auto_size: bool,
}
impl Default for Column {
    fn default() -> Self {
        Self {
            id: "name".into(),
            is_enabled: true,
            width: 180.,
            align: Align::Start,
            pinned: None,
            auto_size: false,
        }
    }
}

/// CSS-pixel widths from the original table model. Zoom is applied at rendering.
pub fn layout_columns(
    columns: &[Column],
    auto_fit: bool,
    total_width: f64,
    center_width: f64,
) -> Vec<Column> {
    let mut columns = columns
        .iter()
        .filter(|column| column.is_enabled)
        .cloned()
        .collect::<Vec<_>>();
    if !auto_fit
        && columns.iter().any(|column| column.pinned.is_none())
        && !columns
            .iter()
            .any(|column| column.pinned.is_none() && column.auto_size)
    {
        columns.push(Column {
            id: "__layoutFill".into(),
            width: 0.,
            auto_size: true,
            ..Default::default()
        });
    }
    columns.sort_by_key(|column| match column.pinned.as_deref() {
        Some("left") => 0,
        Some("right") => 2,
        _ => 1,
    });
    let unpinned = columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| column.pinned.is_none().then_some(index))
        .collect::<Vec<_>>();
    let base = columns
        .iter()
        .map(|column| f64::from(column.width))
        .collect::<Vec<_>>();
    let mut widths = base
        .iter()
        .map(|width| (width + 0.5).floor())
        .collect::<Vec<_>>();
    let unpinned_total = unpinned.iter().map(|&index| base[index]).sum::<f64>();
    if auto_fit {
        let pinned = columns
            .iter()
            .filter(|column| column.pinned.is_some())
            .map(|column| f64::from(column.width))
            .sum::<f64>();
        let available = total_width - pinned;
        if unpinned_total != 0. && available > 0. {
            let scale = available / unpinned_total;
            for &index in &unpinned {
                widths[index] = (base[index] * scale + 0.5).floor();
            }
            let difference = total_width - widths.iter().sum::<f64>();
            let mut largest = unpinned.clone();
            largest.sort_by(|&a, &b| widths[b].total_cmp(&widths[a]));
            for index in largest.into_iter().take(difference.abs().ceil() as usize) {
                widths[index] += difference.signum();
            }
        }
    } else {
        let auto = unpinned
            .into_iter()
            .filter(|&index| columns[index].auto_size)
            .collect::<Vec<_>>();
        let extra = (center_width - unpinned_total).max(0.);
        if !auto.is_empty() && extra > 0. {
            let extra_per = extra / auto.len() as f64;
            for index in auto {
                widths[index] = (base[index] + extra_per + 0.5).floor();
            }
        }
    }
    for (column, width) in columns.iter_mut().zip(widths) {
        column.width = width as f32;
    }
    columns
}

pub fn detail_fixed_width(id: &str) -> Option<f32> {
    Some(match id {
        "actions" | "userFavorite" => 32.,
        "bitDepth" | "bitRate" => 88.,
        "bpm" | "year" => 56.,
        "channels" | "container" => 80.,
        "date" | "createdAt" | "releaseDate" | "releaseYear" => 128.,
        "discNumber" => 36.,
        "duration" => 72.,
        "sampleRate" => 112.,
        "trackNumber" | "userRating" => 64.,
        _ => return None,
    })
}

pub fn resized_column_width(start: f32, delta: f32) -> f32 {
    (start + delta).clamp(10., 1000.)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Table {
    pub columns: Vec<Column>,
    pub size: Size,
    pub auto_fit_columns: bool,
    pub enable_header: bool,
    pub enable_alternate_row_colors: bool,
    pub enable_horizontal_borders: bool,
    pub enable_vertical_borders: bool,
    pub enable_row_hover_highlight: bool,
}
impl Default for Table {
    fn default() -> Self {
        Self {
            columns: default_columns(),
            size: Size::Default,
            auto_fit_columns: true,
            enable_header: true,
            enable_alternate_row_colors: false,
            enable_horizontal_borders: true,
            enable_vertical_borders: false,
            enable_row_hover_highlight: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GridRow {
    pub id: String,
    pub is_enabled: bool,
    pub align: Align,
}
impl Default for GridRow {
    fn default() -> Self {
        Self {
            id: "name".into(),
            is_enabled: true,
            align: Align::Start,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grid {
    pub item_gap: String,
    pub items_per_row: usize,
    pub items_per_row_enabled: bool,
    pub rows: Vec<GridRow>,
    pub size: Size,
}
impl Default for Grid {
    fn default() -> Self {
        Self {
            item_gap: "md".into(),
            items_per_row: 4,
            items_per_row_enabled: false,
            rows: ["name", "artists", "year"]
                .into_iter()
                .map(|id| GridRow {
                    id: id.into(),
                    ..Default::default()
                })
                .collect(),
            size: Size::Default,
        }
    }
}
impl Grid {
    pub fn height(&self) -> f32 {
        self.size
            .card_height()
            .max(self.rows.iter().filter(|row| row.is_enabled).count() as f32 * 20. + 140.)
    }
    pub fn artwork_height(&self) -> f32 {
        self.height() - self.rows.iter().filter(|row| row.is_enabled).count() as f32 * 20. - 60.
    }
    pub fn gap(&self) -> f32 {
        match self.item_gap.as_str() {
            "xs" => 4.,
            "sm" => 8.,
            "lg" => 20.,
            "xl" => 28.,
            _ => 12.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub display: String,
    pub items_per_page: usize,
    pub pagination: String,
    pub table: Table,
    pub grid: Grid,
    pub detail: Option<Table>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            display: "table".into(),
            items_per_page: 100,
            pagination: "infinite".into(),
            table: Table::default(),
            grid: Grid::default(),
            detail: None,
        }
    }
}
pub fn default_columns() -> Vec<Column> {
    COLUMNS
        .iter()
        .map(|(id, _)| Column {
            id: (*id).into(),
            is_enabled: matches!(*id, "name" | "artists" | "album" | "duration" | "actions"),
            width: match *id {
                "name" => 280.,
                "duration" | "year" | "trackNumber" | "discNumber" => 80.,
                "imageUrl" => 60.,
                "actions" => 320.,
                _ => 180.,
            },
            ..Default::default()
        })
        .collect()
}
fn base_key(key: &str) -> &str {
    match key {
        "albumArtistAlbum" | "genreAlbum" => "album",
        "albumArtistSong"
        | "albumArtistFavoriteSong"
        | "albumArtistTopSong"
        | "albumArtistCommunitySong"
        | "genreSong" => "song",
        "albumArtistSimilar" => "albumArtist",
        "fullScreenSong" => "fullScreen",
        _ => key,
    }
}

pub fn configured(configs: &BTreeMap<String, Config>, key: &str) -> Config {
    configs
        .get(key)
        .or_else(|| configs.get(base_key(key)))
        .cloned()
        .unwrap_or_else(|| default_for(key))
}

pub fn default_for(key: &str) -> Config {
    static DEFAULTS: OnceLock<BTreeMap<String, Config>> = OnceLock::new();
    let defaults = DEFAULTS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Original {
            lists: BTreeMap<String, Config>,
        }
        serde_json::from_str::<Original>(include_str!("../assets/electron-default-settings.json"))
            .expect("embedded original list defaults")
            .lists
    });
    if let Some(config) = defaults.get(base_key(key)) {
        return config.clone();
    }
    let mut config = Config::default();
    if key == "folder" {
        for column in &mut config.table.columns {
            column.is_enabled = matches!(
                column.id.as_str(),
                "name" | "artists" | "duration" | "actions"
            );
        }
    }
    config
}
pub fn label(id: &str) -> &str {
    COLUMNS
        .iter()
        .find(|(key, _)| *key == id)
        .map_or(id, |(_, label)| label)
}
pub fn validate(configs: &BTreeMap<String, Config>) -> Result<()> {
    ensure!(configs.len() <= 100, "Too many list configurations");
    for (key, config) in configs {
        ensure!(
            !key.is_empty() && key.len() <= 100,
            "Invalid list configuration key"
        );
        ensure!(
            matches!(
                config.display.as_str(),
                "table" | "poster" | "detail" | "list"
            ),
            "Invalid list display"
        );
        ensure!(
            (1..=500).contains(&config.items_per_page)
                && matches!(config.pagination.as_str(), "infinite" | "paginated"),
            "Invalid list pagination"
        );
        ensure!(
            (1..=24).contains(&config.grid.items_per_row)
                && matches!(
                    config.grid.item_gap.as_str(),
                    "xs" | "sm" | "md" | "lg" | "xl"
                ),
            "Invalid grid dimensions"
        );
        for table in std::iter::once(&config.table).chain(config.detail.iter()) {
            let mut ids = HashSet::new();
            ensure!(
                table.columns.len() <= COLUMNS.len(),
                "Too many list columns"
            );
            for column in &table.columns {
                ensure!(
                    COLUMNS.iter().any(|(id, _)| *id == column.id) && ids.insert(&column.id),
                    "Invalid or duplicate list column"
                );
                ensure!(
                    column.width.is_finite() && (0. ..=2400.).contains(&column.width),
                    "Invalid column width"
                );
                ensure!(
                    column
                        .pinned
                        .as_ref()
                        .is_none_or(|side| matches!(side.as_str(), "left" | "right")),
                    "Invalid pinned column"
                );
            }
        }
        let mut ids = HashSet::new();
        for row in &config.grid.rows {
            ensure!(
                COLUMNS.iter().any(|(id, _)| *id == row.id) && ids.insert(&row.id),
                "Invalid or duplicate card row"
            );
        }
    }
    Ok(())
}
pub fn text(item: &Item, id: &str, index: usize) -> String {
    let number = |n: u32| if n == 0 { String::new() } else { n.to_string() };
    match id {
        "name" => item.name.clone(),
        "titleArtist" => format!("{} · {}", item.name, item.artist),
        "titleCombined" => format!("{} · {}", item.name, item.album),
        "artists" => item.artist.clone(),
        "albumArtists" => item.album_artist.clone(),
        "album" => item.album.clone(),
        "duration" => {
            if item.duration > 0. {
                duration_label(item.duration)
            } else {
                String::new()
            }
        }
        "trackNumber" => number(item.track),
        "discNumber" => number(item.disc),
        "year" => number(item.year),
        "genres" | "genreBadge" => {
            if item.genre_links.is_some() || !item.genre_names.is_empty() {
                crate::genres::links(item)
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                item.genre.clone()
            }
        }
        "userFavorite" => {
            if item.favorite {
                "♥".into()
            } else {
                String::new()
            }
        }
        "userRating" => "★".repeat(item.rating as usize),
        "playCount" => number(item.play_count),
        "rowIndex" => (index + 1).to_string(),
        "albumGroup" => item.album.clone(),
        "releaseType" => item.release_types.join(" / "),
        "username" => item.owner_name.clone(),
        "comment" => item.comment.clone(),
        "container" => item.container.clone(),
        "path" => item.path.clone(),
        "id" => item.id.clone(),
        "sortName" => item
            .column_data
            .get(id)
            .cloned()
            .unwrap_or_else(|| item.name.clone()),
        "releaseYear" => item
            .column_data
            .get(id)
            .cloned()
            .unwrap_or_else(|| number(item.year)),
        "public" => item.public.to_string(),
        "bitRate" | "sampleRate" | "bitDepth" | "size" => {
            let raw = item.column_data.get(id).cloned().unwrap_or_default();
            if let Ok(number) = raw.parse::<f64>() {
                match id {
                    "bitRate" => format!("{number:.0} kbps"),
                    "sampleRate" => format!("{} kHz", number / 1000.),
                    "bitDepth" => format!("{number:.0} bit"),
                    _ => {
                        let mut value = number;
                        let mut unit = 0;
                        let units = ["B", "KiB", "MiB", "GiB", "TiB"];
                        while value >= 1024. && unit < 4 {
                            value /= 1024.;
                            unit += 1;
                        }
                        format!("{value:.1} {}", units[unit])
                    }
                }
            } else {
                raw
            }
        }
        _ => item.column_data.get(id).cloned().unwrap_or_default(),
    }
}
pub const SONG_SORTS: [(crate::model::Sort, &str); 19] = [
    (crate::model::Sort::Id, "filter.id"),
    (crate::model::Sort::Album, "filter.album"),
    (crate::model::Sort::AlbumArtist, "filter.albumArtist"),
    (crate::model::Sort::Artist, "filter.artist"),
    (crate::model::Sort::Bpm, "filter.bpm"),
    (crate::model::Sort::Channels, "common.channel"),
    (crate::model::Sort::Comment, "filter.comment"),
    (crate::model::Sort::Duration, "filter.duration"),
    (crate::model::Sort::Favorited, "filter.isFavorited"),
    (crate::model::Sort::Genre, "filter.genre"),
    (crate::model::Sort::Name, "filter.name"),
    (crate::model::Sort::Random, "filter.random"),
    (crate::model::Sort::SortName, "filter.sortName"),
    (crate::model::Sort::Frequent, "filter.playCount"),
    (crate::model::Sort::Rating, "filter.rating"),
    (crate::model::Sort::Newest, "filter.recentlyAdded"),
    (crate::model::Sort::Recent, "filter.recentlyPlayed"),
    (crate::model::Sort::ReleaseYear, "filter.releaseYear"),
    (crate::model::Sort::Year, "filter.year"),
];

pub const ALBUM_SORTS: [(crate::model::Sort, &str); 14] = [
    (crate::model::Sort::AlbumArtist, "filter.albumArtist"),
    (crate::model::Sort::Id, "filter.id"),
    (crate::model::Sort::Duration, "filter.duration"),
    (crate::model::Sort::Favorited, "filter.favorited"),
    (crate::model::Sort::Name, "filter.name"),
    (crate::model::Sort::SortName, "filter.sortName"),
    (crate::model::Sort::Frequent, "filter.playCount"),
    (crate::model::Sort::Random, "filter.random"),
    (crate::model::Sort::Rating, "filter.rating"),
    (crate::model::Sort::Newest, "filter.recentlyAdded"),
    (crate::model::Sort::Recent, "filter.recentlyPlayed"),
    (crate::model::Sort::ReleaseDate, "filter.releaseDate"),
    (crate::model::Sort::Year, "filter.releaseYear"),
    (crate::model::Sort::SongCount, "filter.songCount"),
];

pub fn search_albums(items: &[Item], query: &str) -> Vec<usize> {
    search_items(items, query, crate::model::ItemKind::Album)
}

/// The layout editor highlights localized matches without removing or sorting rows.
pub fn config_search_matches(query: &str, label: &str) -> bool {
    !query.trim().is_empty()
        && crate::lyric_match::search_score(query, label, 1., 0.3, false).is_some()
}

/// Return original list indices in Fuse relevance order, preserving duplicate entries.
pub fn search_items<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    query: &str,
    kind: crate::model::ItemKind,
) -> Vec<usize> {
    use crate::model::ItemKind;
    let mut results = items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if query.trim().is_empty() {
                return Some((index, 1.));
            }
            let join = |links: Vec<(String, String)>| {
                links
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let artists = join(crate::artists::links(item, false));
            let album_artists = join(crate::artists::links(item, true));
            let genres = join(crate::genres::links(item));
            let fields: Vec<&str> = match kind {
                ItemKind::Album => vec![
                    item.name.as_str(),
                    item.column_data
                        .get("releaseType")
                        .map_or("", String::as_str),
                    &artists,
                    &album_artists,
                    &genres,
                ],
                ItemKind::Song => vec![&item.album, &item.name, &artists, &album_artists],
                ItemKind::AlbumArtist | ItemKind::Playlist => vec![&item.name, &genres],
                _ => vec![item.name.as_str()],
            };
            let weight = 1. / fields.len() as f64;
            let scores = fields
                .into_iter()
                .filter_map(|field| {
                    crate::lyric_match::search_score(query, field, weight, 0.3, true)
                })
                .collect::<Vec<_>>();
            (!scores.is_empty()).then(|| (index, scores.into_iter().product::<f64>()))
        })
        .collect::<Vec<_>>();
    results.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    results.into_iter().map(|(index, _)| index).collect()
}

/// Apply client-side search before paging or display sorting.
pub fn search_list(items: Vec<Item>, query: &str, kind: crate::model::ItemKind) -> Vec<Item> {
    if query.trim().is_empty() {
        return items;
    }
    search_items(&items, query, kind)
        .into_iter()
        .map(|index| items[index].clone())
        .collect()
}

pub fn radio_items(
    items: Vec<Item>,
    query: &str,
    sort: crate::model::Sort,
    descending: bool,
) -> Vec<Item> {
    let mut items = search_list(items, query, crate::model::ItemKind::Radio);
    if sort == crate::model::Sort::Id {
        if descending {
            items.reverse();
        }
    } else {
        sort_items(&mut items, crate::model::Sort::Name, descending);
    }
    items
}

/// The original client-side album sorter differs from server sorting and song sorting.
pub fn compare_albums(
    a: &Item,
    b: &Item,
    sort: crate::model::Sort,
    descending: bool,
) -> std::cmp::Ordering {
    use crate::model::Sort;
    use std::cmp::Ordering;
    fn optional<T: PartialOrd>(a: Option<T>, b: Option<T>) -> Ordering {
        a.is_none()
            .cmp(&b.is_none())
            .then_with(|| a.partial_cmp(&b).unwrap_or(Ordering::Equal))
    }
    fn release(item: &Item, date: &str, year: &str) -> Option<i64> {
        use chrono::TimeZone;
        if let Some(value) = item.column_data.get(date).filter(|value| !value.is_empty()) {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(value) {
                return Some(date.timestamp_millis());
            }
            let iso = match value.len() {
                4 => format!("{value}-01-01"),
                7 => format!("{value}-01"),
                _ => value.clone(),
            };
            return chrono::NaiveDate::parse_from_str(&iso, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| date.and_utc().timestamp_millis());
        }
        match item
            .column_data
            .get(year)
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|year| *year != 0)
        {
            Some(year) => chrono::Local
                .with_ymd_and_hms(
                    if (1..100).contains(&year) {
                        year + 1900
                    } else {
                        year
                    },
                    1,
                    1,
                    0,
                    0,
                    0,
                )
                .earliest()
                .map(|date| date.timestamp_millis()),
            None => Some(0),
        }
    }
    let name = || {
        a.name
            .to_lowercase()
            .encode_utf16()
            .cmp(b.name.to_lowercase().encode_utf16())
    };
    let number = |item: &Item, key: &str| {
        item.column_data
            .get(key)
            .map_or_else(
                || match key {
                    "duration" => Some(item.duration),
                    "playCount" => Some(item.play_count as f64),
                    "userRating" => (item.rating > 0).then_some(item.rating as f64),
                    _ => None,
                },
                |value| value.parse::<f64>().ok(),
            )
            .filter(|value| value.is_finite())
    };
    let string = |item: &Item, key: &str| {
        item.column_data
            .get(key)
            .filter(|value| !value.is_empty())
            .map(|value| value.encode_utf16().collect::<Vec<_>>())
    };
    let order = match sort {
        Sort::Name => name(),
        Sort::SortName => a
            .column_data
            .get("sortName")
            .unwrap_or(&a.name)
            .encode_utf16()
            .cmp(
                b.column_data
                    .get("sortName")
                    .unwrap_or(&b.name)
                    .encode_utf16(),
            ),
        // The original sorter reads legacy albumArtist/starred properties, absent on normalized albums.
        Sort::AlbumArtist => return name(),
        Sort::Favorited | Sort::Id => Ordering::Equal,
        Sort::ReleaseDate => optional(
            release(a, "originalDate", "originalYear"),
            release(b, "originalDate", "originalYear"),
        )
        .then_with(|| {
            optional(
                release(a, "releaseDate", "releaseYear"),
                release(b, "releaseDate", "releaseYear"),
            )
        }),
        Sort::Year => optional(number(a, "releaseYear"), number(b, "releaseYear")),
        Sort::Duration | Sort::Frequent | Sort::Rating | Sort::SongCount => {
            optional(number(a, sort.column()), number(b, sort.column()))
        }
        Sort::Newest | Sort::Recent => optional(string(a, sort.column()), string(b, sort.column())),
        _ => Ordering::Equal,
    };
    let order = if descending { order.reverse() } else { order };
    if sort == Sort::ReleaseDate {
        order.then_with(name)
    } else {
        order
    }
}

pub fn sort_albums(items: &mut [Item], sort: crate::model::Sort, descending: bool) {
    match sort {
        crate::model::Sort::Id if descending => items.reverse(),
        crate::model::Sort::Random => {
            use rand::seq::SliceRandom;
            items.shuffle(&mut rand::thread_rng());
        }
        _ => items.sort_by(|a, b| compare_albums(a, b, sort, descending)),
    }
}

pub fn compare(a: &Item, b: &Item, column: &str) -> std::cmp::Ordering {
    if a.kind == crate::model::ItemKind::Song
        && b.kind == crate::model::ItemKind::Song
        && let Some(order) = compare_songs(a, b, column)
    {
        return order;
    }
    match column {
        "duration" => a.duration.total_cmp(&b.duration),
        "year" => a.year.cmp(&b.year),
        "trackNumber" => a.disc.cmp(&b.disc).then(a.track.cmp(&b.track)),
        "discNumber" => a.disc.cmp(&b.disc),
        "playCount" => a.play_count.cmp(&b.play_count),
        "userRating" => a.rating.cmp(&b.rating),
        "userFavorite" => a.favorite.cmp(&b.favorite),
        "public" => a.public.cmp(&b.public),
        "id" if a.playlist_position.is_some() && b.playlist_position.is_some() => {
            a.playlist_position.cmp(&b.playlist_position)
        }
        "albumCount" | "songCount" | "bpm" | "channels" | "bitRate" | "sampleRate" | "bitDepth"
        | "size" | "communityRating" | "criticRating" | "releaseYear" => {
            let number = |item: &Item| {
                item.column_data
                    .get(column)
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(if column == "releaseYear" {
                        item.year as f64
                    } else {
                        0.
                    })
            };
            number(a).total_cmp(&number(b))
        }
        _ => text(a, column, 0)
            .to_lowercase()
            .encode_utf16()
            .cmp(text(b, column, 0).to_lowercase().encode_utf16()),
    }
}

fn compare_songs(a: &Item, b: &Item, column: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    // Lodash sorts missing metadata after defined values and retains complete ties.
    fn optional<T: PartialOrd>(a: Option<T>, b: Option<T>) -> Ordering {
        a.is_none()
            .cmp(&b.is_none())
            .then_with(|| a.partial_cmp(&b).unwrap_or(Ordering::Equal))
    }
    let folded = |a: &str, b: &str| {
        a.to_lowercase()
            .encode_utf16()
            .cmp(b.to_lowercase().encode_utf16())
    };
    let album = || folded(&a.album, &b.album);
    let name = || folded(&a.name, &b.name);
    let number = |item: &Item| {
        item.column_data
            .get(column)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
    };
    let optional_text = |item: &Item| {
        item.column_data
            .get(column)
            .map(|value| value.encode_utf16().collect::<Vec<_>>())
    };
    let first_credit = |item: &Item| {
        let links = if column == "albumArtists" {
            crate::artists::links(item, true)
        } else {
            crate::genres::links(item)
        };
        links
            .first()
            .map(|(_, name)| name.to_lowercase().encode_utf16().collect::<Vec<_>>())
    };
    let order = match column {
        "name" => return Some(name().then_with(album)),
        "sortName" => {
            return Some(
                text(a, column, 0)
                    .encode_utf16()
                    .cmp(text(b, column, 0).encode_utf16()),
            );
        }
        "id" => return Some(Ordering::Equal),
        "album" => album(),
        "albumArtists" | "genres" => optional(first_credit(a), first_credit(b)),
        "artists" => folded(&a.artist, &b.artist),
        "bpm" | "channels" | "releaseYear" => optional(number(a), number(b)),
        "comment" => {
            let comment = |item: &Item| {
                item.column_data
                    .get(column)
                    .map(String::as_str)
                    .or_else(|| (!item.comment.is_empty()).then_some(item.comment.as_str()))
                    .map(|value| value.encode_utf16().collect::<Vec<_>>())
            };
            optional(comment(a), comment(b))
        }
        "lastPlayedAt" | "releaseDate" => optional(optional_text(a), optional_text(b)),
        "createdAt" => {
            let day = |item: &Item| {
                item.column_data.get(column).and_then(|value| {
                    chrono::DateTime::parse_from_rfc3339(value)
                        .map(|date| date.with_timezone(&chrono::Local).date_naive())
                        .ok()
                        .or_else(|| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
                })
            };
            optional(day(a), day(b))
        }
        "duration" => a.duration.total_cmp(&b.duration),
        "playCount" => a.play_count.cmp(&b.play_count),
        "year" => a.year.cmp(&b.year),
        "userFavorite" | "starredAt" => a.favorite.cmp(&b.favorite).then_with(name),
        "userRating" => a.rating.cmp(&b.rating).then_with(name),
        _ => return None,
    };
    Some(
        order
            .then_with(|| {
                if matches!(column, "album" | "artists") {
                    Ordering::Equal
                } else {
                    album()
                }
            })
            .then(a.disc.cmp(&b.disc))
            .then(a.track.cmp(&b.track)),
    )
}
pub fn folder_items(
    mut items: Vec<Item>,
    sort: crate::model::Sort,
    descending: bool,
    search: &str,
) -> Vec<Item> {
    use crate::model::ItemKind;
    let search = search.to_lowercase();
    items.retain(|item| {
        item.name.to_lowercase().contains(&search)
            || (item.kind == ItemKind::Song
                && [item.album.as_str(), item.artist.as_str()]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&search)))
    });
    sort_items(&mut items, sort, descending);
    items.sort_by(
        |a, b| match (a.kind == ItemKind::Folder, b.kind == ItemKind::Folder) {
            (true, true) => {
                let order = compare(a, b, "name");
                if descending { order.reverse() } else { order }
            }
            (a, b) => b.cmp(&a),
        },
    );
    items
}

pub fn sort_items(items: &mut [Item], sort: crate::model::Sort, descending: bool) {
    if sort == crate::model::Sort::Id
        && items
            .iter()
            .all(|item| item.kind == crate::model::ItemKind::Song)
    {
        if descending {
            items.reverse();
        }
    } else if sort == crate::model::Sort::Random {
        use rand::seq::SliceRandom;
        items.shuffle(&mut rand::thread_rng());
    } else {
        items.sort_by(|a, b| {
            let order = compare(a, b, sort.column());
            if descending { order.reverse() } else { order }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_search_matches_original_localized_highlights() {
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/list-config-search-reference.json"))
                .unwrap();
        for group in reference {
            for search in group["searches"].as_array().unwrap() {
                let matches = group["labels"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .filter(|(_, label)| {
                        config_search_matches(
                            search["query"].as_str().unwrap(),
                            label.as_str().unwrap(),
                        )
                    })
                    .map(|(index, _)| index.to_string())
                    .collect::<Vec<_>>();
                assert_eq!(
                    serde_json::json!(matches),
                    search["matches"],
                    "{} {}: {search}",
                    group["section"],
                    group["language"]
                );
            }
        }
    }
    #[test]
    fn album_search_matches_original_fuse_fields_typos_and_relevance() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/album-search-reference.json")).unwrap();
        let items: Vec<Item> = serde_json::from_value(reference["items"].clone()).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let rows = search_albums(&items, case["query"].as_str().unwrap());
            assert_eq!(
                serde_json::json!(
                    rows.into_iter()
                        .map(|index| &items[index].id)
                        .collect::<Vec<_>>()
                ),
                case["ids"],
                "{case}"
            );
        }
    }
    #[test]
    fn library_search_matches_original_fuse_fields_typos_relevance_and_exclusions() {
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/library-search-reference.json")).unwrap();
        for list in reference {
            let items: Vec<Item> = serde_json::from_value(list["items"].clone()).unwrap();
            let kind = serde_json::from_value(list["kind"].clone()).unwrap();
            if let Some(cases) = list["filters"].as_array() {
                for case in cases {
                    let filters: crate::filters::Filters =
                        serde_json::from_value(case["filters"].clone()).unwrap();
                    let rows = filters.search(items.clone(), case["query"].as_str().unwrap(), kind);
                    assert_eq!(
                        serde_json::json!(rows.iter().map(|item| &item.id).collect::<Vec<_>>()),
                        case["ids"],
                        "{case}"
                    );
                }
            }
            if let Some(cases) = list["sorts"].as_array() {
                for case in cases {
                    let rows = radio_items(
                        items.clone(),
                        case["query"].as_str().unwrap(),
                        serde_json::from_value(case["sort"].clone()).unwrap(),
                        case["descending"].as_bool().unwrap(),
                    );
                    assert_eq!(
                        serde_json::json!(rows.iter().map(|item| &item.id).collect::<Vec<_>>()),
                        case["ids"],
                        "{case}"
                    );
                }
            }
            for case in list["searches"].as_array().unwrap() {
                let rows = search_items(&items, case["query"].as_str().unwrap(), kind);
                assert_eq!(
                    serde_json::json!(
                        rows.into_iter()
                            .map(|index| &items[index].id)
                            .collect::<Vec<_>>()
                    ),
                    case["ids"],
                    "{}: {case}",
                    list["sourceKind"]
                );
            }
        }
    }
    #[test]
    fn album_sort_matches_original_client_order_nulls_dates_and_stable_ties() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/album-sort-reference.json")).unwrap();
        assert_eq!(serde_json::json!(ALBUM_SORTS), reference["menu"]);
        let source: Vec<Item> = serde_json::from_value(reference["items"].clone()).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let mut albums = source.clone();
            sort_albums(
                &mut albums,
                serde_json::from_value(case["sort"].clone()).unwrap(),
                case["descending"].as_bool().unwrap(),
            );
            assert_eq!(
                serde_json::json!(albums.iter().map(|item| &item.id).collect::<Vec<_>>()),
                case["ids"],
                "{case}"
            );
        }
    }
    #[test]
    fn song_sort_matches_original_secondary_keys_missing_values_and_stable_ties() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/song-sort-reference.json")).unwrap();
        assert_eq!(serde_json::json!(SONG_SORTS), reference["menu"]);
        let source: Vec<Item> = serde_json::from_value(reference["items"].clone()).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let mut songs = source.clone();
            let sort = serde_json::from_value(case["sort"].clone()).unwrap();
            sort_items(&mut songs, sort, case["descending"].as_bool().unwrap());
            assert_eq!(
                serde_json::json!(songs.iter().map(|item| &item.id).collect::<Vec<_>>()),
                case["ids"],
                "{case}"
            );
            let mut playlist = source.clone();
            crate::playlist_albums::sort_songs(
                &mut playlist,
                sort,
                case["descending"].as_bool().unwrap(),
            );
            assert_eq!(playlist, songs, "Playlist: {case}");
        }
    }
    #[test]
    fn table_layout_and_resize_match_the_original_source() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/table-layout-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let columns: Vec<Column> = serde_json::from_value(case["columns"].clone()).unwrap();
            let actual = layout_columns(
                &columns,
                case["autoFitColumns"].as_bool().unwrap(),
                case["totalContainerWidth"].as_f64().unwrap(),
                case["centerContainerWidth"].as_f64().unwrap(),
            );
            assert_eq!(
                serde_json::json!(actual.iter().map(|column| &column.id).collect::<Vec<_>>()),
                case["ids"],
                "{case}"
            );
            assert_eq!(
                actual
                    .iter()
                    .map(|column| f64::from(column.width))
                    .collect::<Vec<_>>(),
                case["widths"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|width| width.as_f64().unwrap())
                    .collect::<Vec<_>>(),
                "{case}"
            );
        }
        for (id, width) in reference["fixed"].as_object().unwrap() {
            assert_eq!(
                detail_fixed_width(id),
                Some(width.as_f64().unwrap() as f32),
                "{id}"
            );
        }
        assert_eq!(detail_fixed_width("name"), None);
        for (name, size) in [
            ("compact", Size::Compact),
            ("default", Size::Default),
            ("large", Size::Large),
        ] {
            assert_eq!(
                f64::from(size.row_height()),
                reference["sizes"][name]["table"].as_f64().unwrap()
            );
            assert_eq!(
                f64::from(size.detail_row_height()),
                reference["sizes"][name]["detail"].as_f64().unwrap()
            );
        }
        for case in reference["resizing"].as_array().unwrap() {
            assert_eq!(
                resized_column_width(
                    case["start"].as_f64().unwrap() as f32,
                    case["delta"].as_f64().unwrap() as f32
                ),
                case["width"].as_f64().unwrap() as f32
            );
        }
    }
    #[test]
    fn client_text_sort_matches_electron_casefolded_utf16_order() {
        let mut items = ["Song 2", "Song 10", "SONG 1", "\u{e000}", "\u{1f600}"]
            .into_iter()
            .map(|name| Item {
                name: name.into(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        sort_items(&mut items, crate::model::Sort::Name, false);
        assert_eq!(
            items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["SONG 1", "Song 10", "Song 2", "\u{1f600}", "\u{e000}"]
        );
        sort_items(&mut items, crate::model::Sort::Name, true);
        assert_eq!(items[0].name, "\u{e000}");
    }
    #[test]
    fn defaults_match_original_lists_and_album_artist_credits() {
        let original: serde_json::Value =
            serde_json::from_str(include_str!("../assets/electron-default-settings.json")).unwrap();
        let album = default_for("album");
        assert_eq!(
            album
                .grid
                .rows
                .iter()
                .filter(|row| row.is_enabled)
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["name", "albumArtists", "releaseYear"]
        );
        for (alias, key) in [
            ("genreAlbum", "album"),
            ("albumArtistAlbum", "album"),
            ("genreSong", "song"),
            ("albumArtistSong", "song"),
            ("fullScreenSong", "fullScreen"),
        ] {
            assert_eq!(
                serde_json::to_value(default_for(alias)).unwrap(),
                serde_json::to_value(default_for(key)).unwrap()
            );
        }
        let mut saved = BTreeMap::from([("album".into(), album)]);
        saved.get_mut("album").unwrap().grid.rows[1].is_enabled = false;
        assert!(!configured(&saved, "genreAlbum").grid.rows[1].is_enabled);
        saved.insert("genreAlbum".into(), default_for("album"));
        assert!(configured(&saved, "genreAlbum").grid.rows[1].is_enabled);
        for (key, value) in original["lists"].as_object().unwrap() {
            let expected: Config = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(default_for(key)).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{key}"
            );
        }
    }

    #[test]
    fn list_configuration_preserves_columns_cards_and_pagination_and_rejects_invalid_widths() {
        let mut config = default_for("album");
        config.pagination = "paginated".into();
        config.items_per_page = 25;
        config.table.columns.reverse();
        config.table.columns[0].pinned = Some("right".into());
        config.grid.items_per_row_enabled = true;
        config.grid.items_per_row = 6;
        let mut configs = BTreeMap::from([("album".into(), config)]);
        validate(&configs).unwrap();
        let copy: BTreeMap<String, Config> =
            serde_json::from_str(&serde_json::to_string(&configs).unwrap()).unwrap();
        assert_eq!(copy["album"].table.columns[0].id, "actions");
        assert_eq!(copy["album"].grid.items_per_row, 6);
        assert_eq!(copy["album"].items_per_page, 25);
        configs.get_mut("album").unwrap().table.columns[0].width = f32::NAN;
        assert!(validate(&configs).is_err());
        let item = Item {
            name: "Song".into(),
            artist: "Artist".into(),
            track: 3,
            ..Default::default()
        };
        assert_eq!(text(&item, "titleArtist", 0), "Song · Artist");
        assert_eq!(text(&item, "trackNumber", 0), "3");
        let item = Item {
            album: "Album".into(),
            release_types: vec!["live".into()],
            ..item
        };
        assert_eq!(text(&item, "albumGroup", 0), "Album");
        assert_eq!(text(&item, "releaseType", 0), "live");
    }
}
