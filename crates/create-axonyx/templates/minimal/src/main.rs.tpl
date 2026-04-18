mod db;
mod generated;

use std::fs;
use std::path::PathBuf;

use axonyx_runtime::preview_ax_app;
use axonyx_runtime::backend_prelude::AxEnv;
use db::db_url;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = AxEnv::from_env();
    let layout_source = fs::read_to_string("app/layout.ax").ok();
    let page_source = fs::read_to_string("app/page.ax")?;
    let preview_html = preview_ax_app(layout_source.as_deref(), &page_source)?;
    let preview_path = preview_path();

    if let Some(parent) = preview_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&preview_path, preview_html)?;

    println!("Axonyx app '{{APP_NAME}}' is ready.");
    println!(
        "Public app name: {}",
        env.public("app_name")
            .unwrap_or_else(|_| "{{APP_NAME}}".to_string())
    );
    println!(
        "Database configured: {}",
        env.secret("db_url").map(|_| "yes").unwrap_or("no")
    );
    println!("DB helper sees URL: {}", db_url(&env).map(|_| "yes").unwrap_or("no"));
    println!("Preview page: {}", preview_path.display());
    println!("Generated backend file: src/generated/backend.rs");
    println!("UI shell: app/layout.ax");
    println!("UI entry: app/page.ax");
    println!("Posts loader: app/posts/loader.ax");
    println!("Posts actions: app/posts/actions.ax");
    println!("API route: routes/api/posts.ax");
    Ok(())
}

fn preview_path() -> PathBuf {
    PathBuf::from("target").join("axonyx-preview.html")
}
