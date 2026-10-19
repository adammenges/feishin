use std::{collections::BTreeMap, sync::Arc};

/// Explicit OpenType variation coordinates, applied after CSS face selection.
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(try_from = "Vec<(String, f32)>", into = "Vec<(String, f32)>")]
pub struct FontVariations(#[schemars(with = "Vec<(String, f32)>")] Arc<Vec<(String, u32)>>);

impl TryFrom<Vec<(String, f32)>> for FontVariations {
    type Error = String;

    fn try_from(values: Vec<(String, f32)>) -> Result<Self, Self::Error> {
        if values.len() > 64
            || values.iter().any(|(tag, value)| {
                tag.len() != 4
                    || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
                    || !value.is_finite()
            })
        {
            return Err("Invalid font variation settings".into());
        }
        Ok(Self(Arc::new(
            values
                .into_iter()
                .map(|(tag, value)| {
                    (
                        tag,
                        if value == 0. {
                            0f32.to_bits()
                        } else {
                            value.to_bits()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        )))
    }
}

impl From<FontVariations> for Vec<(String, f32)> {
    fn from(values: FontVariations) -> Self {
        values
            .iter()
            .map(|(tag, value)| (tag.to_owned(), value))
            .collect()
    }
}

impl FontVariations {
    // The bundled CSS fallback faces have fixed defaults but allow user axes to
    // override them. Their private companions retain the original variable data.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(crate) fn fallback_companion(&self, postscript: &str) -> Option<(&'static str, f32)> {
        let (family, style) = if let Some(style) = postscript.strip_prefix("FeishinNoto-jp-") {
            if self.get("wght").is_none() {
                return None;
            }
            ("Noto Sans JP Variable", style)
        } else if let Some(style) = postscript.strip_prefix("FeishinNoto-hebrew-") {
            if self.get("wght").is_none() && self.get("wdth").is_none() {
                return None;
            }
            ("Noto Sans Hebrew Variable", style)
        } else {
            return None;
        };
        ["Regular", "Medium", "SemiBold", "Bold", "ExtraBold"]
            .iter()
            .position(|value| *value == style)
            .map(|index| (family, 500. + index as f32 * 100.))
    }

    /// Iterate over canonical, case-sensitive tags and finite coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.0
            .iter()
            .map(|(tag, bits)| (tag.as_str(), f32::from_bits(*bits)))
    }

    /// Look up an explicit axis override.
    pub fn get(&self, tag: &str) -> Option<f32> {
        self.iter()
            .find_map(|(key, value)| (key == tag).then_some(value))
    }
}
