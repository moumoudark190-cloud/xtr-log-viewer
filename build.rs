// build.rs
fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/logo.ico");   // path to your .ico file
        res.compile().unwrap();
    }
}
