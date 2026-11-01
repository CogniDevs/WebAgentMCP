fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        if std::path::Path::new("resources/icon.ico").exists() {
            res.set_icon("resources/icon.ico");
        }
        let _ = res.compile();
    }
}