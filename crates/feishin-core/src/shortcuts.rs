//! Persisted shortcut actions shared by settings import and the native UI.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! actions {
    ($($action:ident => ($label:literal, $caption:literal, $global:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub enum Action { $($action),+ }
        impl Action {
            pub const ALL: &'static [Self] = &[$(Self::$action),+];
            pub fn label(self) -> &'static str { match self { $(Self::$action => $label),+ } }
            pub fn caption(self) -> &'static str { match self { $(Self::$action => $caption),+ } }
            pub fn allow_global(self) -> bool { match self { $(Self::$action => $global),+ } }
        }
    }
}
actions! {
    PlayPause => ("Play / pause", "setting.hotkey_playbackPlayPause", true),
    Play => ("Play", "setting.hotkey_playbackPlay", true),
    Pause => ("Pause", "setting.hotkey_playbackPause", true),
    Stop => ("Stop", "setting.hotkey_playbackStop", true),
    Next => ("Next track", "setting.hotkey_playbackNext", true),
    Previous => ("Previous track", "setting.hotkey_playbackPrevious", true),
    NextAlbum => ("Next album", "setting.hotkey_playbackNextAlbum", true),
    PreviousAlbum => ("Previous album", "setting.hotkey_playbackPreviousAlbum", true),
    SkipForward => ("Seek forward", "setting.hotkey_skipForward", true),
    SkipBackward => ("Seek backward", "setting.hotkey_skipBackward", true),
    ToggleRepeat => ("Repeat mode", "setting.hotkey_toggleRepeat", true),
    ToggleShuffle => ("Shuffle", "setting.hotkey_toggleShuffle", true),
    VolumeUp => ("Volume up", "setting.hotkey_volumeUp", true),
    VolumeDown => ("Volume down", "setting.hotkey_volumeDown", true),
    VolumeMute => ("Mute", "setting.hotkey_volumeMute", true),
    FavoriteCurrentAdd => ("Favorite current track", "setting.hotkey_favoriteCurrentSong", true),
    FavoriteCurrentRemove => ("Unfavorite current track", "setting.hotkey_unfavoriteCurrentSong", true),
    FavoriteCurrentToggle => ("Toggle current track favorite", "setting.hotkey_toggleCurrentSongFavorite", true),
    FavoritePreviousAdd => ("Favorite previous track", "setting.hotkey_favoritePreviousSong", true),
    FavoritePreviousRemove => ("Unfavorite previous track", "setting.hotkey_unfavoritePreviousSong", true),
    FavoritePreviousToggle => ("Toggle previous track favorite", "setting.hotkey_togglePreviousSongFavorite", true),
    Rate0 => ("Clear track rating", "setting.hotkey_rate0", true),
    Rate1 => ("Rate track 1", "setting.hotkey_rate1", true),
    Rate2 => ("Rate track 2", "setting.hotkey_rate2", true),
    Rate3 => ("Rate track 3", "setting.hotkey_rate3", true),
    Rate4 => ("Rate track 4", "setting.hotkey_rate4", true),
    Rate5 => ("Rate track 5", "setting.hotkey_rate5", true),
    GlobalSearch => ("Search library", "setting.hotkey_globalSearch", false),
    LocalSearch => ("Filter current list", "setting.hotkey_localSearch", false),
    NavigateHome => ("Home", "setting.hotkey_navigateHome", false),
    BrowserBack => ("Back", "setting.hotkey_browserBack", false),
    BrowserForward => ("Forward", "setting.hotkey_browserForward", false),
    ListNavigateToPage => ("Open selected item", "setting.hotkey_listNavigateToPage", false),
    ListPlayDefault => ("Play selected items", "setting.hotkey_listPlayDefault", false),
    ListPlayNow => ("Play selected now", "setting.hotkey_listPlayNow", false),
    ListPlayNext => ("Play selected next", "setting.hotkey_listPlayNext", false),
    ListPlayLast => ("Play selected at end", "setting.hotkey_listPlayLast", false),
    ListShowPlayingSong => ("Show playing track", "setting.hotkey_listShowPlayingSong", false),
    ToggleQueue => ("Show / hide queue", "setting.hotkey_toggleQueue", false),
    ToggleFullscreenPlayer => ("Expand / collapse player", "setting.hotkey_toggleFullScreenPlayer", false),
    ToggleFullscreenWindow => ("Window fullscreen", "Window fullscreen", false),
    ZoomIn => ("Zoom in", "setting.hotkey_zoomIn", true),
    ZoomOut => ("Zoom out", "setting.hotkey_zoomOut", true),
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Binding {
    pub key: String,
    pub global: bool,
    /// Match the key position, as Electron's recorder does. Older native bindings
    /// retain character matching when this field is absent.
    #[serde(skip_serializing_if = "is_false")]
    pub physical: bool,
}
fn is_false(value: &bool) -> bool {
    !value
}
pub type Bindings = BTreeMap<Action, Binding>;
pub fn defaults() -> Bindings {
    let mut bindings: Bindings = Action::ALL
        .iter()
        .map(|action| (*action, Binding::default()))
        .collect();
    for (action, key) in [
        (Action::PlayPause, "space"),
        (Action::Next, "alt-right"),
        (Action::Previous, "alt-left"),
        (Action::GlobalSearch, "secondary-k"),
        (Action::LocalSearch, "secondary-f"),
        (Action::ListNavigateToPage, "secondary-g"),
        (Action::ListPlayDefault, "enter"),
        (Action::ListShowPlayingSong, "secondary-l"),
        (Action::BrowserBack, "secondary-["),
        (Action::ToggleFullscreenPlayer, "alt-enter"),
        (Action::ToggleFullscreenWindow, "f11"),
    ] {
        let binding = bindings.get_mut(&action).unwrap();
        binding.key = key.into();
        binding.physical = true;
    }
    bindings
}
/// Electron uses '+' separators and names its primary modifier 'mod'.
pub fn from_electron(value: &serde_json::Value, current: &Bindings) -> anyhow::Result<Bindings> {
    let values = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Shortcuts must be an object"))?;
    let mut bindings = current.clone();
    for (name, value) in values {
        let Ok(action) = serde_json::from_value::<Action>(name.clone().into()) else {
            continue;
        };
        let key = value["hotkey"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Shortcut needs a key"))?;
        anyhow::ensure!(
            key.len() <= 128 && !key.chars().any(char::is_control),
            "Invalid shortcut"
        );
        let key = key
            .split('+')
            .map(|part| match part {
                "mod" => "secondary",
                "meta" => "cmd",
                "esc" => "escape",
                "arrowdown" => "down",
                "arrowleft" => "left",
                "arrowright" => "right",
                "arrowup" => "up",
                "equal" => "=",
                "minus" => "-",
                other => other,
            })
            .collect::<Vec<_>>()
            .join("-");
        let global = value["isGlobal"].as_bool().unwrap_or(false) && action.allow_global();
        bindings.insert(
            action,
            Binding {
                physical: !key.is_empty(),
                key,
                global,
            },
        );
    }
    Ok(bindings)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_captions_match_original_actions_in_every_bundled_language() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/shortcut-labels-reference.json")).unwrap();
        for (language, labels) in reference["languages"].as_object().unwrap() {
            for (id, expected) in labels["actions"].as_object().unwrap() {
                let action: Action = serde_json::from_value(id.clone().into()).unwrap();
                assert_eq!(
                    crate::i18n::text(language, action.caption()),
                    expected.as_str().unwrap(),
                    "{language}: {id}"
                );
            }
        }
    }
    #[test]
    fn electron_shortcuts_preserve_actions_modifiers_and_global_scope() {
        let value = serde_json::json!({"next":{"hotkey":"mod+shift+arrowright","isGlobal":true},"globalSearch":{"hotkey":"meta+k","isGlobal":true},"playPause":{"hotkey":"","isGlobal":false}});
        let bindings = from_electron(&value, &defaults()).unwrap();
        assert_eq!(bindings[&Action::Next].key, "secondary-shift-right");
        assert!(bindings[&Action::Next].global);
        assert!(bindings[&Action::Next].physical);
        assert_eq!(bindings[&Action::GlobalSearch].key, "cmd-k");
        assert!(!bindings[&Action::GlobalSearch].global);
        assert!(bindings[&Action::PlayPause].key.is_empty());
        assert!(!bindings[&Action::PlayPause].physical);
        let legacy: Binding = serde_json::from_value(serde_json::json!({
            "key": "ctrl-+", "global": false
        }))
        .unwrap();
        assert!(
            !legacy.physical,
            "Existing native shortcuts retain character matching"
        );
        assert!(
            !serde_json::to_value(legacy)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("physical")
        );
        let encoded = serde_json::to_string(&bindings).unwrap();
        assert_eq!(
            serde_json::from_str::<Bindings>(&encoded).unwrap().len(),
            Action::ALL.len()
        );
    }
}
