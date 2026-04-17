mod db;
mod generated;

use axonyx_runtime::backend_prelude::AxEnv;
use db::db_url;

fn main() {
    let env = AxEnv::from_env();

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
    println!("Generated backend file: src/generated/backend.rs");
    println!("UI entry: app/page.ax");
    println!("Posts loader: app/posts/loader.ax");
    println!("Posts actions: app/posts/actions.ax");
    println!("API route: routes/api/posts.ax");
}
