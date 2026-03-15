use std::fs;
use std::path::Path;

fn main() {
    let dist = Path::new("web/dist");
    if !dist.exists() {
        fs::create_dir_all(dist).expect("Failed to create web/dist directory");
        fs::write(
            dist.join("index.html"),
            "<html><body>Run <code>npm run build</code> in web/ to build the viewer.</body></html>",
        )
        .expect("Failed to write placeholder index.html");
    }
}
