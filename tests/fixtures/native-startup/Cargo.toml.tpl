[package]
name = "axonyx-native-startup-smoke"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
axonyx-runtime = { path = "{{RUNTIME_PATH}}", features = ["axum"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
