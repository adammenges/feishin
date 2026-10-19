# Native ICC compatibility patch

Source: unmodified crates.io moxcms 0.8.1, upstream revision
`c4affa196727d17f21a7f6f148a558867decb470`.
The upstream crate archive SHA256 is
`bb85c154ba489f01b25c0d36ae69a87e4a1c73a72631fc6c0eb6dde34a73e44b`.
Original BSD-3-Clause copyright and license text are retained in LICENSE.md.

The CMYK branch of `src/conversions/lut_transforms.rs` uses the existing staged
transform rather than fusing Lab/XYZ and nonlinear RGB conversion into a
17-point lookup table. This avoids visible rounding differences in tagged JPEG
artwork. The staged CMYK transform also omits the ICC v2 Lab rescaling, matching
Chromium's skcms interpretation of those profiles. Other color spaces retain
upstream behavior. GPUI enables the existing `any_to_any` feature for this path.

The host application's twenty-one independent Chromium fixtures compare tagged
and untagged CMYK/YCCK JPEG, RGB/grayscale PNG/JPEG/WebP, invalid profiles and
prepared backgrounds. RGB channel tolerance is two levels; alpha is exact.
The libjpeg fixture encoder and Chromium canvas provide the reference pixels,
independently of the native decoder and color transform.
