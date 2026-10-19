// Matches @mantine/colors-generator 9.3.1 and chroma-js 3.2.0's D65/LCh saturation.
// Copyright (c) 2021 Vitaly Rtishchev; (c) 2011-2025 Gregor Aisch.
// See assets/mantine-colors-LICENSE.txt and assets/chroma-LICENSE.txt.
const LIGHTNESS: [f64; 10] = [
    0.96, 0.907, 0.805, 0.697, 0.605, 0.547, 0.518, 0.445, 0.395, 0.34,
];
const SATURATION: [f64; 10] = [0.32, 0.16, 0.08, 0.04, 0., 0., 0.04, 0.08, 0.16, 0.32];
const WHITE: [f64; 3] = [0.95047, 1., 1.08883];
const RGB_XYZ: [[f64; 3]; 3] = [
    [0.4124564390896922, 0.357576077643909, 0.18043748326639894],
    [0.21267285140562253, 0.715152155287818, 0.07217499330655958],
    [0.0193338955823293, 0.11919202588130297, 0.9503040785363679],
];
const XYZ_RGB: [[f64; 3]; 3] = [
    [3.2404541621141045, -1.5371385127977166, -0.498531409556016],
    [
        -0.9692660305051868,
        1.8760108454466942,
        0.041556017530349834,
    ],
    [
        0.055643430959114726,
        -0.2040259135167538,
        1.0572251882231791,
    ],
];
fn multiply(matrix: [[f64; 3]; 3], input: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row.into_iter().zip(input).map(|(a, b)| a * b).sum())
}
fn saturate(rgb: [f64; 3], delta: f64) -> [f64; 3] {
    let linear = rgb.map(|c| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    let xyz = multiply(RGB_XYZ, linear);
    let [x, y, z] = std::array::from_fn(|i| {
        let c = xyz[i] / WHITE[i];
        if c > 216. / 24389. {
            c.cbrt()
        } else {
            (24389. / 27. * c + 16.) / 116.
        }
    });
    let light = 116. * y - 16.;
    let a = 500. * (x - y);
    let b = 200. * (y - z);
    let chroma = a.hypot(b);
    let hue = if (chroma * 10000.).round() == 0. {
        0.
    } else {
        b.atan2(a)
    };
    let chroma = (chroma + 18. * delta).max(0.);
    let a = hue.cos() * chroma;
    let b = hue.sin() * chroma;
    let fy = (light + 16.) / 116.;
    let f = [a * 0.002 + fy, fy, fy - b * 0.005];
    let xyz = std::array::from_fn(|i| {
        let c = f[i];
        WHITE[i]
            * if c.powi(3) > 216. / 24389. {
                c.powi(3)
            } else {
                (116. * c - 16.) / (24389. / 27.)
            }
    });
    multiply(XYZ_RGB, xyz).map(|c| {
        let sign = c.signum();
        let c = c.abs();
        (sign
            * if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1. / 2.4) - 0.055
            })
        .clamp(0., 1.)
    })
}
pub fn scale(rgba: [f32; 4]) -> [[f32; 4]; 10] {
    let [r, g, b, _] = rgba.map(f64::from);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let light = (max + min) / 2.;
    let delta = max - min;
    let saturation = if delta == 0. {
        0.
    } else if light < 0.5 {
        delta / (max + min)
    } else {
        delta / (2. - max - min)
    };
    let hue = if delta == 0. {
        0.
    } else if max == r {
        (g - b) / delta
    } else if max == g {
        2. + (b - r) / delta
    } else {
        4. + (r - g) / delta
    } / 6.;
    let base = (0..10)
        .min_by(|&a, &b| {
            (LIGHTNESS[a] - light)
                .abs()
                .total_cmp(&(LIGHTNESS[b] - light).abs())
        })
        .unwrap();
    std::array::from_fn(|i| {
        let l = LIGHTNESS[i];
        let rgb = if i == base {
            [r, g, b]
        } else {
            let upper = if l < 0.5 {
                l * (1. + saturation)
            } else {
                l + saturation - l * saturation
            };
            let lower = 2. * l - upper;
            let rgb = [hue + 1. / 3., hue, hue - 1. / 3.].map(|h| {
                let h = h.rem_euclid(1.);
                if saturation == 0. {
                    l
                } else if 6. * h < 1. {
                    lower + (upper - lower) * 6. * h
                } else if 2. * h < 1. {
                    upper
                } else if 3. * h < 2. {
                    lower + (upper - lower) * (2. / 3. - h) * 6.
                } else {
                    lower
                }
            });
            saturate(rgb, SATURATION[i] - SATURATION[base])
        };
        // The original generator returns six-digit hex colors, with opaque alpha.
        [
            (rgb[0] * 255.).round() as f32 / 255.,
            (rgb[1] * 255.).round() as f32 / 255.,
            (rgb[2] * 255.).round() as f32 / 255.,
            1.,
        ]
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn accent_scales_match_original_mantine_chroma_reference() {
        let fixtures: Vec<(String, Vec<String>)> =
            serde_json::from_str(include_str!("../assets/accent-reference.json")).unwrap();
        for (color, expected) in fixtures {
            let actual = super::scale(crate::appearance::color(&color).unwrap());
            for (i, expected) in expected.iter().enumerate() {
                let expected = crate::appearance::color(expected).unwrap();
                assert_eq!(actual[i], expected, "{color} shade {i}");
            }
        }
    }
}
