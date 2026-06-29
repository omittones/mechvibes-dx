use std::io;

fn main() -> io::Result<()> {
    // Only compile resources on Windows
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();

        // Set application icon
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let icon_path = std::path::Path::new(&manifest_dir)
            .join("assets/icon.ico")
            .canonicalize()
            .expect("icon.ico not found");
        res.set_icon(icon_path.to_str().unwrap());

        // Set application metadata
        res.set("ProductName", "MechvibesDX");
        res.set(
            "FileDescription",
            "MechvibesDX - Interactive Sound Simulator",
        );
        res.set("CompanyName", "Hai Nguyen");
        res.set("LegalCopyright", "Copyright (C) 2026 Hai Nguyen");

        // Compile the resource file
        res.compile()?;
    }

    Ok(())
}
