use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AxRuntimeError {
    #[error("runtime operation failed: {message}")]
    Message { message: String },
}

impl AxRuntimeError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

pub type AxRuntimeResult<T> = Result<T, AxRuntimeError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxQueryRequest {
    pub collection: String,
    pub filters: Vec<AxQueryFilterRequest>,
    pub orders: Vec<AxQueryOrderRequest>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxQueryFilterRequest {
    pub field: String,
    pub op: AxQueryFilterOp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxQueryFilterOp {
    Eq,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxQueryOrderRequest {
    pub field: String,
    pub direction: AxQueryOrderDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxQueryOrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxInsertRequest {
    pub collection: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxUpdateRequest {
    pub collection: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxSendRequest {
    pub target: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxEnv {
    pub public: BTreeMap<String, String>,
    pub secret: BTreeMap<String, String>,
}

impl AxEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_env() -> Self {
        let mut env = Self::new();

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

    pub fn with_public(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.public.insert(key.into(), value.into());
        self
    }

    pub fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.secret.insert(key.into(), value.into());
        self
    }

    pub fn public(&self, key: &str) -> AxRuntimeResult<String> {
        self.public
            .get(key)
            .cloned()
            .ok_or_else(|| AxRuntimeError::message(format!("missing public env key `{key}`")))
    }

    pub fn secret(&self, key: &str) -> AxRuntimeResult<String> {
        self.secret
            .get(key)
            .cloned()
            .ok_or_else(|| AxRuntimeError::message(format!("missing secret env key `{key}`")))
    }
}

fn normalize_env_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

pub trait AxRuntimeEnvAccess {
    fn env(&self) -> &AxEnv;
}

pub trait AxQueryExecutor {
    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value>;
}

pub trait AxMutationExecutor {
    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value>;
    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value>;
}

pub trait AxRevalidator {
    fn revalidate(&self, target: &str) -> AxRuntimeResult<()>;
}

pub trait AxMessenger {
    fn send(&self, request: &AxSendRequest) -> AxRuntimeResult<()>;
}

pub trait AxBackendRuntime:
    AxQueryExecutor + AxMutationExecutor + AxRevalidator + AxMessenger + AxRuntimeEnvAccess
{
}

impl<T> AxBackendRuntime for T where
    T: AxQueryExecutor + AxMutationExecutor + AxRevalidator + AxMessenger + AxRuntimeEnvAccess
{
}

pub fn ok_payload() -> Value {
    json!({ "ok": true })
}

pub mod prelude {
    pub use super::ok_payload;
    pub use super::AxBackendRuntime;
    pub use super::AxEnv;
    pub use super::AxInsertRequest;
    pub use super::AxMessenger;
    pub use super::AxMutationExecutor;
    pub use super::AxQueryExecutor;
    pub use super::AxQueryFilterOp;
    pub use super::AxQueryFilterRequest;
    pub use super::AxQueryOrderDirection;
    pub use super::AxQueryOrderRequest;
    pub use super::AxQueryRequest;
    pub use super::AxRevalidator;
    pub use super::AxRuntimeError;
    pub use super::AxRuntimeEnvAccess;
    pub use super::AxRuntimeResult;
    pub use super::AxSendRequest;
    pub use super::AxUpdateRequest;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryRuntime {
        env: AxEnv,
    }

    impl AxRuntimeEnvAccess for MemoryRuntime {
        fn env(&self) -> &AxEnv {
            &self.env
        }
    }

    impl AxQueryExecutor for MemoryRuntime {
        fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
            Ok(json!({
                "collection": request.collection,
                "limit": request.limit,
            }))
        }
    }

    impl AxMutationExecutor for MemoryRuntime {
        fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
            Ok(json!({
                "inserted": request.collection,
                "fields": request.fields,
            }))
        }

        fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
            Ok(json!({
                "updated": request.collection,
                "fields": request.fields,
            }))
        }
    }

    impl AxRevalidator for MemoryRuntime {
        fn revalidate(&self, target: &str) -> AxRuntimeResult<()> {
            if target.is_empty() {
                return Err(AxRuntimeError::message("missing revalidation target"));
            }
            Ok(())
        }
    }

    impl AxMessenger for MemoryRuntime {
        fn send(&self, request: &AxSendRequest) -> AxRuntimeResult<()> {
            if request.target.is_empty() {
                return Err(AxRuntimeError::message("missing send target"));
            }
            Ok(())
        }
    }

    #[test]
    fn memory_runtime_can_execute_query_contract() {
        let runtime = MemoryRuntime::default();
        let result = runtime
            .load(&AxQueryRequest {
                collection: "posts".to_string(),
                filters: vec![AxQueryFilterRequest {
                    field: "status".to_string(),
                    op: AxQueryFilterOp::Eq,
                    value: json!("published"),
                }],
                orders: vec![AxQueryOrderRequest {
                    field: "created_at".to_string(),
                    direction: AxQueryOrderDirection::Desc,
                }],
                limit: Some(20),
                offset: None,
            })
            .expect("query should execute");

        assert_eq!(
            result,
            json!({
                "collection": "posts",
                "limit": 20,
            })
        );
    }

    #[test]
    fn ok_payload_returns_framework_success_shape() {
        assert_eq!(ok_payload(), json!({ "ok": true }));
    }

    #[test]
    fn env_access_can_read_public_and_secret_values() {
        let runtime = MemoryRuntime {
            env: AxEnv::new()
                .with_public("app_name", "Axonix")
                .with_secret("db_url", "postgres://local/axonix"),
        };

        assert_eq!(
            runtime.env().public("app_name").expect("public key should exist"),
            "Axonix".to_string()
        );
        assert_eq!(
            runtime.env().secret("db_url").expect("secret key should exist"),
            "postgres://local/axonix".to_string()
        );
    }

    #[test]
    fn from_env_collects_ax_public_and_secret_namespaces() {
        let public_prev = std::env::var("AX_PUBLIC_APP_NAME").ok();
        let secret_prev = std::env::var("AX_SECRET_DB_URL").ok();

        std::env::set_var("AX_PUBLIC_APP_NAME", "Axonix");
        std::env::set_var("AX_SECRET_DB_URL", "postgres://local/axonix");

        let env = AxEnv::from_env();

        assert_eq!(
            env.public("app_name").expect("public key should exist"),
            "Axonix".to_string()
        );
        assert_eq!(
            env.secret("db_url").expect("secret key should exist"),
            "postgres://local/axonix".to_string()
        );

        if let Some(value) = public_prev {
            std::env::set_var("AX_PUBLIC_APP_NAME", value);
        } else {
            std::env::remove_var("AX_PUBLIC_APP_NAME");
        }

        if let Some(value) = secret_prev {
            std::env::set_var("AX_SECRET_DB_URL", value);
        } else {
            std::env::remove_var("AX_SECRET_DB_URL");
        }
    }
}
