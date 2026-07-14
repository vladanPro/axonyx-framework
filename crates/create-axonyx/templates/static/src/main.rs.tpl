mod generated;

use std::fs;
use std::path::PathBuf;

use axonyx_runtime::preview_ax_app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout_source = fs::read_to_string("app/layout.ax").ok();
    let page_source = fs::read_to_string("app/page.ax")?;
    let preview_html = preview_ax_app(layout_source.as_deref(), &page_source)?;
    let preview_path = PathBuf::from("target").join("axonyx-preview.html");

    if let Some(parent) = preview_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&preview_path, preview_html)?;

    println!("Axonyx static app '{{APP_NAME}}' is ready.");
    println!("Preview page: {}", preview_path.display());
    println!("Use `cargo ax run dev` for route-aware development.");
    Ok(())
}
