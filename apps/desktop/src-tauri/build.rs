fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest_path =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set"))
                .join("test-common-controls.manifest");
        std::fs::write(
            &manifest_path,
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df"
        language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
        )
        .expect("write Windows test manifest");
        println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }

    tauri_build::build()
}
