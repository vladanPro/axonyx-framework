mod db;
mod generated;
mod runtime;

use db::db_url;
use generated::backend::describe_backend;
use runtime::AxEnv;

fn main() {
    let env = AxEnv::from_env();

    println!("Axonix app '{{APP_NAME}}' is ready.");
    println!(
        "Public app name: {}",
        env.public("app_name").unwrap_or("{{APP_NAME}}")
    );
    println!(
        "Database configured: {}",
        env.secret("db_url").map(|_| "yes").unwrap_or("no")
    );
    println!("DB helper sees URL: {}", db_url(&env).map(|_| "yes").unwrap_or("no"));
    println!("Generated backend: {}", describe_backend(&env));
    println!("UI entry: app/page.ax");
    println!("Posts loader: app/posts/loader.ax");
    println!("Posts actions: app/posts/actions.ax");
    println!("API route: routes/api/posts.ax");
}
