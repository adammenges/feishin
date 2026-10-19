use std::{env, fs, io::Read, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    build_taglib(&root);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    build_fonts(&root, &out);
    let locale_dir = root.join("crates/feishin-core/assets/locales");
    println!("cargo:rerun-if-changed={}", locale_dir.display());
    let mut locales = fs::read_dir(locale_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    locales.sort();
    let mut code = String::from("pub const LOCALES: &[(&str, &str)] = &[\n");
    for path in locales {
        let id = path.file_stem().unwrap().to_str().unwrap();
        code.push_str(&format!(
            "({id:?}, include_str!({:?})),\n",
            path.canonicalize().unwrap()
        ));
    }
    code.push_str("];\n");
    fs::write(out.join("locales.rs"), code).unwrap();
}

fn build_taglib(root: &std::path::Path) {
    let source = root.join("crates/vendor/taglib");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed=native/taglib.cpp");
    let target = env::var("TARGET").unwrap();
    let mut config = cmake::Config::new(source);
    config
        .profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("BUILD_BINDINGS", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("BUILD_EXAMPLES", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON");
    if target.contains("windows") {
        let vcpkg = PathBuf::from(
            env::var_os("VCPKG_INSTALLATION_ROOT")
                .expect("TagLib needs VCPKG_INSTALLATION_ROOT on Windows"),
        );
        config
            .define(
                "CMAKE_TOOLCHAIN_FILE",
                vcpkg.join("scripts/buildsystems/vcpkg.cmake"),
            )
            .define("VCPKG_MANIFEST_MODE", "OFF")
            .define("VCPKG_TARGET_TRIPLET", "x64-windows-static-md")
            .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL");
        println!(
            "cargo:rustc-link-search=native={}",
            vcpkg.join("installed/x64-windows-static-md/lib").display()
        );
    }
    let installed = config.build();
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(installed.join("include/taglib"))
        .file("native/taglib.cpp")
        .compile("feishin_taglib");
    println!(
        "cargo:rustc-link-search=native={}",
        installed.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=tag");
    println!(
        "cargo:rustc-link-lib={}",
        if target.contains("windows") {
            "static=zlib"
        } else {
            "z"
        }
    );
}

// Bundled WOFF2 files are trusted build inputs. Embed native OpenType bytes so
// opening the app never decompresses the same twenty fonts on the UI thread.
fn build_fonts(root: &std::path::Path, out: &std::path::Path) {
    let sources = [
        "../../../assets/fonts/Inter-VariableFont_opsz,wght.woff2",
        "../../../assets/fonts/Poppins-Regular.woff2",
        "../../../assets/fonts/Poppins-Bold.woff2",
        "../../../assets/fonts/Poppins-SemiBold.woff2",
        "../../../assets/fonts/Poppins-Medium.woff2",
        "../../../assets/fonts/Poppins-ExtraBold.woff2",
        "../../../assets/fonts/Poppins-Black.woff2",
        "../assets/fonts/noto-jp-400.woff2",
        "../assets/fonts/noto-jp-500.woff2",
        "../assets/fonts/noto-jp-600.woff2",
        "../assets/fonts/noto-jp-700.woff2",
        "../assets/fonts/noto-jp-800.woff2",
        "../assets/fonts/noto-hebrew-400.woff2",
        "../assets/fonts/noto-hebrew-500.woff2",
        "../assets/fonts/noto-hebrew-600.woff2",
        "../assets/fonts/noto-hebrew-700.woff2",
        "../assets/fonts/noto-hebrew-800.woff2",
        "../assets/fonts/noto-jp-variable.woff2",
        "../assets/fonts/noto-hebrew-variable.woff2",
        "../../../assets/fonts/Symbola.woff2",
    ];
    let mut code = String::from("pub const BUILTIN_FONT_BYTES: &[&[u8]] = &[\n");
    let mut originals = String::from("#[cfg(test)] const BUILTIN_FONT_SOURCES: &[&[u8]] = &[\n");
    for (index, source) in sources.iter().enumerate() {
        let source = root
            .join("crates/feishin-core/src")
            .join(source)
            .canonicalize()
            .unwrap();
        println!("cargo:rerun-if-changed={}", source.display());
        let bytes = fs::read(&source).unwrap();
        let decoded =
            wuff::decompress_woff2_with_custom_brotli(&bytes, &mut |compressed, expected| {
                if expected > 32_000_000 {
                    return Err("Bundled font exceeds 32 MB".into());
                }
                let mut bytes = Vec::new();
                brotli_decompressor::Decompressor::new(compressed, 4096)
                    .take(32_000_001)
                    .read_to_end(&mut bytes)?;
                if bytes.len() != expected {
                    return Err("Invalid bundled font length".into());
                }
                Ok(bytes)
            })
            .expect("Invalid bundled WOFF2 font");
        let destination = out.join(format!("builtin-font-{index}.ttf"));
        fs::write(&destination, decoded).unwrap();
        code.push_str(&format!("include_bytes!({destination:?}),\n"));
        originals.push_str(&format!("include_bytes!({source:?}),\n"));
    }
    code.push_str("];\n");
    originals.push_str("];\n");
    fs::write(out.join("builtin_fonts.rs"), code + &originals).unwrap();
}
