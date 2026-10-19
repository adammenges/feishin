use anyhow::{Result, ensure};
use windows::{Win32::Graphics::DirectWrite::*, core::Interface};

pub(super) fn instance_axes(
    face: &IDWriteFontFace3,
    settings: &crate::FontVariations,
    optical_size: Option<f32>,
) -> Result<Vec<DWRITE_FONT_AXIS_VALUE>> {
    let Ok(variable) = face.cast::<IDWriteFontFace5>() else {
        return Ok(vec![]);
    };
    let coordinates = axes(face)?;
    let mut ranges = vec![DWRITE_FONT_AXIS_RANGE::default(); coordinates.len()];
    unsafe {
        variable.GetFontResource()?.GetFontAxisRanges(&mut ranges)?;
    }
    let mut values = coordinates
        .into_iter()
        .map(|(tag, bits)| {
            let mut value = f32::from_bits(bits);
            let bytes = tag.to_le_bytes();
            let name = std::str::from_utf8(&bytes).unwrap_or("");
            if let Some(range) = ranges.iter().find(|range| range.axisTag.0 == tag)
                && range.minValue.is_finite()
                && range.maxValue.is_finite()
                && range.minValue < range.maxValue
                && let Some(requested) = settings.get(name).or_else(|| {
                    (tag == DWRITE_FONT_AXIS_TAG_OPTICAL_SIZE.0)
                        .then_some(optical_size)
                        .flatten()
                })
            {
                value = requested.clamp(range.minValue, range.maxValue);
            }
            DWRITE_FONT_AXIS_VALUE {
                axisTag: DWRITE_FONT_AXIS_TAG(tag),
                value,
            }
        })
        .collect::<Vec<_>>();
    if !values
        .iter()
        .any(|axis| axis.axisTag == DWRITE_FONT_AXIS_TAG_OPTICAL_SIZE)
        && let Some(size) = settings.get("opsz").or(optical_size)
    {
        values.push(DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_OPTICAL_SIZE,
            value: size,
        });
    }
    Ok(values)
}

pub(super) fn apply_to_face(
    face: &IDWriteFontFace3,
    settings: &crate::FontVariations,
    optical_size: Option<f32>,
) -> Result<IDWriteFontFace3> {
    let Ok(variable) = face.cast::<IDWriteFontFace5>() else {
        return Ok(face.clone());
    };
    let values = instance_axes(face, settings, optical_size)?;
    Ok(unsafe {
        variable
            .GetFontResource()?
            .CreateFontFace(face.GetSimulations(), &values)?
    }
    .cast()?)
}

pub(super) fn enable_variations(
    format: &IDWriteTextFormat1,
    face: &IDWriteFontFace3,
    settings: &crate::FontVariations,
    size: f32,
) -> Result<()> {
    if let Ok(format) = format.cast::<IDWriteTextFormat3>() {
        unsafe {
            format.SetAutomaticFontAxes(DWRITE_AUTOMATIC_FONT_AXES_NONE)?;
            format.SetFontAxisValues(&instance_axes(face, settings, Some(size))?)?;
        }
    }
    Ok(())
}

pub(super) fn axes(face: &IDWriteFontFace3) -> Result<Vec<(u32, u32)>> {
    let Ok(face) = face.cast::<IDWriteFontFace5>() else {
        return Ok(vec![]);
    };
    let count = unsafe { face.GetFontAxisValueCount() } as usize;
    ensure!(count <= 256, "Too many native font variation axes");
    if count == 0 {
        return Ok(vec![]);
    }
    let mut values = vec![DWRITE_FONT_AXIS_VALUE::default(); count];
    unsafe {
        face.GetFontAxisValues(&mut values)?;
    }
    let mut axes = values
        .into_iter()
        .map(|axis| (axis.axisTag.0, axis.value.to_bits()))
        .collect::<Vec<_>>();
    axes.sort_unstable();
    Ok(axes)
}

pub(super) fn synthesize_style(
    face: IDWriteFontFace3,
    italic: bool,
    weight: f32,
) -> Result<IDWriteFontFace3> {
    unsafe {
        let mut simulations = face.GetSimulations().0;
        if italic && face.GetStyle() == DWRITE_FONT_STYLE_NORMAL {
            simulations |= DWRITE_FONT_SIMULATIONS_OBLIQUE.0;
        }
        if weight >= 600.
            && face.GetWeight().0 < 600
            && !axes(&face)?
                .iter()
                .any(|(tag, _)| *tag == DWRITE_FONT_AXIS_TAG_WEIGHT.0)
        {
            simulations |= DWRITE_FONT_SIMULATIONS_BOLD.0;
        }
        if simulations == face.GetSimulations().0 {
            return Ok(face);
        }
        Ok(face
            .GetFontFaceReference()?
            .CreateFontFaceWithSimulations(DWRITE_FONT_SIMULATIONS(simulations))?)
    }
}
