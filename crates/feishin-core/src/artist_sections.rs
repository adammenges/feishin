use crate::releases::Setting;
use std::collections::HashSet;
pub const FIELDS: [(&str, &str); 6] = [
    ("biography", "Biography"),
    ("favoriteSongs", "Favorite songs"),
    ("recentAlbums", "Albums"),
    ("similarArtists", "Similar artists"),
    ("topSongs", "Top songs"),
    ("compilations", "Compilations"),
];
pub fn defaults() -> Vec<Setting> {
    FIELDS
        .iter()
        .filter(|(id, _)| *id != "compilations")
        .map(|(id, _)| Setting {
            id: (*id).into(),
            disabled: false,
        })
        .collect()
}
pub fn validate(items: &mut Vec<Setting>) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    anyhow::ensure!(items.iter().all(|item| FIELDS.iter().any(|(id,_)| *id == item.id) && seen.insert(item.id.clone())), "Invalid artist overview sections");
    for item in defaults() {
        if seen.insert(item.id.clone()) {
            items.push(item);
        }
    }
    Ok(())
}
pub fn label(id: &str) -> &str {
    FIELDS
        .iter()
        .find(|(key, _)| *key == id)
        .map_or(id, |(_, label)| *label)
}
