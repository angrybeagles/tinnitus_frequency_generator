fn main() {
    // On Windows release builds, set the subsystem to "windows" to hide the console
    #[cfg(windows)]
    {
        if std::env::var("PROFILE").unwrap_or_default() == "release" {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/icon.ico"); // Optional: set app icon
            let _ = res.compile(); // Don't fail if icon is missing
        }
    }
}
