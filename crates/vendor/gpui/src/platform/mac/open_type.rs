#![allow(unused, non_upper_case_globals)]

use crate::{FontFeatures, FontStyle};
use cocoa::appkit::CGFloat;
use core_foundation::{
    array::{
        CFArray, CFArrayAppendArray, CFArrayAppendValue, CFArrayCreateMutable, CFArrayGetCount,
        CFArrayGetValueAtIndex, CFArrayRef, CFMutableArrayRef, kCFTypeArrayCallBacks,
    },
    base::{CFRelease, TCFType, kCFAllocatorDefault},
    dictionary::CFDictionary as TypedDictionary,
    dictionary::{
        CFDictionaryCreate, kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    },
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use core_foundation_sys::locale::CFLocaleCopyPreferredLanguages;
use core_graphics::{display::CFDictionary, geometry::CGAffineTransform};
use core_text::{
    font::{CTFont, CTFontRef, cascade_list_for_languages},
    font_descriptor::{
        CTFontDescriptor, CTFontDescriptorCopyAttributes, CTFontDescriptorCreateCopyWithFeature,
        CTFontDescriptorCreateWithAttributes, CTFontDescriptorRef, kCTFontCascadeListAttribute,
        kCTFontFeatureSettingsAttribute,
    },
};
use font_kit::font::Font as FontKitFont;
use std::ptr;

pub fn apply_synthetic_italic(font: &mut FontKitFont, style: FontStyle) -> bool {
    if style == FontStyle::Normal || font.properties().style != font_kit::properties::Style::Normal
    {
        return false;
    }
    // Chromium synthesizes italic with a quarter-unit horizontal shear.
    let matrix = CGAffineTransform {
        a: 1.,
        b: 0.,
        c: 0.25,
        d: 1.,
        tx: 0.,
        ty: 0.,
    };
    unsafe {
        let native = CTFontCreateCopyWithAttributes(
            font.native_font().as_concrete_TypeRef(),
            0.,
            &matrix,
            ptr::null(),
        );
        let native = CTFont::wrap_under_create_rule(native);
        *font = FontKitFont::from_native_font(&native);
    }
    true
}

pub fn needs_synthetic_bold(font: &FontKitFont, weight: f32) -> bool {
    if weight < 600. || font.properties().weight.0 >= 600. {
        return false;
    }
    let identifier = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisIdentifierKey) };
    !font.native_font().get_variation_axes().is_some_and(|axes| {
        axes.iter().any(|axis| {
            axis.find(identifier.clone())
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_i64())
                == Some(i64::from(u32::from_be_bytes(*b"wght")))
        })
    })
}

pub fn apply_variable_weight(font: &mut FontKitFont, weight: f32) -> anyhow::Result<bool> {
    apply_variable_axis(font, *b"wght", weight, true)
}

pub fn apply_variable_optical_size(font: &mut FontKitFont, size: f32) -> anyhow::Result<bool> {
    apply_variable_axis(font, *b"opsz", size, false)
}

pub fn apply_variations(
    font: &mut FontKitFont,
    settings: &crate::FontVariations,
) -> anyhow::Result<()> {
    for (tag, value) in settings.iter() {
        apply_variable_axis(font, tag.as_bytes().try_into().unwrap(), value, false)?;
    }
    Ok(())
}

fn apply_variable_axis(
    font: &mut FontKitFont,
    axis_tag: [u8; 4],
    value: f32,
    reset: bool,
) -> anyhow::Result<bool> {
    use anyhow::Context as _;
    let native = font.native_font();
    let Some(axes) = native.get_variation_axes() else {
        return Ok(false);
    };
    let identifier = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisIdentifierKey) };
    let tag = i64::from(u32::from_be_bytes(axis_tag));
    let Some(axis) = axes.iter().find(|axis| {
        axis.find(identifier.clone())
            .and_then(|value| value.downcast::<CFNumber>())
            .and_then(|value| value.to_i64())
            == Some(tag)
    }) else {
        return Ok(false);
    };
    let minimum = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisMinimumValueKey) };
    let maximum = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisMaximumValueKey) };
    let bound = |key| {
        axis.find(key)
            .and_then(|value| value.downcast::<CFNumber>())
            .and_then(|value| value.to_f64())
            .context("Invalid variable font weight bound")
    };
    let minimum = bound(minimum)?;
    let maximum = bound(maximum)?;
    anyhow::ensure!(
        minimum.is_finite() && maximum.is_finite() && minimum <= maximum && value.is_finite(),
        "Invalid variable font weight range"
    );
    let value = f64::from(value).clamp(minimum, maximum);
    unsafe extern "C" {
        fn CTFontCopyVariation(font: CTFontRef) -> core_foundation::dictionary::CFDictionaryRef;
        static kCTFontOpticalSizeAttribute: CFStringRef;
    }
    let mut values = Vec::new();
    if !reset {
        unsafe {
            let existing = CTFontCopyVariation(native.as_concrete_TypeRef());
            if !existing.is_null() {
                let existing =
                    TypedDictionary::<CFNumber, CFNumber>::wrap_under_create_rule(existing);
                let (tags, coordinates) = existing.get_keys_and_values();
                for (axis, coordinate) in tags.into_iter().zip(coordinates) {
                    let axis = CFNumber::wrap_under_get_rule(axis as _);
                    if axis.to_i64() != Some(tag) {
                        values.push((axis, CFNumber::wrap_under_get_rule(coordinate as _)));
                    }
                }
            }
        }
    }
    values.push((CFNumber::from(tag), CFNumber::from(value)));
    let values = TypedDictionary::from_CFType_pairs(&values);
    let attribute = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAttribute) };
    let mut attributes = vec![(attribute, values.as_CFType())];
    if axis_tag == *b"opsz" {
        attributes.push((
            unsafe { CFString::wrap_under_get_rule(kCTFontOpticalSizeAttribute) },
            CFNumber::from(value).as_CFType(),
        ));
    }
    let attributes = TypedDictionary::from_CFType_pairs(&attributes);
    let descriptor = native
        .copy_descriptor()
        .create_copy_with_attributes(attributes.to_untyped())
        .map_err(|()| anyhow::anyhow!("Cannot select variable font weight"))?;
    let native = core_text::font::new_from_descriptor(&descriptor, native.pt_size());
    *font = unsafe { FontKitFont::from_native_font(&native) };
    Ok(true)
}

pub fn apply_features_and_fallbacks(
    font: &mut FontKitFont,
    features: &FontFeatures,
    fallbacks: Option<&[CTFontDescriptor]>,
) -> anyhow::Result<()> {
    unsafe {
        let mut keys = vec![kCTFontFeatureSettingsAttribute];
        let mut values = vec![generate_feature_array(features)];
        if let Some(fallbacks) = fallbacks
            && !fallbacks.is_empty()
        {
            keys.push(kCTFontCascadeListAttribute);
            values.push(generate_fallback_array(
                fallbacks,
                font.native_font().as_concrete_TypeRef(),
            ));
        }
        let attrs = CFDictionaryCreate(
            kCFAllocatorDefault,
            keys.as_ptr() as _,
            values.as_ptr() as _,
            keys.len() as isize,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        values.into_iter().for_each(|value| CFRelease(value as _));
        let new_descriptor = CTFontDescriptorCreateWithAttributes(attrs);
        CFRelease(attrs as _);
        let new_descriptor = CTFontDescriptor::wrap_under_create_rule(new_descriptor);
        let new_font = CTFontCreateCopyWithAttributes(
            font.native_font().as_concrete_TypeRef(),
            0.0,
            std::ptr::null(),
            new_descriptor.as_concrete_TypeRef(),
        );
        let new_font = CTFont::wrap_under_create_rule(new_font);
        *font = font_kit::font::Font::from_native_font(&new_font);

        Ok(())
    }
}

fn generate_feature_array(features: &FontFeatures) -> CFMutableArrayRef {
    unsafe {
        let feature_array = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        for (tag, value) in features.tag_value_list() {
            let keys = [kCTFontOpenTypeFeatureTag, kCTFontOpenTypeFeatureValue];
            let tag = CFString::new(tag);
            let value = CFNumber::from(*value as i32);
            let values = [tag.as_CFTypeRef(), value.as_CFTypeRef()];
            let dict = CFDictionaryCreate(
                kCFAllocatorDefault,
                &keys as *const _ as _,
                &values as *const _ as _,
                2,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            CFArrayAppendValue(feature_array, dict as _);
            CFRelease(dict as _);
        }
        feature_array
    }
}

fn generate_fallback_array(
    fallbacks: &[CTFontDescriptor],
    font_ref: CTFontRef,
) -> CFMutableArrayRef {
    unsafe {
        let fallback_array = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        for descriptor in fallbacks {
            CFArrayAppendValue(fallback_array, descriptor.as_concrete_TypeRef() as _);
        }
        append_system_fallbacks(fallback_array, font_ref);
        fallback_array
    }
}

fn append_system_fallbacks(fallback_array: CFMutableArrayRef, font_ref: CTFontRef) {
    unsafe {
        let preferred_languages: CFArray<CFString> =
            CFArray::wrap_under_create_rule(CFLocaleCopyPreferredLanguages());

        let default_fallbacks = CTFontCopyDefaultCascadeListForLanguages(
            font_ref,
            preferred_languages.as_concrete_TypeRef(),
        );
        let default_fallbacks: CFArray<CTFontDescriptor> =
            CFArray::wrap_under_create_rule(default_fallbacks);

        default_fallbacks
            .iter()
            .filter(|desc| desc.font_path().is_some())
            .map(|desc| {
                CFArrayAppendValue(fallback_array, desc.as_concrete_TypeRef() as _);
            });
    }
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontOpenTypeFeatureTag: CFStringRef;
    static kCTFontOpenTypeFeatureValue: CFStringRef;
    static kCTFontVariationAttribute: CFStringRef;
    static kCTFontVariationAxisIdentifierKey: CFStringRef;
    static kCTFontVariationAxisMinimumValueKey: CFStringRef;
    static kCTFontVariationAxisMaximumValueKey: CFStringRef;

    fn CTFontCreateCopyWithAttributes(
        font: CTFontRef,
        size: CGFloat,
        matrix: *const CGAffineTransform,
        attributes: CTFontDescriptorRef,
    ) -> CTFontRef;
    fn CTFontCopyDefaultCascadeListForLanguages(
        font: CTFontRef,
        languagePrefList: CFArrayRef,
    ) -> CFArrayRef;
}
