use std::{env, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=TAILWIND_CLI");
    println!("cargo:rerun-if-env-changed=ZOTERO_WEB_LIBRARY");

    configure_web_library();

    topcoat::tailwind::BuildConfig::new()
        .executable_env("TAILWIND_CLI")
        // Keep the asset path stable across Cargo target triples. Topcoat hashes
        // the declared path into its asset ID, while stylesheet!() embeds OUT_DIR.
        // The Nix build and Topcoat's bundler use different target directories.
        .output("target/topcoat/tailwind.css")
        .render()
        .expect("failed to build the Tailwind stylesheet");
}

fn configure_web_library() {
    let Some(package) = env::var_os("ZOTERO_WEB_LIBRARY") else {
        println!(
            "cargo:warning=ZOTERO_WEB_LIBRARY is unset; the embedded Web Library will be unavailable"
        );
        println!("cargo:rustc-env=ZOTERO_WEB_LIBRARY_DIR=");
        return;
    };
    let static_dir = Path::new(&package).join("share/zotero-web-library/static");
    for required in [
        "zotero-web-library.css",
        "zotero-web-library.js",
        "reader/reader.html",
        "note-editor/editor.html",
        "pdf-worker/worker.js",
    ] {
        let path = static_dir.join(required);
        assert!(
            path.is_file(),
            "ZOTERO_WEB_LIBRARY is missing {}",
            path.display()
        );
    }
    println!(
        "cargo:rustc-env=ZOTERO_WEB_LIBRARY_DIR={}",
        static_dir.display()
    );
}
