use anyhow::Result;

/// ScreenCaptureKit supplies interleaved stereo or separate float channel planes.
pub fn stereo_f32(buffers: &[&[u8]], big_endian: bool) -> Result<Vec<f32>> {
    use anyhow::ensure;
    ensure!(
        (1..=2).contains(&buffers.len()),
        "Unexpected system-audio channel layout"
    );
    ensure!(
        buffers
            .iter()
            .all(|bytes| bytes.len() <= 1024 * 1024 && bytes.len().is_multiple_of(4)),
        "Invalid system-audio sample buffer"
    );
    let decode = |bytes: &[u8]| -> f32 {
        let bytes: [u8; 4] = bytes.try_into().unwrap();
        if big_endian {
            f32::from_be_bytes(bytes)
        } else {
            f32::from_le_bytes(bytes)
        }
    };
    if buffers.len() == 1 {
        ensure!(
            buffers[0].len().is_multiple_of(8),
            "Incomplete stereo capture frame"
        );
        Ok(buffers[0]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| decode(bytes))
            .collect())
    } else {
        ensure!(
            buffers[0].len() == buffers[1].len(),
            "Mismatched system-audio channel planes"
        );
        Ok(buffers[0]
            .as_chunks::<4>()
            .0
            .iter()
            .zip(buffers[1].as_chunks::<4>().0)
            .flat_map(|(left, right)| [decode(left), decode(right)])
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stereo_planes_endianness_and_expansion_limits() {
        let left: Vec<_> = [0.1_f32, 0.2, 0.3]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let right: Vec<_> = [0.4_f32, 0.5, 0.6]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        assert_eq!(
            stereo_f32(&[&left, &right], false).unwrap(),
            [0.1, 0.4, 0.2, 0.5, 0.3, 0.6]
        );
        let packed: Vec<_> = [0.1_f32, 0.4, 0.2, 0.5]
            .into_iter()
            .flat_map(f32::to_be_bytes)
            .collect();
        assert_eq!(stereo_f32(&[&packed], true).unwrap(), [0.1, 0.4, 0.2, 0.5]);
        assert!(stereo_f32(&[&left], false).is_err());
        assert!(stereo_f32(&[&left, &right[..4]], false).is_err());
        assert!(stereo_f32(&[&vec![0; 1024 * 1024 + 8]], false).is_err());
    }
}
