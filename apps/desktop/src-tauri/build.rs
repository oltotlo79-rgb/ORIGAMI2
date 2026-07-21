fn main() {
    #[cfg(target_os = "windows")]
    {
        // A Rust test harness has no application manifest, so eagerly importing
        // the Common Controls v6-only TaskDialog entry point prevents the harness
        // from starting. Delay loading keeps native tests independent of that UI
        // activation context; the packaged application still supplies its normal
        // Tauri manifest before any dialog is opened.
        println!("cargo::rustc-link-lib=delayimp");
        println!("cargo::rustc-link-arg=/DELAYLOAD:comctl32.dll");
    }

    tauri_build::build()
}
