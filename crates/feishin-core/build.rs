use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    build_taglib(&root);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
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
