use feishin_core::{
    api::Api,
    lyrics::Line,
    model::*,
    playback::{Control, Event, Player},
    queue::Enqueue,
    storage::{self, State},
};
use gpui::*;
use gpui_component::{
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    slider::{SliderEvent, SliderState},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

actions!(
    feishin,
    [
        TogglePlay,
        NextTrack,
        PreviousTrack,
        FocusSearch,
        GoBack,
        Quit,
        ToggleFullScreenWindow,
        ToggleFullScreenPlayer,
        LeaveFullPlayer
    ]
);

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Home,
    Library(ItemKind),
    Collection(Box<feishin_core::collections::Collection>),
    Collections,
    CollectionEditor,
    Favorites(ItemKind),
    Explore,
    Random,
    Search(String, ItemKind),
    Detail(Box<Item>),
    Similar(Box<Item>),
    Queue,
    Lyrics,
    Servers,
    NoNetwork,
    Settings,
    PlayerSettings,
    HomeSettings,
    FontPicker,
    PresenceSettings,
    QueryFields,
    PlaybackFilters,
    SidebarSettings,
    PanelSettings,
    UpdateSettings,
    SettingsImport,
    AlbumGroups,
    ListSettings(String),
    ListFilters(String),
    Shortcuts,
    PlaylistEditor,
    PlaylistFolderEditor(String),
    PlaylistPicker,
    RadioEditor,
    TagEditor,
    LanguagePicker,
    LyricEditor,
    Share,
    Outputs,
    Info,
    Visualizer,
    VisualizerSettings,
}

// Item equality is unnecessary for navigation; compare route discriminants in the sidebar.
impl Route {
    pub fn title(&self) -> String {
        match self {
            Self::Home => "Home".into(),
            Self::Library(kind) => match kind {
                ItemKind::Album => "Albums",
                ItemKind::Song => "Songs",
                ItemKind::Artist => "Artists",
                ItemKind::AlbumArtist => "Album artists",
                ItemKind::Genre => "Genres",
                ItemKind::Playlist => "Playlists",
                ItemKind::Folder => "Folders",
                ItemKind::Radio => "Radio",
            }
            .into(),
            Self::Collection(collection) => collection.name.clone(),
            Self::Collections => "Collections".into(),
            Self::CollectionEditor => "Save collection".into(),
            Self::Favorites(_) => "Favorites".into(),
            Self::Explore => "Explore".into(),
            Self::Random => "Play random".into(),
            Self::Search(..) => "Search".into(),
            Self::Detail(item) => item.name.clone(),
            Self::Similar(item) => format!("Similar songs: {}", item.name),
            Self::Queue => "Play queue".into(),
            Self::Lyrics => "Now playing".into(),
            Self::Servers => "Servers".into(),
            Self::NoNetwork => "error.noNetwork".into(),
            Self::Settings => "Settings".into(),
            Self::PlayerSettings => "Player appearance".into(),
            Self::HomeSettings => "Home page settings".into(),
            Self::FontPicker => "Fonts".into(),
            Self::PresenceSettings => "setting.discordRichPresence".into(),
            Self::QueryFields => "Custom query builder fields".into(),
            Self::PlaybackFilters => "Player Filters".into(),
            Self::SidebarSettings => "Sidebar settings".into(),
            Self::PanelSettings => "Playback panels".into(),
            Self::UpdateSettings => "Updates".into(),
            Self::SettingsImport => "Import settings".into(),
            Self::ListFilters(_) => "Library filters".into(),
            Self::ListSettings(_) => "List layout".into(),
            Self::AlbumGroups => "Album groups".into(),
            Self::Shortcuts => "Keyboard shortcuts".into(),
            Self::PlaylistFolderEditor(_) => "Rename playlist folder".into(),
            Self::PlaylistEditor => "Playlist editor".into(),
            Self::PlaylistPicker => "Add to playlist".into(),
            Self::RadioEditor => "Add radio station".into(),
            Self::TagEditor => "Edit audio tags".into(),
            Self::LanguagePicker => "Language".into(),
            Self::LyricEditor => "Edit lyrics".into(),
            Self::Share => "Share items".into(),
            Self::Outputs => "Audio outputs".into(),
            Self::Info => "Item information".into(),
            Self::Visualizer => "Visualizer".into(),
            Self::VisualizerSettings => "Visualizer settings".into(),
        }
    }
}

pub struct Feishin {
    pub server_defaults: feishin_core::model::ServerDefaults,
    pub server_discovery: crate::servers::Discovery,
    pub server_check: crate::servers::SessionCheck,
    pub app_menu: Option<crate::app_menu::Signature>,
    pub instance: Option<feishin_core::instance::Instance>,
    pub add_to_playlist: Option<Entity<crate::add_to_playlist::Picker>>,
    pub command_palette: Option<Entity<crate::command_palette::Palette>>,
    pub album_related: Vec<crate::details::RelatedAlbums>,
    pub album_related_generation: u64,
    pub library_scan: crate::scan::State,
    pub cache_busy: bool,
    pub query_cache_generation: u64,
    pub release_notes: crate::release_notes::State,
    pub updates: crate::updates::State,
    #[cfg(windows)]
    pub taskbar: Option<crate::taskbar::Taskbar>,
    #[cfg(windows)]
    pub taskbar_attempted: bool,
    pub queue_drawer: bool,
    pub queue_drawer_generation: u64,
    pub side_queue_search: Entity<InputState>,
    pub side_queue_filter: String,
    pub side_queue_search_debounce: Option<Task<()>>,
    pub side_queue_toolbar_width: f32,
    pub queue_toolbar_width: f32,
    pub side_queue_search_open: bool,
    pub queue_search_open: bool,
    pub side_queue_horizontal_scroll: ScrollHandle,
    pub side_queue_scroll: UniformListScrollHandle,
    pub side_queue_group_scroll: ListState,
    pub queue_group_scroll: ListState,
    pub queue_follow_signature: (Option<Uuid>, bool, bool),
    pub resumed_entry: Option<Uuid>,
    pub queue_autosave: feishin_core::queue::AutoSave,
    pub queue_save_busy: bool,
    pub queue_restore_busy: bool,
    pub pending_queue_change: Option<crate::radio::PendingQueueChange>,
    pub queue_prompt_open: bool,
    pub pending_library_fetch: Option<crate::radio::PendingLibraryFetch>,
    pub library_fetch_prompt_open: bool,
    pub library_fetches:
        std::collections::BTreeMap<u64, std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub hovered_table_row: Option<(crate::lists::TableRow, usize)>,
    pub favorite_requests: usize,
    pub rating_requests: usize,
    pub volume_sync_pending: bool,
    pub volume_change_task: Option<Task<()>>,
    pub notifications: crate::notifications::TrackNotifications,
    pub queue_autosave_input: Entity<InputState>,
    pub accent_input: Entity<InputState>,
    pub cast_devices: Vec<feishin_core::dlna::Device>,
    pub audio_devices: Vec<(String, String)>,
    pub audio_devices_generation: u64,
    pub audio_devices_source: Option<String>,
    pub artist_tab: ArtistTab,
    pub artist_overview: bool,
    pub artist_sections: std::collections::BTreeMap<String, crate::artist_overview::Section>,
    pub artist_section_generation: u64,
    pub artist_searches: [Entity<InputState>; 3],
    pub artist_info: Option<ArtistInfo>,
    pub artist_info_error: String,
    pub artist_info_target: Option<(Uuid, String)>,
    pub artist_role: Option<String>,
    pub cast_selected: Option<feishin_core::dlna::Device>,
    pub cast_connecting: Option<(u64, String)>,
    pub cast_output_open: bool,
    pub cast_builder: Option<crate::speaker_picker::GroupSelection>,
    pub cast_group_pending: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub cast_discovery_generation: u64,
    pub cast_discovery_groups: Vec<feishin_core::dlna::Group>,
    pub cast_scanning: bool,
    pub cast_groups: Vec<feishin_core::dlna::Group>,
    pub cast_members: HashMap<String, feishin_core::dlna::Device>,
    pub cast_member_volumes: HashMap<String, u8>,
    pub cast_member_mutes: HashMap<String, bool>,
    pub cast_volume_pending: HashMap<String, crate::speaker_volume::MemberUpdate>,
    pub cast_topology_generation: u64,
    pub cast_property_versions: HashMap<String, u64>,
    pub cast_property_loading: std::collections::HashSet<String>,
    pub cast_property_pending: HashMap<String, crate::speaker_properties::PropertyWrite>,
    pub cast_properties: HashMap<String, feishin_core::dlna::Properties>,
    pub visualizer_analyzer: feishin_core::visualizer::Analyzer,
    pub visualizer_dimensions: std::rc::Rc<std::cell::Cell<[f32; 2]>>,
    pub system_audio: Option<feishin_core::system_audio::SystemAudio>,
    pub system_audio_ready: bool,
    pub system_audio_error: Option<String>,
    pub visualizer_frame_at: Instant,
    pub visualizer_spin: f32,
    pub visualizer_fps: f32,
    pub visualizer_settings: Option<Entity<InputState>>,
    pub visualizer_numbers: Vec<Entity<InputState>>,
    pub visualizer_subscriptions: Vec<Subscription>,
    pub visualizer_gradient_editor: Option<crate::visualizer::GradientEditor>,
    pub milkdrop: Option<std::rc::Rc<std::cell::RefCell<crate::milkdrop::Renderer>>>,
    pub milkdrop_error: Option<String>,
    pub milkdrop_preset_at: Instant,
    pub milkdrop_cycle_elapsed: f32,
    pub milkdrop_search: Entity<InputState>,
    pub info_items: Vec<Item>,
    pub info_generation: u64,
    pub info_loading: bool,
    pub share_items: Vec<Item>,
    pub share_description: Entity<InputState>,
    pub share_expiry: Entity<InputState>,
    pub share_amount: Entity<InputState>,
    pub share_url: Option<String>,
    pub share_busy: bool,
    pub form_homepage: Entity<InputState>,
    pub lyric_record: Option<feishin_core::lyric_sources::Record>,
    pub lyric_remote_suppressed: HashSet<(Uuid, String)>,
    pub lyric_selections: HashMap<(Uuid, String), crate::lyrics::Selection>,
    pub lyric_song_offset: Entity<InputState>,
    pub lyric_sidebar_offset: Entity<InputState>,
    pub lyric_offset_target: Option<(Uuid, String)>,
    pub lyric_target: Option<Item>,
    pub lyric_text: Entity<InputState>,
    pub lyric_busy: bool,
    pub lyric_generation: u64,
    pub lyric_scroll: ScrollHandle,
    pub sidebar_lyric_scroll: ScrollHandle,
    pub sidebar_lyric_scroll_paused_until: Option<Instant>,
    pub lyric_settings_key: String,
    pub lyric_active: Option<usize>,
    pub lyric_scroll_paused_until: Option<Instant>,
    pub lyric_scroll_motion: [crate::lyric_scroll::Motion; 2],
    pub lyric_numbers: Vec<Entity<InputState>>,
    pub lyric_sliders: Vec<(usize, Entity<SliderState>)>,
    pub lyric_translation_key: Entity<InputState>,
    pub lyric_translation_language: Entity<InputState>,
    pub lyric_translation_region: Entity<InputState>,
    pub lyric_translated: Vec<String>,
    pub lyric_translation_visible: bool,
    pub lyric_translation_busy: bool,
    pub lyric_translation_generation: u64,
    pub lyric_annotations: Vec<feishin_core::japanese::Annotation>,
    pub annotation_generation: u64,
    pub presence: feishin_core::presence::Presence,
    pub last_presence: Instant,
    pub presence_artwork: HashMap<String, (Instant, Option<String>)>,
    pub presence_settings_inputs: [Entity<InputState>; 2],
    pub theme_signature: String,
    pub theme_palette: feishin_core::appearance::Palette,
    pub theme_overlays: std::rc::Rc<std::cell::RefCell<crate::appearance::OverlayCache>>,
    pub last_theme_scan: Instant,
    pub tag_files: Vec<feishin_core::tags::Metadata>,
    pub tag_fields: Vec<crate::editor::Field>,
    pub tag_scope: Option<usize>,
    pub tag_artwork: feishin_core::tags::Artwork,
    pub tag_artwork_path: Option<PathBuf>,
    pub tag_busy: bool,
    pub tag_targets: Vec<(PathBuf, Item)>,
    pub tag_progress: Option<(usize, usize)>,
    pub tag_progress_events: Option<std::sync::mpsc::Receiver<(usize, usize)>>,
    pub tag_subscriptions: Vec<Subscription>,
    pub tag_suggestions: Option<(String, feishin_core::tags::Suggestions)>,
    pub tag_source_names: Vec<String>,
    pub tag_suggestion_generation: u64,
    pub tag_config_edit: Option<String>,
    pub remote: Option<feishin_core::remote::Remote>,
    pub remote_port: Entity<InputState>,
    pub remote_generation: u64,
    pub media: Option<souvlaki::MediaControls>,
    pub(crate) media_metadata: Option<crate::media::Metadata>,
    pub media_events: std::sync::mpsc::Receiver<souvlaki::MediaControlEvent>,
    pub media_sender: std::sync::mpsc::Sender<souvlaki::MediaControlEvent>,
    pub media_attempted: bool,
    pub window_bar_style_active: String,
    pub state: State,
    pub data_dir: PathBuf,
    pub route: Route,
    pub navigation_generation: u64,
    pub secret_form_route: Option<std::mem::Discriminant<Route>>,
    pub history: Vec<crate::navigation::Visit>,
    pub navigation_restore: Option<crate::navigation::Restore>,
    pub page_scroll: ScrollHandle,
    pub sessions: HashMap<Uuid, Api>,
    pub items: Vec<Item>,
    pub random_sample: Option<Vec<Item>>,
    pub album_tracks: HashMap<String, crate::details::AlbumTracks>,
    pub expanded_album: Option<crate::expanded_album::ExpandedAlbum>,
    pub random_form: Option<crate::shuffle::Form>,
    pub album_metadata_cache: HashMap<String, Option<Item>>,
    pub album_group_size: Entity<InputState>,
    pub album_tracks_generation: u64,
    pub album_details_scroll: ListState,
    pub detail_columns: usize,
    pub detail_header_width: Pixels,
    pub library_width: Pixels,
    pub album_metadata_extra: Pixels,
    pub selected_album_tracks: HashSet<(String, usize)>,
    pub album_selection_order: Vec<(String, usize)>,
    pub selected: HashSet<usize>,
    pub grid_selection_order: Vec<usize>,
    pub total: Option<usize>,
    pub has_more: bool,
    pub loading: bool,
    pub message: String,
    pub sort: Sort,
    pub descending: bool,
    pub list_editor: Option<crate::lists::Editor>,
    pub column_resize_origin: Option<(String, String, Pixels, f32)>,
    pub column_resize_preview: Option<(String, String, f32)>,
    pub library_page: usize,
    pub active_filters: feishin_core::filters::Filters,
    pub filter_editor: Option<crate::filters::Editor>,
    pub filter_container_width: Pixels,
    pub playback_filter_inputs: std::collections::HashMap<String, Entity<InputState>>,
    pub filter_subscriptions: Vec<Subscription>,
    pub filter_generation: u64,
    pub filter_input_sync: bool,
    pub list_horizontal_scroll: ScrollHandle,
    pub library_scroll: UniformListScrollHandle,
    pub track_scroll_target: Option<(crate::lists::TableRow, usize, Pixels)>,
    pub queue_display_sort: Option<(String, bool)>,
    pub search: Entity<InputState>,
    pub search_debounce: Option<Task<()>>,
    pub settings_search: Entity<InputState>,
    pub settings_search_open: bool,
    pub settings_tab_focus: [gpui::FocusHandle; 5],
    pub local_search: Entity<InputState>,
    pub local_search_debounce: Option<Task<()>>,
    pub local_filter: String,
    pub server_name: Entity<InputState>,
    pub server_url: Entity<InputState>,
    pub server_remote_url: Entity<InputState>,
    pub server_libraries: Entity<InputState>,
    pub server_options: ServerOptions,
    pub editing_server: Option<Uuid>,
    pub server_library_choices: Vec<Item>,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
    pub remote_username: Entity<InputState>,
    pub remote_password: Entity<InputState>,
    pub server_kind: ServerType,
    pub quick_connect: crate::quick_connect::State,
    pub signing_out: std::collections::HashSet<Uuid>,
    pub legacy_auth: bool,
    pub remember: bool,
    pub mpv_path: Entity<InputState>,
    pub mpv_device: Entity<InputState>,
    pub mpv_options: Entity<InputState>,
    pub preset_name: Entity<InputState>,
    pub compressor_preset_name: Entity<InputState>,
    pub audio_preset_selects: [Entity<crate::equalizer::PresetSelect>; 2],
    pub settings_import: Option<feishin_core::migration::Preview>,
    pub settings_import_loading: bool,
    pub settings_import_generation: u64,
    pub settings_import_error: String,
    pub ffmpeg_path: Entity<InputState>,
    pub path_replace: Entity<InputState>,
    pub path_replace_with: Entity<InputState>,
    pub path_previews: HashMap<Uuid, String>,
    pub collection_rename: Option<crate::collections::CollectionRename>,
    pub collection_draft: Option<feishin_core::collections::Collection>,
    pub form_name: Entity<InputState>,
    pub form_comment: Entity<InputState>,
    pub form_rules: Entity<InputState>,
    pub smart_editor: Option<crate::smart::Editor>,
    pub query_field_inputs: Vec<[Entity<InputState>; 2]>,
    pub query_field_types: Vec<String>,
    pub css_input: Entity<InputState>,
    pub css_file_signature: Option<String>,
    pub playlist_options: Vec<Item>,
    pub playlist_tracks_edit: Option<crate::playlist_actions::TrackEdit>,
    pub playlist_users: Vec<(String, String)>,
    pub playlist_loading: bool,
    pub playlist_generation: u64,
    pub playlist_folder_path: String,
    pub shared_playlist_folder_path: String,
    pub playlist_folder_shared: bool,
    pub folder_browser: crate::folders::Browser,
    pub sidebar_playlists_open: bool,
    pub sidebar_resize: Option<feishin_core::sidebar::Resize>,
    pub sidebar_closed_sections: HashSet<&'static str>,
    pub sidebar_playlist_hover: Option<String>,
    pub sidebar_folder_drag_expanded: HashSet<(Uuid, bool, String)>,
    pub playlist_separator: Entity<InputState>,
    pub playlist_folder_color: Entity<InputState>,
    pub playlist_folder_indent_input: Entity<InputState>,
    pub form_owner: String,
    pub form_sync: Option<bool>,
    pub form_public: bool,
    pub form_songs: Vec<Item>,
    pub form_target: Option<Item>,
    pub playlist_saving: bool,
    pub volume: Entity<SliderState>,
    pub equalizer: Vec<Entity<SliderState>>,
    pub equalizer_inputs: Vec<Entity<InputState>>,
    pub preamp_slider: Entity<SliderState>,
    pub audio_numbers: Vec<Entity<InputState>>,
    pub compressor_sliders: Vec<Entity<SliderState>>,
    pub scrobble_percentage_slider: Entity<SliderState>,
    pub progress: Entity<SliderState>,
    pub seek_preview: crate::progress::SeekPreview,
    pub progress_entry: Option<Uuid>,
    pub lyrics: Vec<Line>,
    pub covers: HashMap<String, PathBuf>,
    pub cover_versions: HashMap<String, u64>,
    pub image_placeholders: std::cell::RefCell<crate::images::PlaceholderCache>,
    pub image_resolution_inputs: [Entity<InputState>; 5],
    pub image_resolution_open: bool,
    pub censored_covers: HashMap<String, PathBuf>,
    pub detail_background: Option<crate::player_display::Background>,
    pub detail_background_key: String,
    pub last_user_activity: Instant,
    pub fullscreen_idle_state: (bool, u64),
    pub fullscreen_idle_input: Entity<InputState>,
    pub volume_settings_inputs: [Entity<InputState>; 3],
    pub volume_wheel_slider: Entity<SliderState>,
    pub speed_input: Entity<InputState>,
    pub sample_rate_input: Entity<InputState>,
    pub bitrate_input: Entity<InputState>,
    pub transcode_format_input: Entity<InputState>,
    pub player_numbers: Vec<Entity<InputState>>,
    pub player_background: Option<crate::player_display::Background>,
    pub player_background_key: String,
    pub player_title_at: Instant,
    pub waveform_sliders: Vec<Entity<SliderState>>,
    pub waveform_data: Option<feishin_core::waveform::Data>,
    pub waveform_key: String,
    pub waveform_generation: u64,
    pub waveform_job: Option<crate::waveform::Job>,
    pub waveform_dragging: bool,
    pub waveform_preview: Option<f64>,
    pub waveform_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    pub full_player: bool,
    // A render can build the library and the outgoing player in the same frame.
    pub rendering_full_player: Option<bool>,
    pub visualizer_display: crate::visualizer_display::State,
    pub player_related: crate::player_related::Related,
    pub queue_scroll: UniformListScrollHandle,
    pub queue_selected: HashSet<Uuid>,
    pub queue_selection_order: Vec<Uuid>,
    pub side_queue_selected: HashSet<Uuid>,
    pub side_queue_selection_order: Vec<Uuid>,
    pub forward_history: Vec<crate::navigation::Visit>,
    pub record_shortcut: Option<feishin_core::shortcuts::Action>,
    pub global_shortcuts: crate::shortcuts::Globals,
    pub last_played: Option<(Uuid, Item)>,
    pub previous_played: Option<Item>,
    pub radio_item: Option<feishin_core::queue::Entry>,
    pub radio_position: f64,
    pub radio_metadata: feishin_core::playback::RadioMetadata,
    pub playing: bool,
    pub buffering: bool,
    pub loaded: bool,
    pub sleep_timer: Option<feishin_core::sleep_timer::Timer>,
    pub sleep_inputs: [Entity<InputState>; 3],
    pub auto_dj_busy: bool,
    pub auto_dj_last: Option<(Uuid, usize)>,
    pub auto_dj_generation: u64,
    pub auto_dj_numbers: Vec<Entity<InputState>>,
    pub auto_dj_folder: Entity<InputState>,
    pub player: Player,
    pub power: feishin_core::power::Inhibitor,
    pub player_generation: u64,
    pub jukebox_server: Option<Uuid>,
    pub request_generation: u64,
    pub play_request_generation: u64,
    pub listening: crate::scrobble::Listening,
    pub track_reports: crate::scrobble::TrackReports,
    pub report_task: Option<Task<()>>,
    pub last_position: f64,
    pub tray: Option<crate::tray::Tray>,
    pub tray_error: Option<String>,
    pub window_hidden: bool,
    pub tray_initialized: bool,
    pub dock_language: String,
    pub home_sections: std::collections::BTreeMap<feishin_core::home::Kind, crate::home::Section>,
    pub home_generation: u64,
    pub home_feature_width: Pixels,
    pub home_feature_wheel_at: Option<Instant>,
    pub home_backgrounds: HashMap<String, Option<crate::player_display::Background>>,
    pub loaded_font_signature: String,
    pub custom_font_family: String,
    pub sidebar_width: Entity<InputState>,
    pub sidebar_regex: Entity<InputState>,
    pub native_hide_supported: bool,
    pub lyric_position_at: Instant,
    pub last_save: Instant,
    pub last_report: Instant,
    pub last_seek_report: Option<Instant>,
    pub save_enabled: bool,
    pub focus: FocusHandle,
    pub library_focus: FocusHandle,
    pub sidebar_search_focus: FocusHandle,
    pub related_focus: FocusHandle,
    pub artist_song_focus: [FocusHandle; 2],
    pub queue_focus: FocusHandle,
    pub side_queue_focus: FocusHandle,
    pub subscriptions: Vec<Subscription>,
}

impl Feishin {
    #[cfg(windows)]
    pub fn sync_taskbar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if cfg!(test) {
            return;
        }
        let labels = [
            self.tr("Previous Track"),
            self.tr("Play/Pause"),
            self.tr("Next Track"),
        ];
        if !self.taskbar_attempted {
            self.taskbar_attempted = true;
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                match crate::taskbar::Taskbar::new(handle.hwnd.get() as *mut _, labels.clone()) {
                    Ok(taskbar) => self.taskbar = Some(taskbar),
                    Err(error) => log::warn!("Windows taskbar controls failed: {error:#}"),
                }
            }
        }
        let Some(taskbar) = &mut self.taskbar else {
            return;
        };
        taskbar.update(self.playing, labels);
        let commands = taskbar.poll();
        for command in commands {
            self.run_shortcut(
                match command {
                    crate::taskbar::Command::Previous => feishin_core::shortcuts::Action::Previous,
                    crate::taskbar::Command::PlayPause => {
                        feishin_core::shortcuts::Action::PlayPause
                    }
                    crate::taskbar::Command::Next => feishin_core::shortcuts::Action::Next,
                },
                window,
                cx,
            );
        }
    }
    pub fn new(
        data_dir: PathBuf,
        loaded: anyhow::Result<State>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (mut state, message, save_enabled) = match loaded {
            Ok(state) => (state, String::new(), true),
            Err(error) => (State::default(), error.to_string(), false),
        };
        state.queue.up_next_enabled = state.settings.up_next_queue;
        state.queue.sync_up_next();
        // Casting is not resumed at startup; restore the local speaker volume.
        for server in &mut state.servers {
            server.options.ignore_ssl = state.settings.ignore_ssl;
        }
        if let Some(volume) = state.settings.previous_local_volume.take() {
            state.settings.volume = volume;
            state.settings.muted = false;
        }
        if !cfg!(test) && state.settings.discord_config.last_fm_key_enabled != Some(false) {
            state.settings.discord_config.last_fm_api_key =
                feishin_core::storage::load_lastfm_key().unwrap_or_default();
        }
        gpui_component::set_locale(&state.settings.language);
        cx.set_global(crate::appearance::Language(state.settings.language.clone()));
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                crate::appearance::input_state(window, cx).placeholder(placeholder.to_owned())
            })
        };
        let presence_settings_inputs = [
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .label_transform(|label, cx| crate::presence_settings::text(label, cx).into())
                    .aria_label("setting.discordApplicationId")
                    .default_value(state.settings.discord_client_id.clone())
            }),
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .label_transform(|label, cx| crate::presence_settings::text(label, cx).into())
                    .aria_label("setting.lastfmApiKey")
                    .placeholder("Leave empty to keep the saved Last.fm key")
                    .masked(true)
            }),
        ];
        let css_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.customCss")
                .auto_grow(8, usize::MAX)
                .default_value(state.settings.css.content.clone())
        });
        let search = input("Search your library...", window, cx);
        let local_search = input("common.search", window, cx);
        let share_amount = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Share expiration amount")
                .default_value("1")
        });
        let share_description = input("Share description", window, cx);
        let share_expiry = input("YYYY-MM-DD HH:MM:SS (empty = server default)", window, cx);
        let form_homepage = input("Station homepage URL (optional)", window, cx);
        let lyric_translation_key = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("API key (leave empty to use saved key)")
                .aria_label("setting.translationApiKey")
                .masked(true)
        });
        let lyric_translation_language = input("Translation target language", window, cx);
        lyric_translation_language.update(cx, |input, cx| {
            input.set_value(
                state.settings.lyric_cloud_translation.language.clone(),
                window,
                cx,
            )
        });
        let lyric_translation_region = input("Azure region (optional)", window, cx);
        lyric_translation_region.update(cx, |input, cx| {
            input.set_value(
                state.settings.lyric_cloud_translation.azure_region.clone(),
                window,
                cx,
            )
        });
        let lyric_song_offset =
            cx.new(|cx| crate::appearance::input_state(window, cx).aria_label("Lyric offset"));
        let lyric_sidebar_offset =
            cx.new(|cx| crate::appearance::input_state(window, cx).aria_label("Lyric offset"));
        let lyric_text = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .multi_line(true)
                .rows(18)
                .placeholder("Paste lyrics or timestamped LRC")
        });
        let server_name = input("form.addServer.input_name", window, cx);
        let server_url = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("https://music.example.com")
                .aria_label("form.addServer.input_url")
        });
        let server_remote_url = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("form.addServer.input_remoteUrlPlaceholder")
                .aria_label("form.addServer.input_remoteUrl")
        });
        let server_defaults = if cfg!(test) {
            feishin_core::model::ServerDefaults::default()
        } else {
            feishin_core::model::ServerDefaults::from_environment(|key| std::env::var(key).ok())
        };
        for (input, value) in [
            (&server_name, &server_defaults.name),
            (&server_url, &server_defaults.url),
            (&server_remote_url, &server_defaults.remote_url),
        ] {
            input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
        }
        let server_libraries = input(
            "Library IDs, separated by commas (empty means all)",
            window,
            cx,
        );
        let username = input("form.addServer.input_username", window, cx);
        let password = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("form.addServer.input_password")
                .masked(true)
        });
        let remote_username = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.remoteUsername")
                .default_value(state.settings.remote_username.clone())
        });
        let remote_port = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.remotePort")
                .default_value(state.settings.remote_port.to_string())
        });
        let remote_password = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("setting.remotePassword")
                .aria_label("setting.remotePassword")
                .masked(true)
        });
        let accent_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Accent color")
                .default_value(state.settings.accent.clone())
        });
        let image_resolution_inputs = feishin_core::images::Placement::ALL.map(|placement| {
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .aria_label(format!(
                        "{} {}",
                        placement.label_key(),
                        "setting.imageResolution"
                    ))
                    .label_transform(move |label, cx| {
                        if label.is_empty() {
                            return label.clone();
                        }
                        format!(
                            "{} {}",
                            crate::appearance::text(placement.label_key(), cx),
                            crate::appearance::text("setting.imageResolution", cx)
                        )
                        .into()
                    })
                    .default_value(state.settings.image_resolution.get(placement).to_string())
            })
        });
        let artist_searches =
            ["Search top songs", "Search favorite songs", "Search albums"].map(|label| {
                cx.new(|cx| crate::appearance::input_state(window, cx).placeholder(label))
            });
        let mpv_path = input("MPV binary path (empty = native audio)", window, cx);
        let album_group_size = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Album artwork size")
                .default_value(state.settings.album_group.image_size.to_string())
        });
        let speed_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Playback speed")
                .default_value(state.settings.speed.to_string())
        });
        let sample_rate_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.transcodeSampleRate")
                .default_value(
                    state
                        .settings
                        .max_sample_rate
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                )
        });
        let bitrate_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.transcodeBitrate")
                .default_value(
                    state
                        .settings
                        .bitrate
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                )
        });
        let transcode_format_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.transcodeFormat")
                .placeholder("mp3, opus, flac")
                .default_value(state.settings.format.clone())
        });
        let volume_settings_inputs = [
            (state.settings.volume_width, "setting.volumeWidth"),
            (
                state.settings.artist_radio_count as f32,
                "setting.artistRadioCount",
            ),
            (state.settings.button_size, "setting.buttonSize"),
        ]
        .map(|(value, label)| {
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .aria_label(label)
                    .default_value(value.to_string())
            })
        });
        let volume_wheel_slider = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(20.)
                .step(1.)
                .default_value(state.settings.volume_wheel_step)
        });
        let fullscreen_idle_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Fullscreen idle delay (minutes)")
                .default_value(state.settings.fullscreen_auto_open_minutes.to_string())
        });
        let queue_autosave_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Queue autosave count")
                .default_value(state.settings.queue_autosave_count.max(1).to_string())
        });
        let preset_name = input("setting.equalizerPresetNamePlaceholder", window, cx);
        let compressor_preset_name = input("setting.equalizerPresetNamePlaceholder", window, cx);
        let mpv_device = input("MPV audio device identifier (auto = default)", window, cx);
        let mpv_options = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("MPV options")
                .multi_line(true)
                .rows(8)
        });
        mpv_options.update(cx, |input, cx| input.set_value(serde_json::to_string_pretty(&serde_json::json!({"arguments":state.settings.mpv_extra_args,"properties":state.settings.mpv_properties})).unwrap(), window, cx));
        let ffmpeg_path = input("FFmpeg binary path for speaker speed and DSP", window, cx);
        ffmpeg_path.update(cx, |input, cx| {
            input.set_value(state.settings.ffmpeg_path.clone(), window, cx)
        });
        mpv_device.update(cx, |input, cx| {
            input.set_value(state.settings.mpv_audio_device.clone(), window, cx)
        });
        let path_replace = input("setting.pathReplace_optionRemovePrefix", window, cx);
        path_replace.update(cx, |input, cx| {
            input.set_value(state.settings.path_replace.clone(), window, cx)
        });
        let path_replace_with = input("setting.pathReplace_optionAddPrefix", window, cx);
        path_replace_with.update(cx, |input, cx| {
            input.set_value(state.settings.path_replace_with.clone(), window, cx)
        });
        mpv_path.update(cx, |input, cx| {
            input.set_value(state.settings.mpv_path.clone(), window, cx)
        });
        let playlist_folder_indent_input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.sidebarPlaylistFolderTreeIndent")
                .default_value(state.settings.playlist_folder_indent.to_string())
        });
        let playlist_separator = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.sidebarPlaylistFolderSeparator")
        });
        playlist_separator.update(cx, |input, cx| {
            input.set_value(state.settings.playlist_separator.clone(), window, cx)
        });
        let playlist_folder_color = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("setting.sidebarPlaylistFolderTreeLineColor")
        });
        playlist_folder_color.update(cx, |input, cx| {
            input.set_value(state.settings.playlist_folder_color.clone(), window, cx)
        });
        let sleep_inputs = [
            ("datetime.hourLong", 0),
            ("datetime.minuteLong", 20),
            ("datetime.secondLong", 0),
        ]
        .map(|(label, value)| {
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .aria_label(label)
                    .default_value(value.to_string())
            })
        });
        let sidebar_width = input("Sidebar width", window, cx);
        let sidebar_regex = input(
            "Hide playlist names matching a regular expression",
            window,
            cx,
        );
        sidebar_width.update(cx, |input, cx| {
            input.set_value(state.settings.sidebar.width.to_string(), window, cx)
        });
        sidebar_regex.update(cx, |input, cx| {
            input.set_value(
                state.settings.sidebar.playlist_filter_regex.clone(),
                window,
                cx,
            )
        });
        let form_name = input("Name", window, cx);
        let milkdrop_search = input("Find a MilkDrop preset", window, cx);
        let form_comment = input("Description or stream URL", window, cx);
        let form_rules = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .placeholder("Navidrome smart playlist rules (JSON, optional)")
                .multi_line(true)
        });
        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .default_value(state.settings.volume)
        });
        let progress = cx.new(|_| SliderState::new().min(0.0).max(0.0).step(1.0));
        let equalizer = state
            .settings
            .equalizer
            .iter()
            .map(|gain| {
                cx.new(|_| {
                    SliderState::new()
                        .min(-12.0)
                        .max(12.0)
                        .step(0.5)
                        .default_value(*gain)
                })
            })
            .collect();
        let equalizer_inputs = state
            .settings
            .equalizer
            .iter()
            .enumerate()
            .map(|(index, gain)| {
                let input = input(
                    &format!("{} Hz", feishin_core::dsp::FREQUENCIES[index]),
                    window,
                    cx,
                );
                input.update(cx, |input, cx| {
                    input.set_value(gain.to_string(), window, cx)
                });
                input
            })
            .collect();
        let preamp_slider = cx.new(|_| {
            SliderState::new()
                .min(-12.)
                .max(12.)
                .step(0.5)
                .default_value(state.settings.equalizer_preamp)
        });
        let audio_preset_selects = std::array::from_fn(|index| {
            cx.new(|cx| {
                crate::equalizer::PresetSelect::new(
                    crate::equalizer::preset_options(&state.settings, index == 1),
                    None,
                    window,
                    cx,
                )
                .searchable(true)
            })
        });
        let auto_dj_numbers = [
            state.settings.auto_dj_config.item_count,
            state.settings.auto_dj_config.timing,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let input = input(
                if index == 0 {
                    "setting.autoDJ_itemCount"
                } else {
                    "setting.autoDJ_timing"
                },
                window,
                cx,
            );
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
            input
        })
        .collect();
        let auto_dj_folder = input("Auto DJ music library ID (optional)", window, cx);
        auto_dj_folder.update(cx, |input, cx| {
            input.set_value(
                state
                    .settings
                    .auto_dj_config
                    .music_folder
                    .clone()
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        let (media_tx, media_events) = std::sync::mpsc::channel();
        let c = state.settings.compressor;
        let audio_numbers = [
            state.settings.equalizer_preamp,
            c.threshold,
            c.ratio,
            c.attack,
            c.release,
            c.makeup,
            c.knee,
            state.settings.scrobble_percentage,
            state.settings.scrobble_seconds,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let input = input(crate::library::AUDIO_NUMBERS[index].0, window, cx);
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
            input
        })
        .collect();
        let compressor_sliders = [c.threshold, c.ratio, c.attack, c.release, c.makeup, c.knee]
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let (_, min, max, step) = crate::library::AUDIO_NUMBERS[index + 1];
                cx.new(|_| {
                    SliderState::new()
                        .min(min)
                        .max(max)
                        .step(step)
                        .default_value(value)
                })
            })
            .collect();
        let scrobble_percentage_slider = cx.new(|_| {
            SliderState::new()
                .min(25.)
                .max(90.)
                .step(1.)
                .default_value(state.settings.scrobble_percentage)
        });
        let window_bar_style_active = state.settings.window_bar_style.clone();
        let player_expanded = state.settings.player_display.expanded
            && !state.settings.player_display.visualizer_expanded;
        let visualizer_display = crate::visualizer_display::State::restored(
            state.settings.player_display.visualizer_expanded,
        );
        let mut view = Self {
            server_discovery: Default::default(),
            server_check: Default::default(),
            app_menu: None,
            instance: None,
            #[cfg(windows)]
            taskbar: None,
            #[cfg(windows)]
            taskbar_attempted: false,
            cast_devices: vec![],
            audio_devices: vec![],
            audio_devices_generation: 0,
            audio_devices_source: None,
            queue_drawer: false,
            queue_drawer_generation: 0,
            side_queue_search: cx
                .new(|cx| crate::appearance::input_state(window, cx).placeholder("Search queue")),
            side_queue_toolbar_width: 0.,
            queue_toolbar_width: 0.,
            side_queue_search_open: false,
            side_queue_filter: String::new(),
            side_queue_search_debounce: None,
            queue_search_open: false,
            side_queue_horizontal_scroll: ScrollHandle::new(),
            side_queue_scroll: UniformListScrollHandle::new(),
            side_queue_group_scroll: ListState::new(0, ListAlignment::Top, px(0.)),
            queue_group_scroll: ListState::new(0, ListAlignment::Top, px(0.)),
            queue_follow_signature: (None, false, false),
            resumed_entry: None,
            queue_autosave: Default::default(),
            queue_save_busy: false,
            queue_restore_busy: false,
            pending_queue_change: None,
            queue_prompt_open: false,
            pending_library_fetch: None,
            library_fetch_prompt_open: false,
            library_fetches: Default::default(),
            hovered_table_row: None,
            favorite_requests: 0,
            rating_requests: 0,
            volume_sync_pending: false,
            volume_change_task: None,
            notifications: Default::default(),
            queue_autosave_input,
            accent_input,
            artist_tab: ArtistTab::Albums,
            artist_overview: true,
            artist_sections: Default::default(),
            artist_section_generation: 0,
            artist_searches,
            artist_info: None,
            artist_info_error: String::new(),
            artist_info_target: None,
            artist_role: None,
            cast_selected: None,
            cast_connecting: None,
            cast_output_open: false,
            cast_builder: None,
            cast_group_pending: None,
            cast_discovery_generation: 0,
            cast_discovery_groups: vec![],
            cast_scanning: false,
            cast_groups: vec![],
            cast_members: HashMap::new(),
            cast_member_volumes: HashMap::new(),
            cast_volume_pending: Default::default(),
            cast_member_mutes: Default::default(),
            cast_topology_generation: 0,
            cast_property_versions: HashMap::new(),
            cast_property_loading: std::collections::HashSet::new(),
            cast_property_pending: HashMap::new(),
            cast_properties: HashMap::new(),
            visualizer_analyzer: Default::default(),
            visualizer_dimensions: std::rc::Rc::new(std::cell::Cell::new([1280., 540.])),
            system_audio: None,
            system_audio_ready: false,
            system_audio_error: None,
            visualizer_frame_at: Instant::now(),
            visualizer_spin: 0.,
            visualizer_fps: 0.,
            visualizer_settings: None,
            visualizer_numbers: vec![],
            visualizer_subscriptions: vec![],
            visualizer_gradient_editor: None,
            milkdrop: None,
            milkdrop_error: None,
            milkdrop_preset_at: Instant::now(),
            milkdrop_cycle_elapsed: 0.,
            milkdrop_search,
            info_items: vec![],
            info_generation: 0,
            info_loading: false,
            share_items: vec![],
            share_description,
            share_expiry,
            share_amount,
            share_url: None,
            share_busy: false,
            form_homepage,
            lyric_record: None,
            lyric_remote_suppressed: HashSet::new(),
            lyric_selections: HashMap::new(),
            lyric_song_offset,
            lyric_sidebar_offset,
            lyric_offset_target: None,
            lyric_target: None,
            lyric_text,
            lyric_busy: false,
            lyric_generation: 0,
            lyric_scroll: ScrollHandle::default(),
            sidebar_lyric_scroll: ScrollHandle::default(),
            sidebar_lyric_scroll_paused_until: None,
            lyric_settings_key: "default".into(),
            lyric_active: None,
            lyric_scroll_paused_until: None,
            lyric_scroll_motion: Default::default(),
            lyric_numbers: vec![],
            lyric_sliders: vec![],
            lyric_translation_key,
            lyric_translation_language,
            lyric_translation_region,
            lyric_translated: vec![],
            lyric_translation_visible: false,
            lyric_translation_busy: false,
            lyric_translation_generation: 0,
            lyric_annotations: vec![],
            annotation_generation: 0,
            presence: feishin_core::presence::Presence::new(),
            last_presence: Instant::now(),
            presence_artwork: HashMap::new(),
            presence_settings_inputs,
            theme_signature: String::new(),
            theme_palette: Default::default(),
            theme_overlays: Default::default(),
            last_theme_scan: Instant::now(),
            tag_files: vec![],
            tag_fields: vec![],
            tag_scope: None,
            tag_artwork: Default::default(),
            tag_artwork_path: None,
            tag_busy: false,
            tag_targets: vec![],
            tag_progress: None,
            tag_progress_events: None,
            tag_subscriptions: vec![],
            tag_suggestions: None,
            tag_source_names: vec![],
            tag_suggestion_generation: 0,
            tag_config_edit: None,
            remote: None,
            remote_port,
            remote_generation: 0,
            media: None,
            media_metadata: None,
            media_events,
            media_sender: media_tx,
            media_attempted: false,
            window_bar_style_active,
            secret_form_route: None,
            navigation_generation: 0,
            route: if state.active_server.is_some() {
                Route::Home
            } else {
                Route::Servers
            },
            state,
            updates: Default::default(),
            cache_busy: false,
            library_scan: Default::default(),
            query_cache_generation: 0,
            album_related: vec![],
            album_related_generation: 0,
            release_notes: Default::default(),
            data_dir,
            history: vec![],
            navigation_restore: None,
            page_scroll: ScrollHandle::new(),
            sessions: HashMap::new(),
            items: vec![],
            random_sample: None,
            album_tracks: HashMap::new(),
            expanded_album: None,
            random_form: None,
            album_metadata_cache: HashMap::new(),
            album_group_size,
            album_tracks_generation: 0,
            album_details_scroll: ListState::new(0, ListAlignment::Top, px(300.)),
            detail_columns: 1,
            detail_header_width: px(0.),
            library_width: px(0.),
            album_metadata_extra: px(0.),
            selected_album_tracks: HashSet::new(),
            album_selection_order: vec![],
            selected: HashSet::new(),
            grid_selection_order: vec![],
            total: None,
            has_more: false,
            loading: false,
            message,
            sort: Sort::Name,
            descending: false,
            list_editor: None,
            column_resize_preview: None,
            column_resize_origin: None,
            library_page: 0,
            active_filters: Default::default(),
            filter_editor: None,
            filter_container_width: px(0.),
            playback_filter_inputs: Default::default(),
            filter_subscriptions: vec![],
            filter_generation: 0,
            filter_input_sync: false,
            list_horizontal_scroll: ScrollHandle::new(),
            library_scroll: UniformListScrollHandle::new(),
            track_scroll_target: None,
            queue_display_sort: None,
            settings_search: input("Search settings", window, cx),
            settings_search_open: false,
            settings_tab_focus: std::array::from_fn(|_| cx.focus_handle()),
            search,
            search_debounce: None,
            local_search,
            local_search_debounce: None,
            local_filter: String::new(),
            server_name,
            server_url,
            server_remote_url,
            server_libraries,
            server_options: ServerOptions {
                remote_url: server_defaults.remote_url.clone(),
                ..Default::default()
            },
            editing_server: None,
            server_library_choices: Vec::new(),
            username,
            password,
            remote_username,
            remote_password,
            server_kind: server_defaults.kind,
            quick_connect: Default::default(),
            signing_out: Default::default(),
            legacy_auth: server_defaults.legacy_auth,
            server_defaults,
            remember: true,
            mpv_path,
            mpv_device,
            mpv_options,
            preset_name,
            compressor_preset_name,
            audio_preset_selects,
            settings_import: None,
            settings_import_loading: false,
            settings_import_generation: 0,
            settings_import_error: String::new(),
            ffmpeg_path,
            path_replace,
            path_replace_with,
            path_previews: HashMap::new(),
            collection_draft: None,
            collection_rename: None,
            form_name,
            form_comment,
            form_rules,
            smart_editor: None,
            query_field_inputs: vec![],
            query_field_types: vec![],
            css_input,
            css_file_signature: None,
            playlist_options: vec![],
            playlist_tracks_edit: None,
            playlist_users: vec![],
            playlist_loading: false,
            playlist_generation: 0,
            playlist_folder_path: String::new(),
            shared_playlist_folder_path: String::new(),
            playlist_folder_shared: false,
            folder_browser: Default::default(),
            sidebar_playlists_open: true,
            sidebar_resize: None,
            sidebar_closed_sections: HashSet::from(["shared-playlists"]),
            sidebar_playlist_hover: None,
            sidebar_folder_drag_expanded: HashSet::new(),
            playlist_separator,
            playlist_folder_color,
            playlist_folder_indent_input,
            form_owner: String::new(),
            form_sync: None,
            form_public: false,
            form_songs: vec![],
            form_target: None,
            playlist_saving: false,
            volume,
            equalizer,
            equalizer_inputs,
            preamp_slider,
            audio_numbers,
            compressor_sliders,
            scrobble_percentage_slider,
            progress,
            seek_preview: Default::default(),
            progress_entry: None,
            lyrics: vec![],
            covers: HashMap::new(),
            cover_versions: HashMap::new(),
            image_placeholders: Default::default(),
            image_resolution_inputs,
            image_resolution_open: false,
            censored_covers: HashMap::new(),
            detail_background: None,
            detail_background_key: String::new(),
            last_user_activity: Instant::now(),
            fullscreen_idle_state: (false, 0),
            fullscreen_idle_input,
            volume_settings_inputs,
            volume_wheel_slider,
            speed_input,
            sample_rate_input,
            bitrate_input,
            transcode_format_input,
            player_numbers: vec![],
            player_background: None,
            player_background_key: String::new(),
            player_title_at: Instant::now(),
            waveform_sliders: Vec::new(),
            waveform_data: None,
            waveform_key: String::new(),
            waveform_generation: 0,
            waveform_job: None,
            waveform_dragging: false,
            waveform_preview: None,
            waveform_bounds: Default::default(),
            full_player: player_expanded,
            rendering_full_player: None,
            visualizer_display,
            player_related: Default::default(),
            queue_scroll: UniformListScrollHandle::default(),
            queue_selected: HashSet::new(),
            queue_selection_order: vec![],
            side_queue_selected: HashSet::new(),
            side_queue_selection_order: vec![],
            forward_history: vec![],
            record_shortcut: None,
            global_shortcuts: Default::default(),
            last_played: None,
            previous_played: None,
            radio_item: None,
            radio_position: 0.0,
            radio_metadata: Default::default(),
            playing: false,
            buffering: false,
            loaded: false,
            sleep_timer: None,
            sleep_inputs,
            auto_dj_busy: false,
            auto_dj_last: None,
            auto_dj_generation: 0,
            auto_dj_numbers,
            auto_dj_folder,
            player: Player::new(),
            jukebox_server: None,
            power: Default::default(),
            player_generation: 0,
            request_generation: 0,
            play_request_generation: 0,
            listening: Default::default(),
            track_reports: Default::default(),
            report_task: None,
            last_position: 0.0,
            tray: None,
            tray_error: None,
            window_hidden: false,
            tray_initialized: false,
            dock_language: String::new(),
            home_sections: Default::default(),
            home_generation: 0,
            home_feature_width: px(0.),
            home_feature_wheel_at: None,
            home_backgrounds: Default::default(),
            loaded_font_signature: String::new(),
            custom_font_family: String::new(),
            sidebar_width,
            sidebar_regex,
            native_hide_supported: cfg!(test),
            lyric_position_at: Instant::now(),
            last_save: Instant::now(),
            last_report: Instant::now(),
            last_seek_report: None,
            save_enabled,
            focus: cx.focus_handle(),
            library_focus: cx.focus_handle(),
            sidebar_search_focus: cx.focus_handle(),
            related_focus: cx.focus_handle(),
            artist_song_focus: std::array::from_fn(|_| cx.focus_handle()),
            queue_focus: cx.focus_handle(),
            side_queue_focus: cx.focus_handle(),
            subscriptions: vec![],
            command_palette: None,
            add_to_playlist: None,
        };
        view.subscriptions
            .push(cx.observe_window_bounds(window, |this, window, _| this.remember_window(window)));
        view.subscriptions
            .push(view.observe_duplicate_shortcuts(window, cx));
        view.subscriptions
            .push(cx.on_focus_lost(window, |this, window, _| window.focus(&this.focus)));
        view.subscriptions
            .push(cx.on_blur(&view.focus, window, |this, _, cx| {
                if this.record_shortcut.take().is_some() {
                    cx.notify();
                }
            }));
        view.subscriptions.push(cx.on_app_quit(|this, _| {
            this.save();
            this.install_update_on_quit();
            async {}
        }));
        view.subscriptions.push(cx.subscribe_in(
            &view.settings_search,
            window,
            |this, input, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Focus if this.route == Route::Settings => {
                        this.settings_search_open = true
                    }
                    InputEvent::Blur if input.read(cx).value().is_empty() => {
                        this.settings_search_open = false
                    }
                    _ => {}
                }
                cx.notify();
            },
        ));
        for input in [
            &view.server_name,
            &view.server_url,
            &view.server_remote_url,
            &view.server_libraries,
            &view.username,
            &view.password,
        ] {
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.cancel_quick_connect();
                        cx.notify();
                    }
                },
            ));
        }
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |this, cx| this.prepare_window_close(window, cx))
                .unwrap_or(true)
        });
        view.remember_window(window);
        view.restore_remote(window, cx);
        view.install_shortcuts(cx);
        window.focus(&view.focus);
        view.apply_theme(window, cx);
        view.subscriptions.push(cx.subscribe_in(
            &view.milkdrop_search,
            window,
            |_, _, _: &InputEvent, _, cx| cx.notify(),
        ));
        view.subscriptions.push(cx.subscribe_in(
            &view.form_name,
            window,
            |this, input, event, window, cx| {
                if this.route == Route::TagEditor {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.add_tag_field(&input.read(cx).value(), window, cx);
                    }
                    cx.notify();
                }
            },
        ));
        view.subscriptions.push(cx.subscribe_in(
            &view.side_queue_search.clone(),
            window,
            |this, input, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Change => {
                        let query = input.read(cx).value().to_string();
                        let timer = cx
                            .background_executor()
                            .timer(std::time::Duration::from_millis(200));
                        this.side_queue_search_debounce = Some(cx.spawn(async move |view, cx| {
                            timer.await;
                            view.update(cx, |view, cx| {
                                view.side_queue_filter = query;
                                cx.notify();
                            })
                            .ok();
                        }));
                    }
                    InputEvent::Focus => this.side_queue_search_open = true,
                    InputEvent::Blur if input.read(cx).value().is_empty() => {
                        this.side_queue_search_open = false
                    }
                    _ => {}
                }
                cx.notify();
            },
        ));
        view.player.send(Control::Configure {
            settings: Box::new(view.state.settings.clone()),
            gain: 1.0,
        });
        view.init_lyric_numbers(window, cx);
        view.init_player_numbers(window, cx);
        view.init_auto_dj_numbers(window, cx);
        view.init_number_steps(window, cx);
        view.init_equalizer_controls(window, cx);
        for (index, input) in view.image_resolution_inputs.clone().iter().enumerate() {
            view.subscriptions
                .extend(Self::bind_number_step(input, 0., 2000., 1., window, cx));
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur)
                        && let Ok(value) = input.read(cx).value().parse::<f64>()
                        && value.is_finite()
                    {
                        let clamped = value.clamp(0., 2000.).to_string();
                        if clamped != input.read(cx).value().as_str() {
                            input.update(cx, |input, cx| input.set_value(clamped, window, cx));
                        }
                    }
                    if matches!(
                        event,
                        InputEvent::Change | InputEvent::Blur | InputEvent::PressEnter { .. }
                    ) {
                        this.apply_image_resolution(index, cx);
                    }
                },
            ));
        }
        view.subscriptions.push(cx.subscribe_in(
            &view.volume_wheel_slider,
            window,
            |this, _, event: &SliderEvent, _, cx| {
                if let SliderEvent::ChangeEnd(value) = event {
                    this.state.settings.volume_wheel_step = value.end().clamp(1., 20.);
                    this.save();
                    cx.notify();
                }
            },
        ));
        for (index, input) in view.volume_settings_inputs.clone().iter().enumerate() {
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.apply_control_number(index, cx);
                    }
                },
            ));
        }
        view.subscriptions.push(cx.subscribe_in(
            &view.scrobble_percentage_slider,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.state.settings.scrobble_percentage = value.end().clamp(25., 90.);
                    this.audio_numbers[7].update(cx, |input, cx| {
                        input.set_value(
                            this.state.settings.scrobble_percentage.to_string(),
                            window,
                            cx,
                        )
                    });
                    this.save();
                    cx.notify();
                }
            },
        ));
        for (index, input) in view.audio_numbers.clone().iter().enumerate() {
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event, window, cx| {
                    if index == 8 && matches!(event, InputEvent::Change) {
                        let (_, min, max, _) = crate::library::AUDIO_NUMBERS[index];
                        if let Ok(value) = this.audio_numbers[index].read(cx).value().parse::<f32>()
                            && value.is_finite()
                            && (min..=max).contains(&value)
                        {
                            this.state.settings.scrobble_seconds = value;
                            this.save();
                            cx.notify();
                        }
                    } else if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        this.apply_audio_number(index, window, cx);
                        if index <= 6 && matches!(event, InputEvent::PressEnter { .. }) {
                            window.focus(&this.focus);
                        }
                    }
                },
            ));
            view.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let (_, min, max, step) = crate::library::AUDIO_NUMBERS[index];
                    let input = this.audio_numbers[index].clone();
                    let value = input.read(cx).value().parse::<f32>().unwrap_or(0.0);
                    let value = (value
                        + if *action == StepAction::Increment {
                            step
                        } else {
                            -step
                        })
                    .clamp(min, max);
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                    this.apply_audio_number(index, window, cx);
                },
            ));
        }
        for (index, slider) in view.compressor_sliders.clone().iter().enumerate() {
            view.subscriptions.push(cx.subscribe_in(
                slider,
                window,
                move |this, _, event, window, cx| {
                    let (value, commit) = match event {
                        SliderEvent::Change(value) => (value.end(), false),
                        SliderEvent::ChangeEnd(value) => (value.end(), true),
                    };
                    this.set_compressor_value(index, value, commit, window, cx);
                },
            ));
        }
        view.subscriptions.push(cx.subscribe_in(
            &view.local_search,
            window,
            |this, input, event, _, cx| {
                match event {
                    InputEvent::Focus => this.queue_search_open = true,
                    InputEvent::Blur if input.read(cx).value().is_empty() => {
                        this.queue_search_open = false
                    }
                    _ => {}
                }
                if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                    cx.notify();
                }
                let query = input.read(cx).value().to_string();
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_local_filter(query, cx);
                } else if matches!(event, InputEvent::Change) {
                    this.local_search_debounce = None;
                    if query != this.local_filter
                        && (this.route == Route::Queue || this.has_local_filter())
                    {
                        let route = this.route.clone();
                        let timer =
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(
                                    if route == Route::Queue { 200 } else { 300 },
                                ));
                        this.local_search_debounce = Some(cx.spawn(async move |this, cx| {
                            timer.await;
                            this.update(cx, |this, cx| {
                                if this.route == route {
                                    this.submit_local_filter(query, cx);
                                }
                            })
                            .ok();
                        }));
                    }
                }
            },
        ));
        view.subscriptions.push(
            cx.subscribe_in(&view.search, window, |this, _, event, _, cx| {
                let query = this.search.read(cx).value().to_string();
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_search(query, cx);
                } else if matches!(event, InputEvent::Change)
                    && matches!(&this.route, Route::Search(term, _) if *term != query || this.search_debounce.is_some())
                {
                    this.request_generation += 1;
                    let generation = this.request_generation;
                    let timer = cx.background_executor().timer(std::time::Duration::from_millis(200));
                    this.search_debounce = Some(cx.spawn(async move |this, cx| {
                        timer.await;
                        this.update(cx, |this, cx| {
                            if this.request_generation == generation
                                && matches!(this.route, Route::Search(..))
                            {
                                this.submit_search(query, cx);
                            }
                        })
                        .ok();
                    }));
                }
            }),
        );
        for (index, input) in view.artist_searches.clone().into_iter().enumerate() {
            view.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event, _, cx| {
                    if matches!(event, gpui_component::input::InputEvent::Change) {
                        let id = ["topSongs", "favoriteSongs", "recentAlbums"][index];
                        let query = this.artist_searches[index].read(cx).value().to_string();
                        if let Some(section) = this.artist_sections.get_mut(id) {
                            let timer = cx
                                .background_executor()
                                .timer(std::time::Duration::from_millis(300));
                            section.search_task = Some(cx.spawn(async move |view, cx| {
                                timer.await;
                                view.update(cx, |view, cx| {
                                    if let Some(section) = view.artist_sections.get_mut(id) {
                                        section.search = query;
                                        section.page = 0;
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }));
                        }
                        cx.notify();
                    }
                },
            ));
        }
        view.subscriptions.push(cx.subscribe_in(
            &view.volume,
            window,
            |this, _, event, window, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                this.change_master_volume(value.end(), window.modifiers().shift, cx);
            },
        ));
        view.subscriptions.push(cx.subscribe_in(
            &view.progress,
            window,
            |this, _, event, window, cx| {
                if this
                    .current_entry()
                    .is_none_or(|entry| entry.item.kind != ItemKind::Song)
                {
                    return;
                }
                match event {
                    SliderEvent::Change(value) => this.seek_preview.change(f64::from(value.end())),
                    SliderEvent::ChangeEnd(value) => {
                        let target = f64::from(value.end());
                        let current = this.current_position();
                        this.seek_position(target, window, cx);
                        this.seek_preview.release(target, current, Instant::now());
                    }
                }
                cx.notify();
            },
        ));
        view.album_details_scroll.set_scroll_handler(cx.listener(
            |this, event: &ListScrollEvent, _, cx| {
                if event.is_scrolled
                    && event.visible_range.end + 1 >= event.count
                    && this.has_more
                    && !this.loading
                    && this.list_config().pagination == "infinite"
                {
                    this.reload(true, cx);
                }
            },
        ));
        view.init_dock(window, cx);
        view.init_app_menu(window, cx);
        for input in [&view.sidebar_width, &view.sidebar_regex] {
            view.subscriptions
                .push(cx.subscribe_in(input, window, |this, _, event, _, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.apply_sidebar(cx);
                    }
                }));
        }
        view.restore_view();
        let servers = view
            .state
            .servers
            .iter()
            .filter(|server| !view.state.signed_out_servers.contains(&server.id))
            .cloned()
            .collect::<Vec<_>>();
        let task = cx.background_executor().spawn(async move {
            servers
                .into_iter()
                .filter_map(|server| {
                    storage::load_session(&server)
                        .map(|session| (server, session))
                        .ok()
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let sessions = task.await;
            this.update(cx, |this, cx| {
                this.restore_server_sessions(sessions, cx);
            })
            .ok();
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(100)).await;
                if this
                    .update_in(cx, |this, window, cx| this.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        view
    }

    pub fn api(&self) -> Option<Api> {
        self.state
            .active_server
            .and_then(|id| self.sessions.get(&id))
            .cloned()
    }
    pub fn cover_key(item: &Item) -> String {
        feishin_core::api::artwork_key(item)
    }

    pub fn save(&mut self) {
        self.state.queue.up_next_enabled = self.state.settings.up_next_queue;
        self.state.queue.sync_up_next();
        self.arm_next();
        if let Some(remote) = &self.remote {
            remote.publish_with_artwork(
                &self.state,
                self.playing,
                &self.sessions,
                &self.data_dir,
                self.radio_state(),
            );
        }
        if self.save_enabled
            && let Err(error) = storage::save(&self.data_dir.join("state.json"), &self.state)
        {
            self.message = format!("Could not save settings: {error}");
            log::warn!("Settings save failed: {error}");
        }
    }

    pub fn direct_detail_search(&self) -> bool {
        matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album
            || (matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist)
                && self.artist_tab == feishin_core::model::ArtistTab::Albums))
    }

    pub fn has_local_filter(&self) -> bool {
        matches!(
            self.route,
            Route::Library(_)
                | Route::Collection(_)
                | Route::Favorites(_)
                | Route::Explore
                | Route::Detail(_)
                | Route::Similar(_)
        ) && !self.artist_overview_active()
    }

    pub fn submit_local_filter(&mut self, query: String, cx: &mut Context<Self>) {
        self.local_search_debounce = None;
        if self.route != Route::Queue && !self.has_local_filter() {
            return;
        }
        if self.local_filter == query {
            cx.notify();
            return;
        }
        self.local_filter = query;
        if self.route == Route::Queue {
            cx.notify();
        } else {
            self.reload(false, cx);
        }
    }

    fn submit_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_debounce = None;
        if let Route::Search(term, _) = &mut self.route {
            *term = query;
            self.reload(false, cx);
        } else if !query.trim().is_empty() {
            self.navigate(Route::Search(query, ItemKind::Song), cx);
        }
    }

    pub fn change_search_kind(&mut self, kind: ItemKind, cx: &mut Context<Self>) {
        if !matches!(self.route, Route::Search(..)) {
            return;
        }
        self.search_debounce = None;
        self.remember_view();
        self.route = Route::Search(self.search.read(cx).value().to_string(), kind);
        self.restore_view();
        self.reload(false, cx);
    }

    pub fn navigate(&mut self, mut route: Route, cx: &mut Context<Self>) {
        self.navigation_generation = self.navigation_generation.wrapping_add(1);
        if let Some(editor) = &mut self.filter_editor {
            editor.year_debounce = None;
        }
        self.hovered_table_row = None;
        if self.full_player {
            self.full_player = false;
            self.persist_player_modes();
        }
        self.discard_playlist_tracks_edit();
        self.remember_folder_route(&route);
        self.search_debounce = None;
        self.cancel_quick_connect();
        if let Route::Detail(item) = &mut route
            && item.kind == ItemKind::Album
        {
            item.playlist_songs.clear();
            if let Some(song_id) = item.id.strip_prefix("dummy/") {
                item.id = song_id.to_owned();
                item.kind = ItemKind::Song;
            }
        }
        self.remember_view();
        if self.route != route {
            self.artist_tab = ArtistTab::Albums;
            self.artist_overview = true;
        }
        self.forward_history.clear();
        self.history.push(self.current_visit());
        self.route = route;
        self.page_scroll.set_offset(point(px(0.), px(0.)));
        self.restore_view();
        self.list_horizontal_scroll
            .set_offset(point(px(0.), px(0.)));
        self.library_scroll
            .scroll_to_item_strict(0, ScrollStrategy::Top);
        self.selected.clear();
        self.grid_selection_order.clear();
        self.selected_album_tracks.clear();
        self.album_selection_order.clear();
        self.reload(false, cx);
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.filter_editor {
            editor.year_debounce = None;
        }
        if let Some(visit) = self.history.pop() {
            if self.full_player {
                self.full_player = false;
                self.persist_player_modes();
            }
            self.discard_playlist_tracks_edit();
            self.search_debounce = None;
            self.cancel_quick_connect();
            self.remember_view();
            self.forward_history.push(self.current_visit());
            self.remember_folder_route(&visit.route);
            self.restore_visit(visit, cx);
        }
    }

    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.filter_editor {
            editor.year_debounce = None;
        }
        if let Some(visit) = self.forward_history.pop() {
            if self.full_player {
                self.full_player = false;
                self.persist_player_modes();
            }
            self.discard_playlist_tracks_edit();
            self.search_debounce = None;
            self.cancel_quick_connect();
            self.remember_view();
            self.history.push(self.current_visit());
            self.remember_folder_route(&visit.route);
            self.restore_visit(visit, cx);
        }
    }

    pub fn reload(&mut self, more: bool, cx: &mut Context<Self>) {
        if !more {
            self.navigation_restore = None;
        }
        self.reload_with_position(more, cx);
    }

    pub(crate) fn reload_with_position(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.server_check.busy.is_some() {
            cx.notify();
            return;
        }
        if self.playlist_tracks_editing() {
            return;
        }
        self.discard_playlist_tracks_edit();
        self.track_scroll_target = None;
        if !more {
            self.album_related_generation = self.album_related_generation.wrapping_add(1);
            self.album_related.clear();
            if !self.artist_overview_active() {
                self.artist_sections.clear();
            }
            self.random_sample = None;
            self.album_tracks_generation += 1;
            self.album_tracks.clear();
            self.album_metadata_cache.clear();
            self.album_details_scroll.reset(0);
            self.album_metadata_extra = px(0.);
            self.library_page = self.restored_library_page();
            self.library_scroll
                .scroll_to_item_strict(0, ScrollStrategy::Top);
        }
        self.load_library(more, cx);
        if !more {
            self.load_detail_metadata(cx);
        }
    }
    pub fn load_library(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.playlist_tracks_editing() {
            return;
        }
        self.load_playlist_options(cx);
        self.prepare_folder_browser(cx);
        if self.artist_overview_active() {
            if !more {
                self.load_artist_overview(cx);
            }
            return;
        }
        if self.route == Route::Home {
            if !more {
                self.load_home(cx);
            }
            return;
        }
        self.remember_view();
        let config = self.list_config();
        let limit = if self.folder_route() {
            usize::MAX
        } else {
            config.items_per_page
        };
        self.request_generation += 1;
        let generation = self.request_generation;
        if !more {
            self.load_artist_info(cx);
        }
        let offset = if more {
            self.items.len()
        } else {
            self.items.clear();
            self.album_metadata_cache.clear();
            self.total = None;
            self.has_more = false;
            self.selected.clear();
            self.grid_selection_order.clear();
            self.selected_album_tracks.clear();
            self.album_selection_order.clear();
            if config.pagination == "paginated" {
                self.library_page.saturating_mul(limit)
            } else {
                0
            }
        };
        if !matches!(
            self.route,
            Route::Home
                | Route::Library(_)
                | Route::Collection(_)
                | Route::Favorites(_)
                | Route::Explore
                | Route::Search(..)
                | Route::Detail(_)
                | Route::Similar(_)
                | Route::PlaylistPicker
        ) {
            self.loading = false;
            cx.notify();
            return;
        }
        let route_api = if let Route::Similar(item) | Route::Detail(item) = &self.route {
            self.sessions.get(&item.server_id).cloned()
        } else {
            self.api()
        };
        let Some(api) = route_api else {
            self.loading = false;
            self.message = "Select a server and sign in to browse your library".into();
            cx.notify();
            return;
        };
        // Jellyfin's missing-album route loads the song itself, not album children.
        if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Song) {
            self.loading = false;
            self.message.clear();
            cx.notify();
            return;
        }
        if self.route == Route::Library(ItemKind::Radio) && api.server.kind == ServerType::Jellyfin
        {
            let stations = self
                .state
                .radio
                .iter()
                .filter(|item| item.server_id == api.server.id)
                .cloned()
                .collect();
            let page = feishin_core::model::filtered_page(
                feishin_core::lists::radio_items(
                    stations,
                    &self.local_filter,
                    self.sort,
                    self.descending,
                ),
                "",
                offset,
                self.restored_local_limit(limit),
            );
            self.total = page.total;
            self.has_more = page.has_more;
            self.items.extend(page.items);
            self.loading = false;
            cx.notify();
            return;
        }
        self.loading = true;
        self.message.clear();
        let route = self.route.clone();
        let sort = self.sort;
        let descending = self.descending;
        let artist_tab = self.artist_tab;
        let artist_role = self.artist_role.clone();
        let local_filter = self.local_filter.clone();
        let filters = self.active_filters.clone();
        let genre_albums = self.state.settings.genre_albums;
        let playlist_albums = self.state.settings.playlist_albums;
        let sample = self.random_sample.clone();
        let task = cx.background_executor().spawn(async move {
            if matches!(route, Route::Similar(_) | Route::Explore) {
                let mut sample = if let Some(sample) = sample {
                    sample
                } else {
                    let seed = if let Route::Similar(item) = &route {
                        Some(item.as_ref())
                    } else {
                        None
                    };
                    let mut sample = api.random_songs(100, seed)?;
                    feishin_core::lists::sort_items(&mut sample, sort, descending);
                    sample
                };
                let page = filters.page(sample.clone(), &local_filter, offset, limit);
                return Ok((page, Some(std::mem::take(&mut sample))));
            }
            if matches!(route, Route::Library(ItemKind::Folder))
                || matches!(&route, Route::Detail(item) if item.kind == ItemKind::Folder)
            {
                let parent = match &route {
                    Route::Detail(item) => Some(item.as_ref()),
                    _ => None,
                };
                let items = feishin_core::lists::folder_items(
                    api.folder_children(parent)?,
                    sort,
                    descending,
                    &local_filter,
                );
                return Ok((filters.page(items, "", offset, limit), None));
            }
            let page = match route {
                Route::Library(ItemKind::Radio) => {
                    let stations = api.all_items(ListQuery {
                        kind: ItemKind::Radio,
                        ..Default::default()
                    })?;
                    Ok(feishin_core::model::filtered_page(
                        feishin_core::lists::radio_items(stations, &local_filter, sort, descending),
                        "",
                        offset,
                        limit,
                    ))
                }
                Route::Search(term, kind) => api.search(
                    &ListQuery {
                        kind,
                        search: term,
                        offset,
                        limit,
                        sort,
                        descending,
                        filters: filters.clone(),
                        ..Default::default()
                    },
                    &local_filter,
                ),

                Route::Detail(item)
                    if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) =>
                {
                    let items = api.all_artist_items(&item, artist_tab)?;
                    let mut items = if artist_tab == ArtistTab::Songs {
                        items
                            .into_iter()
                            .filter(|item| item.matches(&local_filter) && filters.matches(item))
                            .collect()
                    } else {
                        filters.search(
                            items,
                            &local_filter,
                            if artist_tab == ArtistTab::Albums {
                                ItemKind::Album
                            } else if artist_tab == ArtistTab::Similar {
                                ItemKind::AlbumArtist
                            } else {
                                ItemKind::Song
                            },
                        )
                    };
                    if artist_tab == ArtistTab::Albums {
                        feishin_core::lists::sort_albums(&mut items, sort, descending);
                    } else if sort != Sort::Id
                        || !matches!(
                            artist_tab,
                            ArtistTab::CommunityTop
                                | ArtistTab::PersonalTop
                                | ArtistTab::Favorites
                                | ArtistTab::Rated
                        )
                    {
                        feishin_core::lists::sort_items(&mut items, sort, descending);
                    } else if descending {
                        items.reverse();
                    }
                    Ok(feishin_core::model::filtered_page(items, "", offset, limit))
                }
                Route::Detail(item) if item.kind == ItemKind::Playlist && playlist_albums => {
                    let mut songs =
                        filters.search(api.all_detail_items(&item)?, &local_filter, ItemKind::Song);
                    let random = sort == Sort::Random;
                    if local_filter.trim().is_empty() {
                        feishin_core::playlist_albums::sort_songs(
                            &mut songs,
                            if random { Sort::Album } else { sort },
                            descending,
                        );
                    }
                    let mut rows = feishin_core::playlist_albums::rows(songs);
                    if random {
                        feishin_core::lists::sort_items(&mut rows, Sort::Random, false);
                    }
                    Ok(feishin_core::model::filtered_page(rows, "", offset, limit))
                }
                Route::Detail(item) if item.kind == ItemKind::Genre => api.list(&api.genre_query(
                    &item,
                    ListQuery {
                        kind: if genre_albums {
                            ItemKind::Album
                        } else {
                            ItemKind::Song
                        },
                        search: local_filter,
                        offset,
                        limit,
                        sort,
                        descending,
                        filters,
                        ..Default::default()
                    },
                )?),
                Route::Detail(item) => {
                    let preserve_order =
                        (matches!(item.kind, ItemKind::Playlist | ItemKind::Album)
                            && sort == Sort::Id)
                            || (item.kind == ItemKind::Folder && sort == Sort::Name);
                    if preserve_order
                        && !descending
                        && local_filter.is_empty()
                        && filters.is_empty()
                    {
                        api.detail(&item, offset, limit)
                    } else {
                        let items = api.all_detail_items(&item)?;
                        let mut items = if matches!(item.kind, ItemKind::Album | ItemKind::Playlist)
                        {
                            filters.search(items, &local_filter, ItemKind::Song)
                        } else {
                            items
                                .into_iter()
                                .filter(|item| item.matches(&local_filter) && filters.matches(item))
                                .collect()
                        };
                        if item.kind == ItemKind::Playlist {
                            if local_filter.trim().is_empty() {
                                feishin_core::playlist_albums::sort_songs(
                                    &mut items, sort, descending,
                                );
                            }
                        } else {
                            feishin_core::lists::sort_items(&mut items, sort, descending);
                        }
                        Ok(feishin_core::model::filtered_page(items, "", offset, limit))
                    }
                }

                _ => {
                    let (kind, favorites) = match route {
                        Route::Library(kind) => (kind, false),
                        Route::Collection(ref collection) => (collection.kind, false),
                        Route::Favorites(kind) => (kind, true),
                        Route::PlaylistPicker => (ItemKind::Playlist, false),
                        _ => (ItemKind::Album, false),
                    };
                    api.list(&ListQuery {
                        kind,
                        search: local_filter,
                        offset,
                        limit,
                        sort: if matches!(route, Route::Home) {
                            Sort::Newest
                        } else {
                            sort
                        },
                        descending: descending || matches!(route, Route::Home),
                        favorites,
                        filters,
                        role: if kind == ItemKind::Artist {
                            artist_role
                        } else {
                            None
                        },
                        ..ListQuery::default()
                    })
                }
            };
            page.map(|page| (page, None))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.request_generation != generation {
                    return;
                }
                this.loading = false;
                match result {
                    Ok((page, sample)) => {
                        if sample.is_some() {
                            this.random_sample = sample;
                        }
                        this.total = page.total;
                        this.has_more = page.has_more;
                        this.items.extend(page.items);
                        this.fetch_covers(cx);
                        if this.needs_restore_page() {
                            this.load_library(true, cx);
                        }
                    }
                    Err(error) => {
                        this.has_more = false;
                        this.navigation_restore = None;
                        this.message = error.to_string();
                        log::warn!("Library request failed: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.cancel_quick_connect();
        let server = self.server_from_form(cx);
        if self.signing_out.contains(&server.id) {
            self.message = "Finishing sign out...".into();
            cx.notify();
            return;
        }
        let password = self.password.read(cx).value().to_string();
        let remember = self.remember;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.quick_connect.cancelled = cancelled.clone();
        self.quick_connect.password_busy = true;
        let generation = self.quick_connect.generation;
        self.loading = true;
        self.message = "Connecting...".into();
        let task = cx.background_executor().spawn(async move {
            let (api, _) = Api::authenticate(server, &password)?;
            crate::servers::prepare_login(api, remember, Some(&cancelled))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if this.quick_connect.generation != generation || !this.quick_connect.password_busy
                {
                    return;
                }
                this.quick_connect.password_busy = false;
                this.finish_login(result, window, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn finish_login(
        &mut self,
        result: anyhow::Result<(Api, Option<String>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.loading = false;
        match result {
            Ok((api, warning)) => {
                self.server_check.cancel();
                if self.remember {
                    self.state.signed_out_servers.remove(&api.server.id);
                } else {
                    self.state.signed_out_servers.insert(api.server.id);
                }
                self.reset_library_scan();
                self.reset_playlists();
                self.state.active_server = Some(api.server.id);
                if let Some(server) = self
                    .state
                    .servers
                    .iter_mut()
                    .find(|server| server.id == api.server.id)
                {
                    *server = api.server.clone();
                } else {
                    self.state.servers.push(api.server.clone());
                }
                self.sessions.insert(api.server.id, api);
                self.password
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.save();
                self.navigate(Route::Home, cx);
                if let Some(warning) = warning {
                    self.message = warning;
                }
            }
            Err(error) => {
                self.message = error.to_string();
            }
        }
        cx.notify();
    }

    pub fn choose_server(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.reset_library_scan();
        self.remember_view();
        self.reset_playlists();
        self.state.active_server = Some(id);
        self.route = Route::Servers;
        self.save();
        self.verify_active_server(cx);
        self.navigate(Route::Home, cx);
    }

    pub fn perform(
        &mut self,
        job: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        success: &'static str,
        cx: &mut Context<Self>,
    ) {
        let task = cx.background_executor().spawn(async move { job() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.message = match result {
                    Ok(()) => success.into(),
                    Err(error) => error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn enqueue(&mut self, item: Item, mode: Enqueue, cx: &mut Context<Self>) {
        self.enqueue_from(item, mode, None, None, false, cx);
    }
    pub fn enqueue_items(
        &mut self,
        mut items: Vec<Item>,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            return;
        }
        if items.iter().all(|item| item.kind == ItemKind::Song) {
            if shuffle {
                use rand::seq::SliceRandom;
                items.shuffle(&mut rand::thread_rng());
            }
            self.add_tracks(items, mode, cx);
            return;
        }
        if items.len() == 1 {
            self.enqueue_from(items.remove(0), mode, None, None, shuffle, cx);
            return;
        }
        // The original merged multi-playlist request has no single playlist source.
        let infer_context = !items.iter().all(|item| item.kind == ItemKind::Playlist);
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(request, cancelled.clone());
        let sessions = self
            .sessions
            .iter()
            .map(|(id, api)| (*id, api.clone().with_cancellation(cancelled.clone())))
            .collect::<HashMap<_, _>>();
        let task = cx.background_executor().spawn(async move {
            let mut songs = Vec::new();
            for item in items {
                songs.extend(
                    sessions
                        .get(&item.server_id)
                        .ok_or_else(|| anyhow::anyhow!("Sign in to this item's server"))?
                        .all_tracks(&item)?,
                );
            }
            anyhow::ensure!(!songs.is_empty(), "No playable tracks found");
            if shuffle {
                use rand::seq::SliceRandom;
                songs.shuffle(&mut rand::thread_rng());
            }
            Ok::<_, anyhow::Error>(songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.library_fetches.remove(&request);
                cx.notify();
                if cancelled.load(std::sync::atomic::Ordering::Relaxed)
                    || (matches!(mode, Enqueue::Now) && request != this.play_request_generation)
                {
                    return;
                }
                match result {
                    Ok(songs) => {
                        let context = infer_context
                            .then(|| this.inferred_playlist_context(mode))
                            .flatten();
                        this.add_tracks_in_context(songs, mode, None, context.as_deref(), cx);
                    }
                    Err(error) => this.message = error.to_string(),
                };
            })
            .ok();
        })
        .detach();
    }
    fn enqueue_from(
        &mut self,
        item: Item,
        mode: Enqueue,
        start: Option<Item>,
        radio_count: Option<(usize, bool)>,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if radio_count.is_none()
            && matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist)
            && matches!(&self.route, Route::Detail(artist) if artist.id == item.id && artist.server_id == item.server_id)
            && self.artist_tab == ArtistTab::Albums
        {
            self.enqueue_visible_from(mode, None, shuffle, cx);
            return;
        }
        if item.kind == ItemKind::Radio {
            self.add_tracks(vec![item], mode, cx);
            return;
        }
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            self.message = "Sign in to this track's server".into();
            return;
        };
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(request, cancelled.clone());
        let api = api.with_cancellation(cancelled.clone());
        let playlist_context =
            (radio_count.is_none() && item.kind == ItemKind::Playlist).then(|| item.id.clone());
        let task = cx.background_executor().spawn(async move {
            let mut songs = if let Some((count, skip_first)) = radio_count {
                let mut songs = api.random_songs(count, Some(&item))?;
                anyhow::ensure!(!songs.is_empty(), "No related tracks found");
                if item.kind == ItemKind::Song && (!skip_first || matches!(mode, Enqueue::Now)) {
                    songs.insert(0, item.clone());
                }
                songs
            } else {
                api.all_tracks(&item)?
            };
            if shuffle {
                use rand::seq::SliceRandom;
                songs.shuffle(&mut rand::thread_rng());
            }
            Ok::<_, anyhow::Error>(songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.library_fetches.remove(&request);
                cx.notify();
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if matches!(mode, Enqueue::Now) && request != this.play_request_generation {
                    return;
                }
                match result {
                    Ok(songs) => {
                        let context =
                            playlist_context.or_else(|| this.inferred_playlist_context(mode));
                        this.add_tracks_in_context(
                            songs,
                            mode,
                            start.as_ref(),
                            context.as_deref(),
                            cx,
                        );
                        if !matches!(mode, Enqueue::Now) {
                            this.message = "Added to queue".into();
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn enqueue_artist_radio(
        &mut self,
        item: Item,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
            self.enqueue_from(
                item,
                mode,
                None,
                Some((self.state.settings.artist_radio_count.clamp(1, 500), false)),
                shuffle,
                cx,
            );
        }
    }
    pub fn enqueue_related_radio(
        &mut self,
        item: Item,
        mode: Enqueue,
        skip_first: bool,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if matches!(item.kind, ItemKind::Song | ItemKind::Album) {
            self.enqueue_from(
                item,
                mode,
                None,
                Some((
                    self.state.settings.artist_radio_count.clamp(1, 500),
                    skip_first,
                )),
                shuffle,
                cx,
            );
        }
    }
    pub fn play_visible(&mut self, cx: &mut Context<Self>) {
        let behavior = self.state.settings.play_button_behavior;
        self.enqueue_visible_from(
            behavior.mode(),
            None,
            behavior == feishin_core::queue::PlayBehavior::Shuffle,
            cx,
        );
    }

    pub fn enqueue_visible(&mut self, mode: Enqueue, cx: &mut Context<Self>) {
        self.enqueue_visible_from(mode, None, false, cx);
    }
    pub fn enqueue_visible_shuffled(&mut self, mode: Enqueue, cx: &mut Context<Self>) {
        self.enqueue_visible_from(mode, None, true, cx);
    }
    pub fn play_library_item(&mut self, item: Item, cx: &mut Context<Self>) {
        self.play_library_item_with_shuffle(item, false, cx);
    }
    pub fn shuffle_library_item(&mut self, item: Item, cx: &mut Context<Self>) {
        self.play_library_item_with_shuffle(item, true, cx);
    }
    fn play_library_item_with_shuffle(
        &mut self,
        item: Item,
        force_shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if item.kind == ItemKind::Radio {
            self.enqueue(item, Enqueue::Now, cx);
            return;
        }
        let behavior = self.state.settings.play_button_behavior;
        let mode = behavior.mode();
        let shuffle = force_shuffle || behavior == feishin_core::queue::PlayBehavior::Shuffle;
        if item.kind == ItemKind::Song && self.folder_route() {
            let mut tracks = self
                .items
                .iter()
                .filter(|item| item.kind == ItemKind::Song)
                .cloned()
                .collect::<Vec<_>>();
            if shuffle {
                use rand::seq::SliceRandom;
                tracks.shuffle(&mut rand::thread_rng());
            }
            self.add_tracks_from(tracks, mode, Some(&item), cx);
            return;
        }
        if item.kind == ItemKind::Song {
            self.add_tracks_from(vec![item.clone()], mode, Some(&item), cx);
        } else {
            self.enqueue_from(item, mode, None, None, shuffle, cx);
        }
    }
    fn enqueue_visible_from(
        &mut self,
        mode: Enqueue,
        start: Option<Item>,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if self.needs_library_fetch_confirmation(start.is_some()) {
            self.play_request_generation += 1;
            self.pending_library_fetch = Some(crate::radio::PendingLibraryFetch {
                generation: self.play_request_generation,
                route: self.route.clone(),
                mode,
                start,
                shuffle,
            });
            cx.notify();
            return;
        }
        self.enqueue_visible_confirmed(mode, start, shuffle, cx);
    }
    pub fn enqueue_visible_confirmed(
        &mut self,
        mode: Enqueue,
        start: Option<Item>,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let items = if start.is_none() && self.has_library_selection() {
            self.selected_library_items()
        } else {
            self.items
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    start.is_some() || self.selected.is_empty() || self.selected.contains(i)
                })
                .map(|(_, item)| item.clone())
                .collect()
        };
        let sessions = self.sessions.clone();
        let complete =
            start.is_some() || (self.selected.is_empty() && self.selected_album_tracks.is_empty());
        let artist_tab = self.artist_tab;
        let artist_role = self.artist_role.clone();
        let release_types = self.state.settings.artist_release_types.clone();
        let combined_groups = self.state.settings.artist_grouping == "all";
        let route = self.route.clone();
        let api = match &route {
            Route::Detail(item) | Route::Similar(item) => {
                self.sessions.get(&item.server_id).cloned()
            }
            _ => self.api(),
        };
        let sort = self.sort;
        let descending = self.descending;
        let radio = api
            .as_ref()
            .filter(|api| api.server.kind == ServerType::Jellyfin)
            .map(|api| {
                self.state
                    .radio_page(api.server.id, "", 0, usize::MAX)
                    .items
            });
        let local_filter = self.local_filter.clone();
        let filters = self.active_filters.clone();
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(request, cancelled.clone());
        let sessions = sessions
            .into_iter()
            .map(|(id, api)| (id, api.with_cancellation(cancelled.clone())))
            .collect::<HashMap<_, _>>();
        let api = api.map(|api| api.with_cancellation(cancelled.clone()));
        let genre_albums = self.state.settings.genre_albums;
        let sample = self.random_sample.clone();
        let task = cx.background_executor().spawn(async move {
            let mut songs = async {
                let mut items = if complete {
                    if let Some(api) = &api {
                        match &route {
                            Route::Detail(item)
                                if matches!(
                                    item.kind,
                                    ItemKind::Artist | ItemKind::AlbumArtist
                                ) =>
                            {
                                let items = api.all_artist_items(item, artist_tab)?;
                                let mut items = if artist_tab == ArtistTab::Songs {
                                    items.into_iter().filter(|item| item.matches(&local_filter) && filters.matches(item)).collect()
                                } else {
                                    filters.search(items, &local_filter, if artist_tab == ArtistTab::Albums { ItemKind::Album } else if artist_tab == ArtistTab::Similar { ItemKind::AlbumArtist } else { ItemKind::Song })
                                };
                                if artist_tab == ArtistTab::Albums {
                                    feishin_core::lists::sort_albums(&mut items, sort, descending);
                                } else if sort != Sort::Id
                                    || !matches!(
                                        artist_tab,
                                        ArtistTab::CommunityTop
                                            | ArtistTab::PersonalTop
                                            | ArtistTab::Favorites
                                            | ArtistTab::Rated
                                    )
                                {
                                    feishin_core::lists::sort_items(&mut items, sort, descending);
                                } else if descending {
                                    items.reverse();
                                }
                                if artist_tab == ArtistTab::Albums {
                                    feishin_core::releases::groups(
                                        &items,
                                        &item.id,
                                        combined_groups,
                                        &release_types,
                                    )
                                    .into_iter()
                                    .flat_map(|(_, indexes)| {
                                        indexes
                                            .into_iter()
                                            .map(|index| items[index].clone())
                                            .collect::<Vec<_>>()
                                    })
                                    .collect()
                                } else {
                                    items
                                }
                            }
                            Route::Similar(item) => {
                                let api = sessions.get(&item.server_id).ok_or_else(|| {
                                    anyhow::anyhow!("Sign in to this track's server")
                                })?;
                                let tracks = if let Some(sample) = sample {
                                    sample
                                } else {
                                    api.random_songs(100, Some(item))?
                                };
                                return Ok(tracks
                                    .into_iter()
                                    .filter(|item| {
                                        item.matches(&local_filter) && filters.matches(item)
                                    })
                                    .collect());
                            }
                            Route::Detail(item) if item.kind == ItemKind::Genre => {
                                api.all_items(api.genre_query(item, ListQuery {
                                    kind: if genre_albums { ItemKind::Album } else { ItemKind::Song },
                                    search: local_filter.clone(),
                                    sort: if genre_albums && sort == Sort::Random { Sort::Name } else { sort },
                                    descending,
                                    filters: filters.clone(),
                                    ..Default::default()
                                })?)?
                            }
                            Route::Library(ItemKind::Folder) => api.folder_children(None)?,
                            Route::Detail(item) => {
                                let tracks = api.all_tracks(item)?;
                                let mut tracks = if matches!(item.kind, ItemKind::Album | ItemKind::Playlist) {
                                    filters.search(tracks, &local_filter, ItemKind::Song)
                                } else {
                                    tracks.into_iter().filter(|item| item.matches(&local_filter) && filters.matches(item)).collect()
                                };
                                if item.kind == ItemKind::Playlist {
                                    if local_filter.trim().is_empty() {
                                        feishin_core::playlist_albums::sort_songs(&mut tracks, sort, descending);
                                    }
                                } else {
                                    feishin_core::lists::sort_items(&mut tracks, sort, descending);
                                }
                                return Ok(tracks);
                            }
                            Route::Explore => sample.unwrap_or(items),
                            Route::Home
                            | Route::Library(_)
                            | Route::Collection(_)
                            | Route::Favorites(_)
                            | Route::Search(..) => {
                                let (kind, favorites, search) = match &route {
                                    Route::Library(kind) => (*kind, false, local_filter.clone()),
                                    Route::Collection(collection) => {
                                        (collection.kind, false, local_filter.clone())
                                    }
                                    Route::Favorites(kind) => (*kind, true, local_filter.clone()),
                                    Route::Search(term, kind) => (*kind, false, term.clone()),
                                    _ => (ItemKind::Album, false, local_filter.clone()),
                                };
                                if kind == ItemKind::Radio {
                                    let stations = if let Some(radio) = radio { radio } else { api.all_items(ListQuery { kind, ..Default::default() })? };
                                    feishin_core::lists::radio_items(stations, &local_filter, sort, descending)
                                } else {
                                    api.all_items(ListQuery {
                                        kind,
                                        favorites,
                                        search,
                                        sort: if matches!(route, Route::Home) {
                                            Sort::Newest
                                        } else if kind == ItemKind::Album && sort == Sort::Random {
                                            Sort::Name
                                        } else {
                                            sort
                                        },
                                        descending,
                                        filters: filters.clone(),
                                        role: if kind == ItemKind::Artist {
                                            artist_role
                                        } else {
                                            None
                                        },
                                        ..Default::default()
                                    })?
                                }
                            }
                            _ => items,
                        }
                    } else {
                        items
                    }
                } else {
                    items
                };
                // Random album lists shuffle album blocks after a stable, complete fetch.
                if complete
                    && sort == Sort::Random
                    && items.iter().all(|item| item.kind == ItemKind::Album)
                {
                    use rand::seq::SliceRandom;
                    items.shuffle(&mut rand::thread_rng());
                }
                let mut songs = vec![];
                for item in items.into_iter().filter(|item| {
                    !complete
                        || route == Route::Library(ItemKind::Radio)
                        || matches!(&route, Route::Detail(artist) if matches!(artist.kind, ItemKind::Artist | ItemKind::AlbumArtist))
                        || matches!(&route, Route::Detail(genre) if genre.kind == ItemKind::Genre)
                        || (item.matches(&local_filter) && filters.matches(item))
                }) {
                    songs.extend(
                        sessions
                            .get(&item.server_id)
                            .ok_or_else(|| anyhow::anyhow!("Sign in to this server"))?
                            .all_tracks(&item)?,
                    );
                }
                anyhow::ensure!(!songs.is_empty(), "No playable tracks found");
                Ok::<_, anyhow::Error>(songs)
            }
            .await?;
            if shuffle {
                use rand::seq::SliceRandom;
                songs.shuffle(&mut rand::thread_rng());
            }
            Ok::<_, anyhow::Error>(songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.library_fetches.remove(&request);
                cx.notify();
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if matches!(mode, Enqueue::Now) && this.play_request_generation != request {
                    return;
                }
                match result {
                    Ok(songs) => {
                        this.add_tracks_from(songs, mode, start.as_ref(), cx);
                    }
                    Err(error) => this.message = error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn play_current(&mut self, cx: &mut Context<Self>) {
        self.state.queue.up_next_enabled = self.state.settings.up_next_queue;
        self.state.queue.sync_up_next();
        self.radio_item = None;
        self.radio_position = 0.0;
        self.play_active(cx);
    }
    pub fn play_active(&mut self, cx: &mut Context<Self>) {
        self.sync_jukebox(cx);
        self.resume_current();
        self.begin_track_reports(cx);
        self.reset_waveform();
        self.seek_preview.clear();
        self.progress
            .update(cx, |slider, _| slider.cancel_pointer_change());
        self.player_title_at = Instant::now();
        self.player_background = None;
        self.player_background_key.clear();
        self.loaded = false;
        self.radio_metadata = Default::default();
        self.lyrics.clear();
        self.lyric_busy = false;
        self.lyric_scroll_motion = Default::default();
        self.lyric_scroll_paused_until = None;
        self.sidebar_lyric_scroll_paused_until = None;
        self.lyric_scroll = ScrollHandle::default();
        self.sidebar_lyric_scroll = ScrollHandle::default();
        self.lyric_annotations.clear();
        self.annotation_generation += 1;
        self.lyric_record = None;
        self.lyric_translation_generation += 1;
        self.lyric_translation_busy = false;
        self.lyric_translated.clear();
        self.lyric_translation_visible = false;
        self.lyric_generation += 1;
        let Some(entry) = self.current_entry().cloned() else {
            self.playing = false;
            self.buffering = false;
            self.player_generation = self.player.send(Control::Stop);
            cx.notify();
            return;
        };
        if let Some(pause) = self
            .sleep_timer
            .as_ref()
            .and_then(|timer| timer.track_changed(entry.key, &entry.item))
        {
            self.sleep_timer = None;
            if pause {
                self.stop_playback(cx);
                return;
            }
        }
        let Some(api) = self.sessions.get(&entry.item.server_id).cloned() else {
            self.message = "Sign in to the server for this track".into();
            self.player_generation = self.player.send(Control::Stop);
            self.playing = false;
            self.buffering = false;
            return;
        };
        let stream = if self.state.settings.jukebox {
            if self.jukebox_server == Some(entry.item.server_id)
                && entry.item.kind == ItemKind::Song
            {
                Ok(String::new())
            } else {
                Err(anyhow::anyhow!(
                    "Jukebox tracks must belong to the selected server. Choose local audio to play this item."
                ))
            }
        } else {
            api.stream_url(&entry.item, &self.state.settings)
        };
        match stream {
            Ok(url) => {
                self.buffering = true;
                self.playing = true;
                self.player_generation = self.player.send(Control::Load {
                    url,
                    settings: Box::new(self.state.settings.clone()),
                    item: Box::new(self.stream_metadata(&entry.item)),
                    position: self.current_position(),
                    gain: feishin_core::dsp::replay_gain(&entry.item, &self.state.settings),
                    key: entry.key.to_string(),
                });
            }
            Err(error) => self.message = error.to_string(),
        }
        self.arm_next();
        self.fetch_covers(cx);
        self.load_lyrics(false, cx);
        self.sync_player_related(cx);
        cx.notify();
    }

    pub fn resume_current(&mut self) {
        if self.radio_item.is_none()
            && let Some(entry) = self.state.queue.current()
            && self.resumed_entry != Some(entry.key)
        {
            self.resumed_entry = Some(entry.key);
            let position = entry.item.resume_position;
            if self.state.queue.position == 0.0
                && position.is_finite()
                && position > 0.0
                && (entry.item.duration <= 0.0 || position < entry.item.duration)
            {
                self.state.queue.position = position;
            }
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if !self.loaded && !self.buffering {
            self.play_active(cx);
        } else {
            self.playing = !self.playing;
            self.player.send(Control::Pause(!self.playing));
            self.report(if self.playing { "unpause" } else { "pause" }, cx);
            cx.notify();
        }
    }

    pub fn next(&mut self, automatic: bool, cx: &mut Context<Self>) {
        if self.radio_item.is_some() {
            if automatic {
                self.loaded = false;
                self.playing = false;
                self.buffering = false;
                cx.notify();
            }
            return;
        }
        self.play_request_generation += 1;
        if automatic {
            self.poll_auto_dj(cx);
        }
        let sleep = automatic && self.sleep_stops_at_end();
        if sleep {
            self.sleep_timer = None;
            self.stop_playback(cx);
            self.state.queue.next(automatic);
            self.save();
            return;
        }
        self.state.queue.next(automatic);
        self.save();
        self.play_current(cx);
    }

    pub fn previous(&mut self, cx: &mut Context<Self>) {
        if self.radio_item.is_some() {
            return;
        }
        self.play_request_generation += 1;
        self.state.queue.previous();
        self.save();
        self.play_current(cx);
    }

    pub fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.poll_server_discovery(cx);
        if let Some(instance) = &self.instance {
            match instance.take_activation() {
                Ok(true) => self.tray_command(crate::tray::Command::Show, window, cx),
                Ok(false) => {}
                Err(error) => log::warn!("Cannot receive window activation: {error}"),
            }
        }
        self.expire_server_sessions(window, cx);
        self.sync_tls_settings(cx);
        self.sync_jukebox(cx);
        if std::mem::take(&mut self.volume_sync_pending) {
            self.volume.update(cx, |slider, cx| {
                slider.set_value(self.state.settings.volume, window, cx)
            });
        }
        self.sync_media_controls(window);
        self.poll_track_notifications(cx);
        self.poll_updates(cx);
        self.poll_release_notes(window, cx);
        self.poll_queue_autosave(cx);
        self.poll_fullscreen_idle(cx);
        self.sync_waveform(cx);
        self.sync_dock(cx);
        self.sync_app_menu(window, cx);
        #[cfg(windows)]
        self.sync_taskbar(window, cx);
        self.sync_tray(window, cx);
        if self.last_presence.elapsed() >= Duration::from_secs(5) {
            self.last_presence = Instant::now();
            self.update_presence(cx);
        }
        if self.last_theme_scan.elapsed() >= Duration::from_secs(2) {
            self.last_theme_scan = Instant::now();
            self.apply_theme(window, cx);
        }
        self.poll_auto_dj(cx);
        if !self.loading
            && self.has_more
            && self.list_config().pagination == "infinite"
            && !self.artist_overview_active()
            && !self.uses_detail_scroll()
            && matches!(
                self.route,
                Route::Home
                    | Route::Library(_)
                    | Route::Collection(_)
                    | Route::Favorites(_)
                    | Route::Search(..)
                    | Route::Detail(_)
                    | Route::Similar(_)
                    | Route::PlaylistPicker
            )
        {
            let scroll = self.library_scroll.0.borrow().base_handle.clone();
            if self.list_config().display != "detail"
                && scroll.bounds().size.height > px(0.)
                && (scroll.max_offset().height + scroll.offset().y) < px(180.)
            {
                self.reload(true, cx);
            }
        }
        let mut changed = false;
        if let Some(events) = &self.tag_progress_events {
            for progress in events.try_iter() {
                self.tag_progress = Some(progress);
                changed = true;
            }
        }
        for action in self.global_shortcuts.poll() {
            self.run_shortcut(action, window, cx);
            changed = true;
        }
        let requests = self
            .remote
            .as_ref()
            .map(|remote| remote.requests.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for request in requests {
            self.remote_request(request, window, cx);
            changed = true;
        }
        while let Ok(event) = self.media_events.try_recv() {
            if !self.media_controls_enabled() {
                continue;
            }
            use souvlaki::MediaControlEvent as Media;
            match event {
                Media::Toggle => self.toggle_play(cx),
                Media::Play if !self.playing => self.toggle_play(cx),
                Media::Pause if self.playing => self.toggle_play(cx),
                Media::Next => self.next(false, cx),
                Media::Previous => self.previous(cx),
                Media::Stop => self.stop_playback(cx),
                Media::SetPosition(position) => {
                    self.seek_position(position.0.as_secs_f64(), window, cx);
                }
                Media::Seek(direction) => {
                    let configured = if direction == souvlaki::SeekDirection::Forward {
                        self.state.settings.playerbar.skip_forward_seconds
                    } else {
                        self.state.settings.playerbar.skip_backward_seconds
                    };
                    let seconds = if configured.is_finite() && configured != 0. {
                        configured
                    } else {
                        5.
                    };
                    let delta = if direction == souvlaki::SeekDirection::Forward {
                        seconds
                    } else {
                        -seconds
                    };
                    self.seek_position((self.current_position() + delta).max(0.0), window, cx);
                }
                Media::SeekBy(direction, duration) => {
                    let delta = duration.as_secs_f64()
                        * if direction == souvlaki::SeekDirection::Forward {
                            1.0
                        } else {
                            -1.0
                        };
                    self.seek_position((self.state.queue.position + delta).max(0.0), window, cx);
                }
                Media::SetVolume(volume) if volume.is_finite() => {
                    self.state.settings.volume = (volume * 100.0).round().clamp(0.0, 100.0) as f32;
                    self.state.settings.muted = false;
                    self.volume.update(cx, |slider, cx| {
                        slider.set_value(self.state.settings.volume, window, cx)
                    });
                    self.player
                        .send(Control::Volume(self.state.settings.volume));
                }
                Media::SetShuffle(enabled) if self.radio_item.is_none() => {
                    self.state.queue.set_shuffle(enabled);
                    self.arm_next();
                }
                Media::SetLoopStatus(status) if self.radio_item.is_none() => {
                    self.state.queue.repeat = match status {
                        souvlaki::LoopStatus::None => feishin_core::queue::Repeat::Off,
                        souvlaki::LoopStatus::Track => feishin_core::queue::Repeat::One,
                        souvlaki::LoopStatus::Playlist => feishin_core::queue::Repeat::All,
                    };
                    self.arm_next();
                }
                Media::Raise => cx.activate(true),
                Media::Quit => cx.quit(),
                _ => {}
            }
            self.save();
            changed = true;
        }
        while let Ok(event) = self.player.events.try_recv() {
            match event {
                Event::Loading(id) if id == self.player_generation => {
                    self.buffering = true;
                }
                Event::Started(id) if id == self.player_generation => {
                    self.lyric_position_at = Instant::now();
                    self.record_played();
                    self.notifications.arm(
                        self.state.queue.current,
                        self.state.settings.notify_tracks && self.radio_item.is_none(),
                        Instant::now(),
                    );
                    self.loaded = true;
                    self.buffering = false;
                    self.playing = true;
                    self.report("start", cx);
                    self.arm_next();
                }
                Event::Paused(id, paused) if id == self.player_generation => {
                    self.lyric_position_at = Instant::now();
                    self.playing = !paused && !self.buffering;
                }
                Event::Position(id, position) if id == self.player_generation => {
                    self.lyric_position_at = Instant::now();
                    let delta = position - self.last_position;
                    let can_submit = self.scrobble_active()
                        && self
                            .listening
                            .observe(self.last_position, position, self.playing);
                    self.last_position = position;
                    self.track_reports.position(position);
                    if self.radio_item.is_some() {
                        self.radio_position = position;
                    } else {
                        self.state.queue.position = position;
                    }
                    if delta.abs() > 0.5 {
                        self.lyric_scroll_paused_until = None;
                        self.sidebar_lyric_scroll_paused_until = None;
                        for (motion, scroll) in self
                            .lyric_scroll_motion
                            .iter_mut()
                            .zip([&self.lyric_scroll, &self.sidebar_lyric_scroll])
                        {
                            motion.cancel(f32::from(scroll.offset().y));
                        }
                    }
                    self.update_lyric_follow(position, cx);
                    if self.scrobble_active()
                        && self.playing
                        && self.current_entry().is_some_and(|entry| {
                            entry.item.resumable
                                && self
                                    .sessions
                                    .get(&entry.item.server_id)
                                    .is_some_and(|api| api.server.kind == ServerType::Jellyfin)
                        })
                        && self.listening.progress_due(position)
                    {
                        self.report("timeupdate", cx);
                    }
                    if let Some(entry) = self.current_entry() {
                        let duration = entry.item.duration;
                        self.seek_preview.observe(position, Instant::now());
                        if can_submit
                            && !self.listening.submitted
                            && self
                                .state
                                .settings
                                .should_scrobble(self.listening.seconds, duration)
                        {
                            self.submit_scrobble(cx);
                        }
                    }
                }
                Event::Stopped(id) if id == self.player_generation => {
                    self.loaded = false;
                    self.buffering = false;
                    self.playing = false;
                }
                Event::RadioMetadata(id, metadata) if id == self.player_generation => {
                    self.radio_metadata = metadata;
                }
                Event::DeviceVolume(id, volume) if id == self.player_generation => {
                    self.state.settings.volume = volume as f32;
                    self.volume
                        .update(cx, |slider, cx| slider.set_value(volume as f32, window, cx));
                }
                Event::JukeboxSynced(id, state) if id == self.player_generation => {
                    self.apply_jukebox_state(state, cx);
                }
                Event::SpeakerTopology(id, groups) if id == self.player_generation => {
                    self.cast_topology_generation += 1;
                    self.cast_groups = groups;
                    if let Some(device) = self.cast_selected.clone().filter(|device| device.sonos) {
                        self.load_speaker_properties(device, cx);
                    }
                }
                Event::Ended(id) if id == self.player_generation => {
                    self.next(true, cx);
                }
                Event::Error(id, message) if id == self.player_generation => {
                    self.loaded = false;
                    self.buffering = false;
                    self.playing = false;
                    self.message = message;
                }
                _ => continue,
            }
            changed = true;
        }
        if self
            .sleep_timer
            .as_mut()
            .is_some_and(|timer| timer.tick(Instant::now(), self.playing && !self.buffering))
        {
            self.sleep_timer = None;
            self.player.send(Control::Pause(true));
            self.playing = false;
            self.report("pause", cx);
            self.arm_next();
            changed = true;
        }
        self.poll_track_reports(cx);
        if self.last_save.elapsed() > Duration::from_secs(10) {
            self.last_save = Instant::now();
            self.save();
        }
        if self.playing
            && self.last_report.elapsed() > Duration::from_secs(10)
            && self.current_entry().is_some_and(|entry| {
                self.sessions
                    .get(&entry.item.server_id)
                    .is_some_and(|api| api.server.kind != ServerType::Jellyfin)
            })
        {
            self.last_report = Instant::now();
            self.report("timeupdate", cx);
        }
        self.power.set(
            self.playing && self.loaded && !self.buffering,
            self.state.settings.prevent_sleep,
            self.state.settings.prevent_suspend,
        );
        if changed {
            if let Some(remote) = &self.remote {
                remote.publish_with_artwork(
                    &self.state,
                    self.playing,
                    &self.sessions,
                    &self.data_dir,
                    self.radio_state(),
                );
            }
            cx.notify();
        }
        // Artwork, lyrics and paused queue settings can change without a player event.
        self.update_media();
    }

    pub fn restore_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.settings.remote_enabled {
            return;
        }
        let generation = self.remote_generation;
        let port = self.state.settings.remote_port;
        let expected_username = self.state.settings.remote_username.clone();
        let task = cx.background_executor().spawn(async move {
            let (username, password) = storage::load_remote_credentials()?;
            anyhow::ensure!(
                expected_username.is_empty() || username == expected_username,
                "Enter a password for the configured remote username"
            );
            let remote = feishin_core::remote::Remote::start(port, &username, &password)?;
            Ok::<_, anyhow::Error>((remote, username, password))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if generation != this.remote_generation || !this.state.settings.remote_enabled {
                    return;
                }
                match result {
                    Ok((remote, username, password)) => {
                        this.remote_username
                            .update(cx, |input, cx| input.set_value(username, window, cx));
                        this.remote_password
                            .update(cx, |input, cx| input.set_value(password, window, cx));
                        this.remote = Some(remote);
                        this.save();
                    }
                    Err(error) => this.message = format!("Remote control: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn apply_remote_config(&mut self, cx: &mut Context<Self>) {
        let port = match self.remote_port.read(cx).value().trim().parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => {
                self.message = "Enter a port from 1 to 65535".into();
                cx.notify();
                return;
            }
        };
        let username = self.remote_username.read(cx).value().to_string();
        let password = self.remote_password.read(cx).value().to_string();
        let result = feishin_core::remote::Remote::validate_credentials(&username, &password)
            .and_then(|()| {
                self.remote.as_ref().map_or(Ok(()), |remote| {
                    remote.update_credentials(&username, &password)
                })
            });
        if let Err(error) = result {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        self.remote_generation += 1;
        let port_changed = self.state.settings.remote_port != port;
        self.state.settings.remote_port = port;
        self.state.settings.remote_username = username;
        self.remember_remote(cx);
        self.message = if port_changed {
            self.tr("error.remotePortWarning")
        } else {
            String::new()
        };
        self.save();
        cx.notify();
    }
    fn start_remote(&mut self, cx: &mut Context<Self>) {
        let username = self.remote_username.read(cx).value().to_string();
        let password = self.remote_password.read(cx).value().to_string();
        match feishin_core::remote::Remote::start(
            self.state.settings.remote_port,
            &username,
            &password,
        ) {
            Ok(remote) => {
                remote.publish_with_artwork(
                    &self.state,
                    self.playing,
                    &self.sessions,
                    &self.data_dir,
                    self.radio_state(),
                );
                self.remote = Some(remote);
                self.state.settings.remote_username.clone_from(&username);
                self.state.settings.remote_enabled = true;
                self.message = format!(
                    "Remote control listening on port {}",
                    self.state.settings.remote_port
                );
                self.remember_remote(cx);
            }
            Err(error) => self.message = error.to_string(),
        }
    }
    fn remember_remote(&self, cx: &mut Context<Self>) {
        let username = self.remote_username.read(cx).value().to_string();
        let password = self.remote_password.read(cx).value().to_string();
        let generation = self.remote_generation;
        let task = cx
            .background_executor()
            .spawn(async move { storage::store_remote_credentials(&username, &password) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                this.update(cx, |this, cx| {
                    if generation == this.remote_generation {
                        this.message = format!("Remote credentials could not be saved. {error}");
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }
    pub fn toggle_remote(&mut self, cx: &mut Context<Self>) {
        self.remote_generation += 1;
        if self.remote.is_some() || self.state.settings.remote_enabled {
            self.remote = None;
            self.state.settings.remote_enabled = false;
            self.message = "Remote control stopped".into();
        } else {
            self.start_remote(cx);
        }
        self.save();
        cx.notify();
    }

    pub(crate) fn remote_request(
        &mut self,
        request: feishin_core::remote::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use serde_json::json;
        let value = request.message;
        let event = value["event"].as_str().unwrap_or_default().to_owned();
        let id = value["id"].as_str().unwrap_or_default().to_owned();
        let request_id = value["requestId"].as_str().unwrap_or_default().to_owned();
        let ack = |error: Option<String>| {
            vec![json!({"event":"operation-ack","data":{"requestId":request_id,"error":error}})]
        };
        let control_result: Option<anyhow::Result<()>> = match event.as_str() {
            "next" => {
                self.next(false, cx);
                Some(Ok(()))
            }
            "previous" => {
                self.previous(cx);
                Some(Ok(()))
            }
            "play" => {
                if !self.playing {
                    self.toggle_play(cx);
                }
                Some(Ok(()))
            }
            "pause" => {
                if self.playing {
                    self.toggle_play(cx);
                }
                Some(Ok(()))
            }
            "volume" => {
                if let Some(volume) = value["volume"].as_f64().filter(|v| v.is_finite()) {
                    self.state.settings.volume = volume.clamp(0.0, 100.0) as f32;
                    self.state.settings.muted = false;
                    self.volume.update(cx, |slider, cx| {
                        slider.set_value(self.state.settings.volume, window, cx)
                    });
                    self.player
                        .send(Control::Volume(self.state.settings.volume));
                    Some(Ok(()))
                } else {
                    Some(Err(anyhow::anyhow!("Invalid volume")))
                }
            }
            "position" => {
                let maximum = self
                    .current_entry()
                    .filter(|entry| entry.item.kind == ItemKind::Song)
                    .map_or(0.0, |entry| entry.item.duration);
                if let Some(position) = value["position"]
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0 && maximum > 0.0 && *v <= maximum)
                {
                    self.seek_position(position, window, cx);
                    Some(Ok(()))
                } else {
                    Some(Err(anyhow::anyhow!("Invalid seek position")))
                }
            }
            "repeat" if self.radio_item.is_none() => {
                self.state.queue.repeat = match self.state.queue.repeat {
                    feishin_core::queue::Repeat::Off => feishin_core::queue::Repeat::All,
                    feishin_core::queue::Repeat::All => feishin_core::queue::Repeat::One,
                    feishin_core::queue::Repeat::One => feishin_core::queue::Repeat::Off,
                };
                Some(Ok(()))
            }
            "shuffle" if self.radio_item.is_none() => {
                self.state.queue.set_shuffle(!self.state.queue.shuffle);
                Some(Ok(()))
            }
            "clear-queue" => {
                self.clear_queue_confirmed(cx);
                Some(Ok(()))
            }
            "stop" | "stop-radio" => {
                self.stop_playback(cx);
                self.save();
                Some(Ok(()))
            }
            "repeat" | "shuffle" => Some(Err(anyhow::anyhow!(
                "This control is unavailable during radio playback"
            ))),
            "queue-jump" | "remove-from-queue" => {
                let key = value["uniqueId"]
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok());
                if let Some(key) = key.filter(|key| self.state.queue.order.contains(key)) {
                    if event == "queue-jump" {
                        self.play_request_generation += 1;
                        self.state.queue.current = Some(key);
                        self.state.queue.position = 0.0;
                        self.play_current(cx);
                    } else {
                        let current =
                            self.radio_item.is_none() && self.state.queue.current == Some(key);
                        if current && self.state.queue.entries.len() == 1 {
                            self.report("stop", cx);
                        }
                        self.play_request_generation += 1;
                        self.state.queue.remove(key);
                        if current {
                            self.play_current(cx);
                        }
                    }
                    Some(Ok(()))
                } else {
                    Some(Err(anyhow::anyhow!("Queue entry no longer exists")))
                }
            }
            "reorder-queue" => Some(feishin_core::remote::reorder_queue(
                &mut self.state.queue,
                &value,
            )),
            _ => None,
        };
        if let Some(result) = control_result {
            self.save();
            let _ = request.reply.send(ack(result.err().map(|e| e.to_string())));
            cx.notify();
            return;
        }
        if event == "proxy" {
            let item = self.current_entry().map(|entry| entry.item.clone());
            let api = item
                .as_ref()
                .and_then(|item| self.sessions.get(&item.server_id))
                .cloned();
            let task = cx.background_executor().spawn(async move {
                use base64::Engine;
                let item = item.ok_or_else(|| anyhow::anyhow!("Nothing is playing"))?;
                let api = api.ok_or_else(|| anyhow::anyhow!("Artwork server is unavailable"))?;
                Ok::<_, anyhow::Error>(
                    base64::engine::general_purpose::STANDARD.encode(api.cover_bytes(&item)?),
                )
            });
            cx.spawn(async move |_, _| {
                let replies = match task.await {
                    Ok(bytes) => vec![json!({"event":"proxy","data":bytes})],
                    Err(error) => vec![json!({"event":"error","data":error.to_string()})],
                };
                let _ = request.reply.send(replies);
            })
            .detach();
            return;
        }
        let requested_server = value["serverId"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok());
        let current_server = self
            .state
            .queue
            .current()
            .filter(|entry| entry.item.id == id && matches!(event.as_str(), "favorite" | "rating"))
            .map(|entry| entry.item.server_id);
        let api = requested_server
            .or(current_server)
            .map_or_else(|| self.api(), |id| self.sessions.get(&id).cloned());
        let Some(api) = api else {
            let _ = request.reply.send(ack(Some("No server selected".into())));
            return;
        };
        let play_type = value["playType"].as_str().unwrap_or("now");
        let mode = match play_type {
            "last" | "lastShuffle" => Enqueue::End,
            "next" | "nextShuffle" => Enqueue::Next,
            _ => Enqueue::Now,
        };
        let shuffle = matches!(play_type, "shuffle" | "lastShuffle" | "nextShuffle");
        let local_radio = self
            .state
            .radio_page(
                api.server.id,
                value["searchTerm"].as_str().unwrap_or_default(),
                0,
                usize::MAX,
            )
            .items;
        let error_request_id = request_id.clone();
        let artwork = self.remote.as_ref().map(|remote| remote.artwork());
        let data_dir = self.data_dir.clone();
        let mutation = match event.as_str() {
            "favorite" => Some((api.server.id, id.clone(), value["favorite"].as_bool(), None)),
            "rating" => Some((
                api.server.id,
                id.clone(),
                None,
                value["rating"].as_u64().map(|rating| rating as u8),
            )),
            _ => None,
        };
        let favorite_request = event == "favorite";
        let rating_request = event == "rating";
        self.favorite_requests += usize::from(favorite_request);
        self.rating_requests += usize::from(rating_request);
        cx.notify();
        if matches!(
            event.as_str(),
            "play-track" | "play-album" | "play-playlist" | "play-track-radio" | "play-radio"
        ) && matches!(mode, Enqueue::Now)
        {
            self.play_request_generation += 1;
        }
        let generation = self.play_request_generation;
        let task=cx.background_executor().spawn(smol::unblock(move || {
            let result=match event.as_str(){
                "tracks-request"|"albums-request"|"playlists-request"|"radio-request"=>{
                    let kind=match event.as_str(){"tracks-request"=>ItemKind::Song,"albums-request"=>ItemKind::Album,"playlists-request"=>ItemKind::Playlist,_=>ItemKind::Radio};
                    let page=if kind==ItemKind::Radio && api.server.kind==ServerType::Jellyfin {let offset=value["startIndex"].as_u64().unwrap_or(0) as usize;let limit=(value["limit"].as_u64().unwrap_or(50) as usize).clamp(1,500);Page{total:Some(local_radio.len()),has_more:offset.saturating_add(limit)<local_radio.len(),items:local_radio.iter().skip(offset).take(limit).cloned().collect()}}else{api.list(&ListQuery{kind,offset:value["startIndex"].as_u64().unwrap_or(0) as usize,limit:(value["limit"].as_u64().unwrap_or(50) as usize).clamp(1,500),search:value["searchTerm"].as_str().unwrap_or_default().into(),..Default::default()})?};
                    Ok((Vec::<Item>::new(),vec![json!({"event":event.replace("request","response"),"data":{"requestId":request_id,"items":page.items.iter().map(|item| artwork.as_ref().map_or_else(|| feishin_core::remote::item_json(item), |artwork| artwork.item_json(item, &api, &data_dir))).collect::<Vec<_>>(),"hasMore":page.has_more}})]))
                },
                "play-track"|"play-album"|"play-playlist"|"play-track-radio"=>{
                    let kind=match event.as_str(){"play-album"=>ItemKind::Album,"play-playlist"=>ItemKind::Playlist,_=>ItemKind::Song};let item=api.item(&id,kind)?;
                    let songs=if event=="play-track-radio"{api.random_songs(50,Some(&item))?}else{api.all_tracks(&item)?};Ok((songs,vec![]))
                },
                "play-radio"=>{let stations=if api.server.kind==ServerType::Jellyfin{local_radio}else{api.all_items(ListQuery{kind:ItemKind::Radio,..Default::default()})?};let station=stations.into_iter().find(|item|item.id==id).ok_or_else(||anyhow::anyhow!("Radio station no longer exists"))?;Ok((vec![station],vec![]))},
                "favorite"=>{let item=api.item(&id,ItemKind::Song)?;let enabled=value["favorite"].as_bool().ok_or_else(||anyhow::anyhow!("Invalid favorite state"))?;api.favorite(&item,enabled)?;Ok((vec![],vec![json!({"event":"favorite","data":{"id":id,"favorite":enabled}})]))},
                "rating"=>{let item=api.item(&id,ItemKind::Song)?;let rating=value["rating"].as_u64().filter(|r|*r<=5).ok_or_else(||anyhow::anyhow!("Rating must be 0 to 5"))? as u8;api.rate(&item,rating)?;Ok((vec![],vec![json!({"event":"rating","data":{"id":id,"rating":rating}})]))},
                "add-to-playlist"=>{let song=api.item(value["songId"].as_str().unwrap_or_default(),ItemKind::Song)?;api.add_to_playlist_checked(value["playlistId"].as_str().unwrap_or_default(),&[song])?;Ok((vec![],vec![]))},
                _=>Err(anyhow::anyhow!("Unsupported remote command")),
            };
            result.map(|(songs,replies)|(songs,replies,request_id))
        }));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.favorite_requests -= usize::from(favorite_request);
                this.rating_requests -= usize::from(rating_request);
                let replies = match result {
                    Ok((mut songs, mut replies, request_id)) => {
                        if let Some((server, id, favorite, rating)) = &mutation {
                            let mut item = Item { server_id: *server, id: id.clone(), kind: ItemKind::Song, ..Default::default() };
                            if let Some(favorite) = favorite { item.favorite = *favorite; }
                            if let Some(rating) = rating { item.set_rating(*rating); }
                            this.apply_library_values(&[item], favorite.is_some(), rating.is_some(), cx);
                            this.save();
                        }
                        if !songs.is_empty() && matches!(mode, Enqueue::Now) && generation != this.play_request_generation {
                            let _ = request.reply.send(vec![json!({"event":"operation-ack","data":{"requestId":request_id,"error":"Playback request was superseded"}})]);
                            return;
                        }
                        if !songs.is_empty() {
                            if shuffle {
                                use rand::seq::SliceRandom;
                                songs.shuffle(&mut rand::thread_rng());
                            }
                            if songs.first().is_some_and(|item| item.kind == ItemKind::Radio) {
                                this.start_radio(songs.remove(0), cx);
                            } else {
                                this.filter_playback_tracks(&mut songs);
                                this.state.queue.add(songs, mode);
                                if matches!(mode, Enqueue::Now) { this.play_current(cx); }
                                this.save();
                            }
                        }
                        replies
                            .push(json!({"event":"operation-ack","data":{"requestId":request_id}}));
                        replies
                    }
                    Err(error) => vec![json!({"event":"operation-ack","data":{"requestId":error_request_id,"error":error.to_string()}})],
                };
                let _ = request.reply.send(replies);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_favorite(&mut self, item: Item, cx: &mut Context<Self>) {
        let enabled = !item.favorite;
        self.set_library_values(vec![item], Some(enabled), None, cx);
    }

    pub fn download(&mut self, item: Item, cx: &mut Context<Self>) {
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            return;
        };
        let mut name = item.name.replace(['/', '\\', ':'], "_");
        if !item.container.is_empty() && item.container.chars().all(|c| c.is_ascii_alphanumeric()) {
            name.push('.');
            name.push_str(&item.container);
        }
        if item.kind != ItemKind::Song {
            name.push_str(".zip");
        }
        let picker = cx.prompt_for_new_path(&self.data_dir, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = picker.await {
                this.update(cx, |this, cx| {
                    this.perform(move || api.download(&item, &path), "Downloaded", cx);
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn edit_playlist(
        &mut self,
        target: Option<Item>,
        songs: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if target
            .as_ref()
            .is_some_and(|item| !self.can_edit_playlist(item))
        {
            return;
        }
        self.form_name.update(cx, |input, cx| {
            input.set_value(
                target
                    .as_ref()
                    .map_or("", |item| item.name.as_str())
                    .to_owned(),
                window,
                cx,
            )
        });
        self.form_comment.update(cx, |input, cx| {
            input.set_value(
                target
                    .as_ref()
                    .map_or("", |item| item.comment.as_str())
                    .to_owned(),
                window,
                cx,
            )
        });
        let rules = target
            .as_ref()
            .and_then(|item| item.rules.as_ref())
            .map(|rules| serde_json::to_string_pretty(rules).unwrap_or_default())
            .unwrap_or_default();
        self.form_public = target.as_ref().is_some_and(|item| item.public);
        self.form_owner = target.as_ref().map_or_else(
            || {
                self.api()
                    .map_or(String::new(), |api| api.session().user_id)
            },
            |item| item.owner_id.clone(),
        );
        self.form_sync = target.as_ref().and_then(|item| item.sync);
        self.load_playlist_options(cx);
        self.form_rules
            .update(cx, |input, cx| input.set_value(rules, window, cx));
        self.smart_editor = None;
        if target.as_ref().is_some_and(|item| item.rules.is_some()) {
            self.enable_smart(window, cx);
        }
        self.form_target = target;
        self.form_songs = songs;
        self.navigate(Route::PlaylistEditor, cx);
    }

    pub fn save_playlist(&mut self, cx: &mut Context<Self>) {
        if self.playlist_saving || self.route != Route::PlaylistEditor {
            return;
        }
        if self
            .form_target
            .as_ref()
            .is_some_and(|item| !self.can_edit_playlist(item))
        {
            self.message = "You do not have permission to edit this playlist".into();
            cx.notify();
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        let name = self.form_name.read(cx).value().to_string();
        let comment = self.form_comment.read(cx).value().to_string();
        let public = self.form_public;
        let mut target = self.form_target.clone();
        if let Some(target) = &mut target {
            target.owner_id.clone_from(&self.form_owner);
            target.sync = self.form_sync;
        }
        let owner = self.form_owner.clone();
        let sync = self.form_sync;
        let songs = self.form_songs.clone();
        let raw = self.form_rules.read(cx).value().to_string();
        let rules = if let Some(editor) = &self.smart_editor {
            match editor.snapshot(cx) {
                Ok(node) => Some(node.json()),
                Err(error) => {
                    self.message = error.to_string();
                    cx.notify();
                    return;
                }
            }
        } else if raw.trim().is_empty() {
            None
        } else {
            match serde_json::from_str(&raw) {
                Ok(value) => Some(value),
                Err(error) => {
                    self.message = format!("Invalid smart playlist rules: {error}");
                    cx.notify();
                    return;
                }
            }
        };
        let server = api.server.id;
        let navigation = self.navigation_generation;
        self.playlist_saving = true;
        let task = cx.background_executor().spawn(async move {
            if let Some(target) = target {
                api.update_playlist_checked(&target, &name, &comment, public, rules)
            } else {
                let id = api.create_playlist_checked(&name, public, rules.clone(), &songs)?;
                let playlist = Item {
                    id,
                    server_id: server,
                    kind: ItemKind::Playlist,
                    owner_id: owner,
                    sync,
                    ..Default::default()
                };
                api.update_playlist_checked(&playlist, &name, &comment, public, rules)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.playlist_saving = false;
                cx.notify();
                if this.state.active_server != Some(server) {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.refresh_playlists(cx);
                        if this.route == Route::PlaylistEditor
                            && this.navigation_generation == navigation
                        {
                            this.navigate(Route::Library(ItemKind::Playlist), cx);
                        }
                        this.message = "Playlist saved".into();
                    }
                    Err(error) => {
                        log::warn!("Could not save playlist: {error}");
                        this.message = error.to_string();
                    }
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Drop for Feishin {
    fn drop(&mut self) {
        self.cancel_speaker_group();
        self.save();
    }
}
