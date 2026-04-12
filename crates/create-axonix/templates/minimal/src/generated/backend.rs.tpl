use crate::runtime::AxEnv;

pub fn describe_backend(env: &AxEnv) -> String {
    let app_name = env.public("app_name").unwrap_or("Axonix");
    let db_ready = env.secret("db_url").map(|_| "ready").unwrap_or("missing");

    format!(
        "backend module loaded for {app_name} (db: {db_ready})"
    )
}
