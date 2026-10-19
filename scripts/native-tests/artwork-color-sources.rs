use base64::Engine;
use image::{ExtendedColorType, ImageEncoder};
fn main() {
    let samples: [[u8; 4]; 6] = [
        [80, 130, 170, 255],
        [180, 90, 60, 128],
        [60, 180, 120, 64],
        [120, 90, 190, 0],
        [200, 150, 50, 255],
        [128, 128, 128, 255],
    ];
    let mut result = Vec::new();
    for (name, profile) in [
        ("sRGB", moxcms::ColorProfile::new_srgb()),
        ("Display P3", moxcms::ColorProfile::new_display_p3()),
        ("Adobe RGB", moxcms::ColorProfile::new_adobe_rgb()),
        (
            "Gray gamma 1.8",
            moxcms::ColorProfile::new_gray_with_gamma(1.8),
        ),
    ] {
        let mut profile = profile.encode().unwrap();
        // Normalize the generated ICC header date for repeatable encoded fixtures.
        profile[24..36].copy_from_slice(&[0x07, 0xe8, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
        let gray = name.starts_with("Gray");
        let pixels: Vec<u8> = samples
            .into_iter()
            .flat_map(|p| if gray { vec![p[0], p[3]] } else { p.to_vec() })
            .collect();
        for format in ["png", "png16", "jpeg", "webp"] {
            if gray && format == "webp" {
                continue;
            }
            let mut bytes = Vec::new();
            match format {
                "png" | "png16" => {
                    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
                    encoder.set_icc_profile(profile.clone()).unwrap();
                    let (data, color) = if format == "png16" {
                        (
                            pixels
                                .iter()
                                .flat_map(|&v| (u16::from(v) * 257).to_ne_bytes())
                                .collect(),
                            if gray {
                                ExtendedColorType::La16
                            } else {
                                ExtendedColorType::Rgba16
                            },
                        )
                    } else {
                        (
                            pixels.clone(),
                            if gray {
                                ExtendedColorType::La8
                            } else {
                                ExtendedColorType::Rgba8
                            },
                        )
                    };
                    encoder.write_image(&data, 2, 3, color).unwrap();
                }
                "jpeg" => {
                    let mut encoder =
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
                    encoder.set_icc_profile(profile.clone()).unwrap();
                    let data: Vec<u8> = samples
                        .into_iter()
                        .flat_map(|p| if gray { vec![p[0]] } else { p[..3].to_vec() })
                        .collect();
                    encoder
                        .write_image(
                            &data,
                            2,
                            3,
                            if gray {
                                ExtendedColorType::L8
                            } else {
                                ExtendedColorType::Rgb8
                            },
                        )
                        .unwrap();
                }
                "webp" => {
                    let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
                    encoder.set_icc_profile(profile.clone()).unwrap();
                    encoder
                        .write_image(&pixels, 2, 3, ExtendedColorType::Rgba8)
                        .unwrap();
                }
                _ => unreachable!(),
            }
            result.push(serde_json::json!({"profile":name,"format":format,"bytes":base64::engine::general_purpose::STANDARD.encode(bytes)}));
        }
    }
    for format in ["png", "webp"] {
        let mut bytes = Vec::new();
        let pixels: Vec<u8> = samples.into_iter().flatten().collect();
        if format == "png" {
            let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
            encoder.set_icc_profile(b"malformed ICC".to_vec()).unwrap();
            encoder
                .write_image(&pixels, 2, 3, ExtendedColorType::Rgba8)
                .unwrap();
        } else {
            let mut encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
            encoder.set_icc_profile(b"malformed ICC".to_vec()).unwrap();
            encoder
                .write_image(&pixels, 2, 3, ExtendedColorType::Rgba8)
                .unwrap();
        }
        result.push(serde_json::json!({"profile":"Malformed ICC","format":format,"bytes":base64::engine::general_purpose::STANDARD.encode(bytes)}));
    }
    println!("{}", serde_json::to_string(&result).unwrap());
}
