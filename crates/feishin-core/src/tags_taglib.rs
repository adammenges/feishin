//! Small ownership-safe bridge to the same metadata engine used by Electron.
use crate::tags::{Artwork, Fields, Metadata, Patch};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    ptr::NonNull,
};

unsafe extern "C" {
    fn feishin_tags_open(path: *const c_char) -> *mut c_void;
    fn feishin_tags_close(tags: *mut c_void);
    fn feishin_tags_error(tags: *const c_void) -> *const c_char;
    fn feishin_tags_count(tags: *const c_void) -> usize;
    fn feishin_tags_name(tags: *const c_void, index: usize) -> *const c_char;
    fn feishin_tags_values(tags: *const c_void, index: usize) -> usize;
    fn feishin_tags_value(tags: *const c_void, index: usize, value: usize) -> *const c_char;
    fn feishin_tags_artwork(tags: *const c_void, length: *mut usize) -> *const u8;
    fn feishin_tags_set(
        tags: *mut c_void,
        name: *const c_char,
        values: *const *const c_char,
        count: usize,
    ) -> i32;
    fn feishin_tags_picture(
        tags: *mut c_void,
        bytes: *const u8,
        length: usize,
        mime: *const c_char,
    ) -> i32;
    fn feishin_tags_save(tags: *mut c_void) -> i32;
}
struct Native(NonNull<c_void>);
impl Drop for Native {
    fn drop(&mut self) {
        unsafe { feishin_tags_close(self.0.as_ptr()) };
    }
}
fn string(pointer: *const c_char) -> Result<String> {
    ensure!(
        !pointer.is_null(),
        "Metadata library returned an invalid string"
    );
    // The C++ bridge owns this null-terminated string until its handle is dropped.
    Ok(unsafe { CStr::from_ptr(pointer) }.to_str()?.to_owned())
}
impl Native {
    fn open(path: &Path) -> Result<Self> {
        let path = CString::new(
            path.to_str()
                .context("Audio file path is not valid UTF-8")?,
        )?;
        let pointer = unsafe { feishin_tags_open(path.as_ptr()) };
        let native = NonNull::new(pointer).map(Self);
        let error = string(unsafe { feishin_tags_error(pointer) })?;
        ensure!(error.is_empty(), "{error}");
        native.context("Could not open audio metadata")
    }
    fn checked(&self, result: i32) -> Result<()> {
        ensure!(
            result == 1,
            "{}",
            string(unsafe { feishin_tags_error(self.0.as_ptr()) })?
        );
        Ok(())
    }
}
pub fn read(path: &Path) -> Result<Metadata> {
    let native = Native::open(path)?;
    let pointer = native.0.as_ptr();
    let count = unsafe { feishin_tags_count(pointer) };
    ensure!(count <= 1000, "Audio file has too many metadata fields");
    let mut fields = Fields::new();
    let mut total = 0;
    for index in 0..count {
        let name = string(unsafe { feishin_tags_name(pointer, index) })?;
        let count = unsafe { feishin_tags_values(pointer, index) };
        ensure!(count <= 10_000, "Audio metadata field has too many values");
        let mut values = Vec::with_capacity(count);
        for value in 0..count {
            let value = string(unsafe { feishin_tags_value(pointer, index, value) })?;
            total += value.len();
            ensure!(total <= 20_000_000, "Audio metadata is too large");
            values.push(value);
        }
        fields.insert(name, values);
    }
    let mut length = 0;
    let bytes = unsafe { feishin_tags_artwork(pointer, &mut length) };
    ensure!(
        length <= 20_000_000 && (length == 0 || !bytes.is_null()),
        "Invalid cover artwork"
    );
    // ByteVector owns exactly `length` initialized bytes while `native` is alive.
    let artwork =
        (length > 0).then(|| unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec());
    Ok(Metadata {
        path: path.to_owned(),
        fields,
        artwork,
    })
}
pub fn write(path: &Path, patch: &Patch) -> Result<()> {
    let native = Native::open(path)?;
    for (name, values) in &patch.fields {
        ensure!(
            !name.is_empty() && !name.contains(['=', '\0', '\n']),
            "Invalid tag name"
        );
        let name = CString::new(name.as_str())?;
        let values = values
            .iter()
            .map(|value| CString::new(value.as_str()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let pointers = values
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>();
        native.checked(unsafe {
            feishin_tags_set(
                native.0.as_ptr(),
                name.as_ptr(),
                pointers.as_ptr(),
                pointers.len(),
            )
        })?;
    }
    match &patch.artwork {
        Artwork::Keep => {}
        Artwork::Remove => {
            native.checked(unsafe {
                feishin_tags_picture(native.0.as_ptr(), std::ptr::null(), 0, std::ptr::null())
            })?;
        }
        Artwork::Replace(data) => {
            let mime = CString::new(crate::tags::artwork_mime(data)?)?;
            native.checked(unsafe {
                feishin_tags_picture(native.0.as_ptr(), data.as_ptr(), data.len(), mime.as_ptr())
            })?;
        }
    }
    native.checked(unsafe { feishin_tags_save(native.0.as_ptr()) })
}
