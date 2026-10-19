use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    #[default]
    Navidrome,
    Subsonic,
    Jellyfin,
}

impl ServerType {
    pub const ALL: [Self; 3] = [Self::Navidrome, Self::Subsonic, Self::Jellyfin];

    pub fn label(self) -> &'static str {
        match self {
            Self::Navidrome => "Navidrome",
            Self::Subsonic => "Subsonic",
            Self::Jellyfin => "Jellyfin",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerDefaults {
    pub kind: ServerType,
    pub name: String,
    pub url: String,
    pub remote_url: String,
    pub legacy_auth: bool,
}
impl ServerDefaults {
    pub fn from_environment(get: impl Fn(&str) -> Option<String>) -> Self {
        let kind = get("SERVER_TYPE").and_then(|value| {
            serde_json::from_value::<ServerType>(value.to_lowercase().into()).ok()
        });
        Self {
            kind: kind.unwrap_or_default(),
            name: get("SERVER_NAME")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "My Server".into()),
            url: get("SERVER_URL").unwrap_or_else(|| "http://".into()),
            remote_url: get("REMOTE_URL").unwrap_or_default(),
            legacy_auth: kind.is_some()
                && get("LEGACY_AUTHENTICATION")
                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
        }
    }
}
impl Default for ServerDefaults {
    fn default() -> Self {
        Self::from_environment(|_| None)
    }
}

#[cfg(test)]
mod server_defaults_tests {
    use super::*;
    #[test]
    fn password_retention_defaults_preserve_existing_native_profiles() {
        assert!(!ServerOptions::default().save_password);
        let previous: ServerOptions = serde_json::from_str("{}").unwrap();
        assert!(previous.save_password);
        for save_password in [false, true] {
            let options: ServerOptions =
                serde_json::from_value(serde_json::json!({"save_password": save_password}))
                    .unwrap();
            assert_eq!(options.save_password, save_password);
            let roundtrip: ServerOptions =
                serde_json::from_value(serde_json::to_value(options).unwrap()).unwrap();
            assert_eq!(roundtrip.save_password, save_password);
        }
    }
    #[test]
    fn original_server_environment_defaults_preserve_types_names_addresses_and_legacy_scope() {
        let defaults = ServerDefaults::default();
        assert_eq!(defaults.kind, ServerType::Navidrome);
        assert_eq!(defaults.name, "My Server");
        assert_eq!(defaults.url, "http://");
        assert!(!defaults.legacy_auth);
        for (value, expected) in [
            ("JELLYFIN", ServerType::Jellyfin),
            ("navidrome", ServerType::Navidrome),
            ("SubSonic", ServerType::Subsonic),
        ] {
            let values = std::collections::HashMap::from([
                ("SERVER_TYPE", value),
                ("SERVER_NAME", "音乐"),
                ("SERVER_URL", "https://localhost/music"),
                ("REMOTE_URL", "https://remote.example/music"),
                ("LEGACY_AUTHENTICATION", "TRUE"),
            ]);
            let configured = ServerDefaults::from_environment(|key| {
                values.get(key).map(|value| value.to_string())
            });
            assert_eq!(configured.kind, expected);
            assert!(configured.legacy_auth);
            assert_eq!(configured.name, "音乐");
            assert_eq!(configured.url, "https://localhost/music");
            assert_eq!(configured.remote_url, "https://remote.example/music");
        }
        let invalid = ServerDefaults::from_environment(|key| match key {
            "SERVER_TYPE" => Some("unknown".into()),
            "SERVER_NAME" | "SERVER_URL" => Some(String::new()),
            "LEGACY_AUTHENTICATION" => Some("true".into()),
            _ => None,
        });
        assert_eq!(invalid.kind, ServerType::Navidrome);
        assert!(!invalid.legacy_auth);
        assert_eq!(invalid.name, "My Server");
        assert!(invalid.url.is_empty());
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub username: String,
    pub kind: ServerType,
    #[serde(default)]
    pub legacy_auth: bool,
    #[serde(default)]
    pub options: ServerOptions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerOptions {
    pub ignore_ssl: bool,
    #[serde(default = "previous_password_retention")]
    pub save_password: bool,
    pub remote_url: String,
    pub prefer_remote_url: bool,
    pub prefer_instant_mix: bool,
    pub enable_audiobooks: bool,
    pub music_folder_ids: Vec<String>,
}

fn previous_password_retention() -> bool {
    // Earlier native profiles always retained the password with saved sessions.
    true
}

// Session secrets are serialized only into the operating system credential store.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub salt: String,
    pub subsonic_token: String,
    pub legacy_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_password: Option<String>,
    pub is_admin: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum ItemKind {
    #[default]
    Song,
    Album,
    Artist,
    AlbumArtist,
    Playlist,
    Genre,
    Folder,
    Radio,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub server_id: Uuid,
    pub kind: ItemKind,
    pub name: String,
    pub artist: String,
    pub artist_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_links: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist_links: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "indexmap::IndexMap::is_empty")]
    pub participants: indexmap::IndexMap<String, Vec<(String, String)>>,
    pub album: String,
    pub album_id: String,
    pub artist_id: String,
    pub album_artist: String,
    pub album_artist_names: Vec<String>,
    pub album_artist_id: String,
    pub album_artist_ids: Vec<String>,
    pub artist_ids: Vec<String>,
    pub genre_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre_links: Option<Vec<(String, String)>>,
    pub release_types: Vec<String>,
    pub is_compilation: bool,
    pub column_data: std::collections::BTreeMap<String, String>,
    pub roles: Vec<String>,
    pub duration: f64,
    pub resume_position: f64,
    pub resumable: bool,
    pub musicbrainz_id: String,
    pub musicbrainz_release_group_id: String,
    pub track: u32,
    pub disc: u32,
    pub year: u32,
    pub favorite: bool,
    pub rating: u8,
    pub play_count: u32,
    pub cover_id: String,
    pub thumb_hash: String,
    pub blur_hash: String,
    pub dominant_color: String,
    #[serde(default)]
    pub cover_revision: String,
    pub genre: String,
    pub genre_names: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub record_labels: Vec<String>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub tags: std::collections::BTreeMap<String, Vec<String>>,
    pub stream_url: String,
    #[serde(default)]
    pub homepage: String,
    pub comment: String,
    pub playlist_entry_id: String,
    pub playlist_position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_playlist_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub playlist_songs: Vec<Item>,
    #[serde(default)]
    pub rules: Option<serde_json::Value>,
    #[serde(default)]
    pub public: bool,
    pub owner_id: String,
    pub owner_name: String,
    pub sync: Option<bool>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub container: String,
    #[serde(default)]
    pub folder_root: bool,
    #[serde(default)]
    pub gain_track: Option<f32>,
    #[serde(default)]
    pub gain_album: Option<f32>,
    #[serde(default)]
    pub peak_track: Option<f32>,
    #[serde(default)]
    pub peak_album: Option<f32>,
}

impl Item {
    pub fn set_rating(&mut self, rating: u8) {
        self.rating = rating;
        self.column_data
            .insert("userRating".into(), rating.to_string());
    }
    pub fn primary_artist(&self) -> &str {
        self.artist_names
            .first()
            .map_or(&self.artist, String::as_str)
    }
    pub fn primary_album_artist(&self) -> &str {
        self.album_artist_names.first().map_or_else(
            || {
                if self.album_artist.is_empty() {
                    self.primary_artist()
                } else {
                    &self.album_artist
                }
            },
            String::as_str,
        )
    }
    pub fn action_items(&self) -> Vec<Self> {
        if self.playlist_songs.is_empty() {
            vec![self.clone()]
        } else {
            self.playlist_songs.clone()
        }
    }
    pub fn can_edit_playlist(&self, session: &Session) -> bool {
        self.kind == ItemKind::Playlist
            && (session.is_admin || !self.public || self.owner_id == session.user_id)
    }
    pub fn explicit(&self) -> bool {
        self.explicit_status() == Some("EXPLICIT")
    }

    pub fn explicit_status(&self) -> Option<&'static str> {
        match self.column_data.get("explicitStatus").map(String::as_str) {
            Some("e" | "explicit" | "EXPLICIT") => Some("EXPLICIT"),
            Some("c" | "clean" | "CLEAN") => Some("CLEAN"),
            _ => None,
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let searchable = format!(
            "{} {} {} {} {} {} {}",
            self.name,
            self.artist,
            self.album,
            self.album_artist,
            self.genre,
            self.year,
            self.comment
        )
        .to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|word| searchable.contains(word))
    }
}
pub fn filtered_page(items: Vec<Item>, query: &str, offset: usize, limit: usize) -> Page {
    let items = items
        .into_iter()
        .filter(|item| item.matches(query))
        .collect::<Vec<_>>();
    let total = items.len();
    Page {
        items: items.into_iter().skip(offset).take(limit).collect(),
        total: Some(total),
        has_more: offset.saturating_add(limit) < total,
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArtistInfo {
    pub biography: String,
    pub biography_html: String,
    pub homepage: String,
    pub similar: Vec<Item>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArtistTab {
    #[default]
    Albums,
    Songs,
    PersonalTop,
    CommunityTop,
    Favorites,
    Rated,
    Similar,
}
impl ArtistTab {
    pub const ALL: [Self; 7] = [
        Self::Albums,
        Self::Songs,
        Self::PersonalTop,
        Self::CommunityTop,
        Self::Favorites,
        Self::Rated,
        Self::Similar,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Albums => "Albums",
            Self::Songs => "Songs",
            Self::PersonalTop => "Most played",
            Self::CommunityTop => "Community top songs",
            Self::Favorites => "Favorite songs",
            Self::Rated => "Rated songs",
            Self::Similar => "Similar artists",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: Option<usize>,
    pub has_more: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    #[default]
    Name,
    Newest,
    Recent,
    Frequent,
    Random,
    Year,
    Rating,
    Artist,
    Album,
    Duration,
    AlbumArtist,
    Bpm,
    Channels,
    Comment,
    Genre,
    Id,
    SortName,
    ReleaseDate,
    ReleaseYear,
    SongCount,
    AlbumCount,
    Favorited,
    CommunityRating,
    CriticRating,
    Explicit,
    Owner,
    Public,
    UpdatedAt,
    Track,
    Disc,
    BitRate,
    SampleRate,
    Size,
    Path,
    Container,
}

impl Sort {
    pub const ALL: [Self; 35] = [
        Self::Name,
        Self::Newest,
        Self::Recent,
        Self::Frequent,
        Self::Random,
        Self::Year,
        Self::Rating,
        Self::Artist,
        Self::Album,
        Self::Duration,
        Self::AlbumArtist,
        Self::Bpm,
        Self::Channels,
        Self::Comment,
        Self::Genre,
        Self::Id,
        Self::SortName,
        Self::ReleaseDate,
        Self::ReleaseYear,
        Self::SongCount,
        Self::AlbumCount,
        Self::Favorited,
        Self::CommunityRating,
        Self::CriticRating,
        Self::Explicit,
        Self::Owner,
        Self::Public,
        Self::UpdatedAt,
        Self::Track,
        Self::Disc,
        Self::BitRate,
        Self::SampleRate,
        Self::Size,
        Self::Path,
        Self::Container,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Newest => "Recently added",
            Self::Recent => "Recently played",
            Self::Frequent => "Most played",
            Self::Random => "Random",
            Self::Year => "Year",
            Self::Rating => "Rating",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Duration => "Duration",
            Self::AlbumArtist => "Album artist",
            Self::Bpm => "BPM",
            Self::Channels => "Channels",
            Self::Comment => "Comment",
            Self::Genre => "Genre",
            Self::Id => "Original order / ID",
            Self::SortName => "Sort name",
            Self::ReleaseDate => "Release date",
            Self::ReleaseYear => "Release year",
            Self::SongCount => "Track count",
            Self::AlbumCount => "Album count",
            Self::Favorited => "Date favorited",
            Self::CommunityRating => "Community rating",
            Self::CriticRating => "Critic rating",
            Self::Explicit => "Explicit status",
            Self::Owner => "Owner",
            Self::Public => "Public",
            Self::UpdatedAt => "Last updated",
            Self::Track => "Track number",
            Self::Disc => "Disc number",
            Self::BitRate => "Bitrate",
            Self::SampleRate => "Sample rate",
            Self::Size => "File size",
            Self::Path => "File path",
            Self::Container => "File format",
        }
    }
    pub fn column(self) -> &'static str {
        match self {
            Self::Name | Self::Random => "name",
            Self::Newest => "createdAt",
            Self::Recent => "lastPlayedAt",
            Self::Frequent => "playCount",
            Self::Year => "year",
            Self::Rating => "userRating",
            Self::Artist => "artists",
            Self::AlbumArtist => "albumArtists",
            Self::Album => "album",
            Self::Duration => "duration",
            Self::Bpm => "bpm",
            Self::Channels => "channels",
            Self::Comment => "comment",
            Self::Genre => "genres",
            Self::Id => "id",
            Self::SortName => "sortName",
            Self::ReleaseDate => "releaseDate",
            Self::ReleaseYear => "releaseYear",
            Self::SongCount => "songCount",
            Self::AlbumCount => "albumCount",
            Self::Favorited => "starredAt",
            Self::CommunityRating => "communityRating",
            Self::CriticRating => "criticRating",
            Self::Explicit => "explicitStatus",
            Self::Owner => "username",
            Self::Public => "public",
            Self::UpdatedAt => "updatedAt",
            Self::Track => "trackNumber",
            Self::Disc => "discNumber",
            Self::BitRate => "bitRate",
            Self::SampleRate => "sampleRate",
            Self::Size => "size",
            Self::Path => "path",
            Self::Container => "container",
        }
    }
    pub fn from_column(column: &str) -> Option<Self> {
        match column {
            "titleArtist" | "titleCombined" => Some(Self::Name),
            "date" => Some(Self::ReleaseDate),
            "userFavorite" => Some(Self::Favorited),
            "genreBadge" => Some(Self::Genre),
            "rowIndex" => Some(Self::Id),
            _ => Self::ALL
                .into_iter()
                .find(|sort| *sort != Self::Random && sort.column() == column),
        }
    }
    pub fn native_field(self, server: ServerType, kind: ItemKind) -> Option<&'static str> {
        if server == ServerType::Subsonic && kind != ItemKind::Album && self != Self::Name {
            return None;
        }
        if server == ServerType::Navidrome {
            let supported = match kind {
                ItemKind::Artist | ItemKind::AlbumArtist => matches!(
                    self,
                    Self::Name
                        | Self::SortName
                        | Self::AlbumCount
                        | Self::SongCount
                        | Self::Frequent
                        | Self::Rating
                        | Self::Favorited
                ),
                ItemKind::Playlist => matches!(
                    self,
                    Self::Name
                        | Self::Duration
                        | Self::Owner
                        | Self::Public
                        | Self::SongCount
                        | Self::UpdatedAt
                ),
                ItemKind::Genre => matches!(self, Self::Name | Self::SongCount | Self::AlbumCount),
                ItemKind::Radio => matches!(self, Self::Name | Self::Id),
                _ => true,
            };
            if !supported {
                return None;
            }
        }
        match server {
            ServerType::Navidrome => match self {
                Self::Name | Self::SortName => Some(if kind == ItemKind::Song {
                    "title"
                } else {
                    "name"
                }),
                Self::Newest => Some(if kind == ItemKind::Album {
                    "recently_added"
                } else {
                    "createdAt"
                }),
                Self::Recent => Some(if kind == ItemKind::Album {
                    "play_date"
                } else {
                    "playDate"
                }),
                Self::Frequent => Some(if kind == ItemKind::Album {
                    "play_count"
                } else {
                    "playCount"
                }),
                Self::Random => Some("random"),
                Self::Year => Some(if kind == ItemKind::Album {
                    "max_year"
                } else {
                    "year"
                }),
                Self::Rating => Some("rating"),
                Self::Artist => Some("artist"),
                Self::Album => Some("album"),
                Self::Duration => Some("duration"),
                Self::AlbumArtist => Some(if kind == ItemKind::Album {
                    "album_artist"
                } else {
                    "order_album_artist_name"
                }),
                Self::Bpm => Some("bpm"),
                Self::Channels => Some("channels"),
                Self::Comment => Some("comment"),
                Self::Genre => Some("genre"),
                Self::Id => Some("id"),
                Self::ReleaseYear => Some("releaseYear"),
                Self::SongCount => Some("songCount"),
                Self::AlbumCount => Some("albumCount"),
                Self::Favorited => Some("starred_at"),
                Self::Explicit => Some("explicitStatus"),
                Self::Owner => Some("owner_name"),
                Self::Public => Some("public"),
                Self::UpdatedAt => Some("updatedAt"),
                Self::Track => Some("discNumber,trackNumber"),
                _ => None,
            },
            ServerType::Jellyfin => match self {
                Self::Name | Self::SortName => Some("SortName"),
                Self::Newest => Some("DateCreated"),
                Self::Recent => Some("DatePlayed"),
                Self::Frequent => Some("PlayCount"),
                Self::Random => Some("Random,SortName"),
                Self::Year => Some("ProductionYear"),
                Self::Artist => Some("Artist"),
                Self::AlbumArtist => Some("AlbumArtist"),
                Self::Album => Some("Album"),
                Self::Duration => Some("Runtime"),
                Self::ReleaseDate => Some("PremiereDate"),
                Self::SongCount => Some("ChildCount"),
                Self::CommunityRating => Some("CommunityRating"),
                Self::CriticRating => Some("CriticRating"),
                Self::Track => Some("ParentIndexNumber,IndexNumber,SortName"),
                _ => None,
            },
            ServerType::Subsonic => match self {
                Self::Name => Some("alphabeticalByName"),
                Self::Newest => Some("newest"),
                Self::Recent => Some("recent"),
                Self::Frequent => Some("frequent"),
                Self::Random => Some("random"),
                Self::Year => Some("byYear"),
                Self::Rating => Some("highest"),
                Self::Artist if kind == ItemKind::Album => Some("alphabeticalByArtist"),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListQuery {
    pub kind: ItemKind,
    pub offset: usize,
    pub limit: usize,
    pub search: String,
    pub sort: Sort,
    pub descending: bool,
    pub favorites: bool,
    pub folder_id: Option<String>,
    pub role: Option<String>,
    pub genre: Option<String>,
    pub artist_id: Option<String>,
    pub filters: crate::filters::Filters,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            kind: ItemKind::Album,
            offset: 0,
            limit: 100,
            search: String::new(),
            sort: Sort::Name,
            descending: false,
            favorites: false,
            folder_id: None,
            role: None,
            genre: None,
            artist_id: None,
            filters: Default::default(),
        }
    }
}

pub fn duration_label(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    if seconds >= 86400 {
        format!(
            "{}:{:02}:{:02}:{:02}",
            seconds / 86400,
            seconds / 3600 % 24,
            seconds / 60 % 60,
            seconds % 60
        )
    } else if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    #[test]
    fn playlist_permissions_match_public_ownership_and_admin_rules() {
        let mut session = Session {
            user_id: "me".into(),
            ..Default::default()
        };
        let mut item = Item {
            kind: ItemKind::Playlist,
            owner_id: "other".into(),
            ..Default::default()
        };
        assert!(item.can_edit_playlist(&session));
        item.public = true;
        assert!(!item.can_edit_playlist(&session));
        item.owner_id = "me".into();
        assert!(item.can_edit_playlist(&session));
        item.owner_id.clear();
        assert!(!item.can_edit_playlist(&session));
        session.is_admin = true;
        assert!(item.can_edit_playlist(&session));
        item.kind = ItemKind::Song;
        assert!(!item.can_edit_playlist(&session));
    }
    #[test]
    fn detail_filters_match_metadata_before_paging_without_losing_duplicates() {
        let song = Item {
            name: "声".into(),
            artist: "Björk".into(),
            album: "Live".into(),
            genre: "Pop".into(),
            year: 2020,
            ..Default::default()
        };
        let page = filtered_page(
            vec![song.clone(), Item::default(), song],
            "BJÖRK live 2020",
            1,
            1,
        );
        assert_eq!(page.total, Some(2));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name, "声");
        assert!(!page.has_more);
        assert!(!page.items[0].matches("voice"));
    }
}
