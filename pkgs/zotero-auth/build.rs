fn main() {
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=TAILWIND_CLI");

    topcoat::tailwind::BuildConfig::new()
        .executable_env("TAILWIND_CLI")
        // Keep the asset path stable across Cargo target triples. Topcoat hashes
        // the declared path into its asset ID, while stylesheet!() embeds OUT_DIR.
        // The Nix build and Topcoat's bundler use different target directories.
        .output("target/topcoat/tailwind.css")
        .render()
        .expect("failed to build the Tailwind stylesheet");
}
