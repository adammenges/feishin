use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Placeholder {
    #[default]
    Thumbhash,
    Blurhash,
    DominantColor,
    Off,
}

impl Placeholder {
    pub const ALL: [Self; 4] = [
        Self::Thumbhash,
        Self::Blurhash,
        Self::DominantColor,
        Self::Off,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Thumbhash => "ThumbHash",
            Self::Blurhash => "BlurHash",
            Self::DominantColor => "Dominant color",
            Self::Off => "Off",
        }
    }
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Thumbhash => "setting.imagePlaceholderPriority_optionThumbhash",
            Self::Blurhash => "setting.imagePlaceholderPriority_optionBlurhash",
            Self::DominantColor => "setting.imagePlaceholderPriority_optionDominantColor",
            Self::Off => "setting.imagePlaceholderPriority_optionOff",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    FullScreenPlayer,
    Header,
    ItemCard,
    Sidebar,
    Table,
}

impl Placement {
    pub const ALL: [Self; 5] = [
        Self::FullScreenPlayer,
        Self::Header,
        Self::ItemCard,
        Self::Sidebar,
        Self::Table,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::FullScreenPlayer => "Fullscreen player",
            Self::Header => "Header",
            Self::ItemCard => "Item card",
            Self::Sidebar => "Sidebar",
            Self::Table => "Table",
        }
    }
    pub fn label_key(self) -> &'static str {
        match self {
            Self::FullScreenPlayer => "setting.imageResolution_optionFullScreenPlayer",
            Self::Header => "setting.imageResolution_optionHeader",
            Self::ItemCard => "setting.imageResolution_optionItemCard",
            Self::Sidebar => "setting.imageResolution_optionSidebar",
            Self::Table => "setting.imageResolution_optionTable",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Resolution {
    pub full_screen_player: f64,
    pub header: f64,
    pub item_card: f64,
    pub sidebar: f64,
    pub table: f64,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            full_screen_player: 0.,
            header: 300.,
            item_card: 300.,
            sidebar: 400.,
            table: 80.,
        }
    }
}

impl Resolution {
    pub fn get(&self, placement: Placement) -> f64 {
        match placement {
            Placement::FullScreenPlayer => self.full_screen_player,
            Placement::Header => self.header,
            Placement::ItemCard => self.item_card,
            Placement::Sidebar => self.sidebar,
            Placement::Table => self.table,
        }
    }
    pub fn set(&mut self, placement: Placement, size: f64) {
        *match placement {
            Placement::FullScreenPlayer => &mut self.full_screen_player,
            Placement::Header => &mut self.header,
            Placement::ItemCard => &mut self.item_card,
            Placement::Sidebar => &mut self.sidebar,
            Placement::Table => &mut self.table,
        } = size;
    }
}

pub fn cache_key(item: &crate::model::Item, resolution: f64) -> String {
    let key = crate::api::artwork_key(item);
    if resolution == 400. {
        key
    } else {
        format!("{key}-{resolution}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(item: &crate::model::Item, priority: Placeholder) -> Option<Pixels> {
    use Placeholder::*;
    let order: &[Placeholder] = match priority {
        Thumbhash => &[Thumbhash, Blurhash, DominantColor],
        Blurhash => &[Blurhash, Thumbhash, DominantColor],
        DominantColor => &[DominantColor, Thumbhash, Blurhash],
        Off => &[],
    };
    order.iter().find_map(|source| match source {
        Thumbhash => thumbhash(&item.thumb_hash).ok(),
        Blurhash => blurhash(&item.blur_hash).ok(),
        DominantColor => crate::appearance::color(&item.dominant_color)
            .ok()
            .map(|color| Pixels {
                width: 1,
                height: 1,
                rgba: color
                    .iter()
                    .map(|component| (component * 255.).round() as u8)
                    .collect(),
            }),
        Off => None,
    })
}

// Port of src/shared/utils/thumbhash.ts, preserving its RGBA quantization.
pub fn thumbhash(hash: &str) -> Result<Pixels> {
    ensure!(hash.len() <= 128, "ThumbHash is too large");
    let bytes =
        base64::engine::general_purpose::STANDARD_NO_PAD.decode(hash.trim_end_matches('='))?;
    ensure!(bytes.len() >= 5, "Truncated ThumbHash");
    let header24 = u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16;
    let header16 = u16::from_le_bytes([bytes[3], bytes[4]]);
    let alpha = header24 >> 23 != 0;
    ensure!(!alpha || bytes.len() >= 6, "Truncated ThumbHash alpha");
    let landscape = header16 >> 15 != 0;
    let short = usize::from(header16 & 7);
    ensure!(short > 0, "Invalid ThumbHash dimensions");
    let long = if alpha { 5 } else { 7 };
    let (raw_x, raw_y) = if landscape {
        (long, short)
    } else {
        (short, long)
    };
    let (lx, ly) = (raw_x.max(3), raw_y.max(3));
    let mut index = 0usize;
    let start = if alpha { 6 } else { 5 };
    let mut channel = |nx: usize, ny: usize, scale: f64| -> Result<Vec<f64>> {
        let mut ac = vec![];
        for cy in 0..ny {
            let mut cx = usize::from(cy == 0);
            while cx * ny < nx * (ny - cy) {
                let byte = bytes
                    .get(start + (index >> 1))
                    .context("Truncated ThumbHash coefficients")?;
                let value = (byte >> ((index & 1) << 2)) & 15;
                index += 1;
                ac.push((f64::from(value) / 7.5 - 1.) * scale);
                cx += 1;
            }
        }
        Ok(ac)
    };
    let la = channel(lx, ly, f64::from((header24 >> 18) & 31) / 31.)?;
    let pa = channel(3, 3, f64::from((header16 >> 3) & 63) / 63. * 1.25)?;
    let qa = channel(3, 3, f64::from((header16 >> 9) & 63) / 63. * 1.25)?;
    let aa = if alpha {
        channel(5, 5, f64::from(bytes[5] >> 4) / 15.)?
    } else {
        vec![]
    };
    let ratio = raw_x as f64 / raw_y as f64;
    let width = (if ratio > 1. { 32. } else { 32. * ratio }).round() as u32;
    let height = (if ratio > 1. { 32. / ratio } else { 32. }).round() as u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let fx = (0..lx.max(if alpha { 5 } else { 3 }))
                .map(|cx| {
                    (std::f64::consts::PI / f64::from(width) * (f64::from(x) + 0.5) * cx as f64)
                        .cos()
                })
                .collect::<Vec<_>>();
            let fy = (0..ly.max(if alpha { 5 } else { 3 }))
                .map(|cy| {
                    (std::f64::consts::PI / f64::from(height) * (f64::from(y) + 0.5) * cy as f64)
                        .cos()
                })
                .collect::<Vec<_>>();
            let accumulate = |mut dc: f64, ac: &[f64], nx: usize, ny: usize| {
                let mut index = 0;
                for (cy, fy) in fy.iter().take(ny).enumerate() {
                    let fy2 = fy * 2.;
                    let mut cx = usize::from(cy == 0);
                    while cx * ny < nx * (ny - cy) {
                        dc += ac[index] * fx[cx] * fy2;
                        index += 1;
                        cx += 1;
                    }
                }
                dc
            };
            let l = accumulate(f64::from(header24 & 63) / 63., &la, lx, ly);
            // Preserve the reference's multiplication order for P/Q.
            let mut p = f64::from((header24 >> 6) & 63) / 31.5 - 1.;
            let mut q = f64::from((header24 >> 12) & 63) / 31.5 - 1.;
            let mut index = 0;
            for (cy, fy) in fy.iter().take(3).enumerate() {
                let fy2 = fy * 2.;
                for fx in fx.iter().take(3 - cy).skip(usize::from(cy == 0)) {
                    let f = fx * fy2;
                    p += pa[index] * f;
                    q += qa[index] * f;
                    index += 1;
                }
            }
            let a = if alpha {
                accumulate(f64::from(bytes[5] & 15) / 15., &aa, 5, 5)
            } else {
                1.
            };
            let b = l - (2. / 3.) * p;
            let r = (3. * l - b + q) / 2.;
            rgba.extend([r, r - q, b, a].map(|v| (255. * v.clamp(0., 1.)) as u8));
        }
    }
    Ok(Pixels {
        width,
        height,
        rgba,
    })
}

// Port of src/shared/utils/blurhash.ts; bounded 32 x 32 output.
pub fn blurhash(hash: &str) -> Result<Pixels> {
    const ALPHABET: &[u8] =
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";
    ensure!((6..=166).contains(&hash.len()), "Invalid BlurHash length");
    let digits = hash
        .bytes()
        .map(|byte| {
            ALPHABET
                .iter()
                .position(|v| *v == byte)
                .context("Invalid BlurHash character")
        })
        .collect::<Result<Vec<_>>>()?;
    let size = digits[0];
    ensure!(size <= 80, "Invalid BlurHash dimensions");
    let (nx, ny) = (size % 9 + 1, size / 9 + 1);
    ensure!(digits.len() == 4 + 2 * nx * ny, "Truncated BlurHash");
    let number = |range: std::ops::Range<usize>| {
        digits[range]
            .iter()
            .fold(0usize, |value, digit| value * 83 + digit)
    };
    let maximum = (digits[1] + 1) as f64 / 166.;
    let dc = number(2..6);
    ensure!(dc <= 0xffffff, "Invalid BlurHash DC color");
    let linear = |v: usize| {
        let v = v as f64 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut colors = vec![[linear(dc >> 16), linear(dc >> 8 & 255), linear(dc & 255)]];
    for i in 1..nx * ny {
        let value = number(4 + i * 2..6 + i * 2);
        ensure!(value < 19 * 19 * 19, "Invalid BlurHash AC color");
        colors.push([value / 361, value / 19 % 19, value % 19].map(|value| {
            let value = (value as f64 - 9.) / 9.;
            value.abs().powi(2) * if value < 0. { -1. } else { 1. } * maximum
        }));
    }
    let srgb = |v: f64| {
        let v = v.clamp(0., 1.);
        (if v <= 0.0031308 {
            v * 12.92 * 255. + 0.5
        } else {
            (1.055 * v.powf(1. / 2.4) - 0.055) * 255. + 0.5
        }) as u8
    };
    let mut rgba = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            let mut rgb = [0.; 3];
            for j in 0..ny {
                let basis_y = (std::f64::consts::PI * y as f64 * j as f64 / 32.).cos();
                for i in 0..nx {
                    let basis = (std::f64::consts::PI * x as f64 * i as f64 / 32.).cos() * basis_y;
                    for (channel, color) in rgb.iter_mut().zip(colors[i + j * nx]) {
                        *channel += color * basis;
                    }
                }
            }
            rgba.extend([srgb(rgb[0]), srgb(rgb[1]), srgb(rgb[2]), 255]);
        }
    }
    Ok(Pixels {
        width: 32,
        height: 32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_artwork_preferences_import_and_validate() {
        let preview = crate::migration::preview(&crate::storage::Settings::default(), br##"{"general":{"imagePlaceholderPriority":"dominantColor","imageRes":{"fullScreenPlayer":0,"header":600,"itemCard":500,"sidebar":350,"table":90},"nativeAspectRatio":true}}"##).unwrap();
        assert_eq!(
            preview.settings.image_placeholder,
            Placeholder::DominantColor
        );
        assert!(preview.settings.native_image_aspect_ratio);
        assert_eq!(
            preview.settings.image_resolution,
            Resolution {
                full_screen_player: 0.,
                header: 600.,
                item_card: 500.,
                sidebar: 350.,
                table: 90.
            }
        );
        assert!(preview.omitted.is_empty());
        for value in ["2001", "-1", "123.5"] {
            let imported = crate::migration::preview(
                &preview.settings,
                format!(r#"{{"settings":{{"image_resolution":{{"table":{value}}}}}}}"#).as_bytes(),
            )
            .unwrap();
            assert_eq!(
                imported.settings.image_resolution.table,
                value.parse::<f64>().unwrap()
            );
        }
        assert!(
            crate::migration::preview(
                &preview.settings,
                br#"{"settings":{"image_resolution":{"table":1e999}}}"#,
            )
            .is_err()
        );
        let old: crate::storage::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.image_resolution, Resolution::default());
        assert_eq!(old.image_placeholder, Placeholder::Thumbhash);
        let item = crate::model::Item::default();
        assert_ne!(cache_key(&item, 80.), cache_key(&item, 300.));
        assert_ne!(cache_key(&item, 0.), cache_key(&item, 400.));
        assert_ne!(cache_key(&item, 123.), cache_key(&item, 123.5));
    }

    #[test]
    fn original_javascript_pixel_fixtures_and_fallbacks() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../assets/image-hash-reference.json")).unwrap();
        for fixture in fixtures.as_array().unwrap() {
            let hash = fixture["hash"].as_str().unwrap();
            let pixels = if fixture["kind"] == "thumbhash" {
                thumbhash(hash)
            } else {
                blurhash(hash)
            }
            .unwrap();
            assert_eq!(pixels.width, fixture["width"].as_u64().unwrap() as u32);
            assert_eq!(pixels.height, fixture["height"].as_u64().unwrap() as u32);
            assert_eq!(
                format!("{:x}", md5::compute(&pixels.rgba)),
                fixture["md5"].as_str().unwrap(),
                "{hash}"
            );
        }
        let mut item = crate::model::Item {
            thumb_hash: "invalid!".into(),
            blur_hash: "LEHV6nWB2yk8pyo0adR*.7kCMdnj".into(),
            dominant_color: "#10203080".into(),
            ..Default::default()
        };
        assert_eq!(
            decode(&item, Placeholder::Thumbhash),
            blurhash(&item.blur_hash).ok()
        );
        assert_eq!(
            decode(&item, Placeholder::DominantColor).unwrap().rgba,
            [16, 32, 48, 128]
        );
        assert!(decode(&item, Placeholder::Off).is_none());
        item.blur_hash = "bad".into();
        assert_eq!(decode(&item, Placeholder::Blurhash).unwrap().width, 1);
        for hash in [
            "",
            "a",
            "AAAAAAA=",
            "//////8=",
            "not base64",
            "i".repeat(129).as_str(),
        ] {
            assert!(thumbhash(hash).is_err(), "{hash}");
        }
        for hash in ["", "abc", "~00000", "0~zzzz", "00!!!!", "L00000"] {
            assert!(blurhash(hash).is_err(), "{hash}");
        }
    }
}
