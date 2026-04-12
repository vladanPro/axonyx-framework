use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct AxEnv {
    public: BTreeMap<String, String>,
    secret: BTreeMap<String, String>,
}

impl AxEnv {
    pub fn from_env() -> Self {
        load_dotenv_file(".env");

        let mut env = Self::default();
        for (key, value) in std::env::vars() {
            if let Some(public_key) = key.strip_prefix("AX_PUBLIC_") {
                env.public.insert(normalize_env_key(public_key), value);
                continue;
            }

            if let Some(secret_key) = key.strip_prefix("AX_SECRET_") {
                env.secret.insert(normalize_env_key(secret_key), value);
            }
        }

        env
    }

    pub fn public(&self, key: &str) -> Option<&str> {
        self.public.get(key).map(String::as_str)
    }

    pub fn secret(&self, key: &str) -> Option<&str> {
        self.secret.get(key).map(String::as_str)
    }
}

fn normalize_env_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

fn load_dotenv_file(path: impl AsRef<Path>) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if key.is_empty() {
            continue;
        }

        if std::env::var_os(key).is_none() {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }
}
