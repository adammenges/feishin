//! Preset values match the existing Feishin audio settings.
use crate::dsp::Compressor;
pub const EQ: [(&str, [f32; 12]); 15] = [
    (
        "Acoustic",
        [2.0, 2.0, 3.0, 2.0, 1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0, 1.0],
    ),
    (
        "Bass Boost",
        [6.0, 5.0, 4.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "Bass Cut",
        [
            -6.0, -5.0, -4.0, -2.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
    ),
    (
        "Classical",
        [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, 0.0, -3.0,
        ],
    ),
    (
        "Electronic",
        [4.0, 3.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 2.0, 3.0, 4.0],
    ),
    (
        "Flat",
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "Hip-Hop",
        [5.0, 4.0, 2.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0],
    ),
    (
        "Jazz",
        [2.0, 1.0, 0.0, 1.0, 2.0, 2.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0],
    ),
    (
        "Loudness",
        [
            5.0, 3.0, 1.0, 0.0, -1.0, -2.0, -2.0, -1.0, 0.0, 1.0, 3.0, 6.0,
        ],
    ),
    (
        "Pop",
        [
            -1.0, 0.0, 2.0, 3.0, 3.0, 2.0, 0.0, -1.0, -1.0, 0.0, 0.0, 0.0,
        ],
    ),
    (
        "Rock",
        [3.0, 2.0, 1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0],
    ),
    (
        "Treble Boost",
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
    ),
    (
        "Treble Cut",
        [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, -2.0, -3.0, -4.0, -5.0, -6.0,
        ],
    ),
    (
        "V-Shape",
        [
            5.0, 3.0, 1.0, 0.0, -1.0, -2.0, -2.0, -1.0, 0.0, 1.0, 3.0, 5.0,
        ],
    ),
    (
        "Vocal Boost",
        [-1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0, 0.0, -1.0],
    ),
];
pub fn compressors() -> Vec<(&'static str, Compressor)> {
    vec![
        (
            "Broadcast",
            Compressor {
                enabled: false,
                attack: 15.0,
                knee: 3.0,
                makeup: 6.0,
                ratio: 5.0,
                release: 200.0,
                threshold: -20.0,
            },
        ),
        (
            "Default",
            Compressor {
                enabled: false,
                attack: 20.0,
                knee: 2.83,
                makeup: 6.0,
                ratio: 4.0,
                release: 250.0,
                threshold: -24.0,
            },
        ),
        (
            "Gentle",
            Compressor {
                enabled: false,
                attack: 50.0,
                knee: 6.0,
                makeup: 2.0,
                ratio: 1.5,
                release: 500.0,
                threshold: -15.0,
            },
        ),
        (
            "Heavy",
            Compressor {
                enabled: false,
                attack: 10.0,
                knee: 2.0,
                makeup: 8.0,
                ratio: 8.0,
                release: 150.0,
                threshold: -30.0,
            },
        ),
        (
            "Light",
            Compressor {
                enabled: false,
                attack: 30.0,
                knee: 4.0,
                makeup: 3.0,
                ratio: 2.0,
                release: 400.0,
                threshold: -18.0,
            },
        ),
        (
            "Limiter",
            Compressor {
                enabled: false,
                attack: 1.0,
                knee: 1.0,
                makeup: 0.0,
                ratio: 20.0,
                release: 100.0,
                threshold: -3.0,
            },
        ),
        (
            "Loud Master",
            Compressor {
                enabled: false,
                attack: 5.0,
                knee: 2.0,
                makeup: 10.0,
                ratio: 6.0,
                release: 100.0,
                threshold: -28.0,
            },
        ),
        (
            "Moderate",
            Compressor {
                enabled: false,
                attack: 20.0,
                knee: 3.0,
                makeup: 5.0,
                ratio: 4.0,
                release: 300.0,
                threshold: -24.0,
            },
        ),
    ]
}
