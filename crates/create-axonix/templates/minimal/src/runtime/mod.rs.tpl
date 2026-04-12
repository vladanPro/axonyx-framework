use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

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

    pub fn database_driver(&self) -> &str {
        self.secret("db_driver").unwrap_or("postgres")
    }

    pub fn database_url(&self) -> Option<&str> {
        self.secret("db_url")
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

pub type AxRuntimeResult<T> = Result<T, AxRuntimeError>;

#[derive(Debug, Clone)]
pub struct AxRuntimeError {
    message: String,
}

impl AxRuntimeError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AxRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AxRuntimeError {}

#[derive(Debug, Clone)]
pub struct AxQueryRequest {
    pub collection: String,
    pub filters: Vec<AxQueryFilterRequest>,
    pub orders: Vec<AxQueryOrderRequest>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AxQueryFilterRequest {
    pub field: String,
    pub op: AxQueryFilterOp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxQueryFilterOp {
    Eq,
}

#[derive(Debug, Clone)]
pub struct AxQueryOrderRequest {
    pub field: String,
    pub direction: AxQueryOrderDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxQueryOrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct AxInsertRequest {
    pub collection: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct AxUpdateRequest {
    pub collection: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct AxSendRequest {
    pub target: String,
    pub payload: Value,
}

pub trait AxRuntimeEnvAccess {
    fn env(&self) -> &AxEnv;
}

pub trait AxDatabaseAdapter {
    fn driver(&self) -> &'static str;
    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value>;
    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value>;
    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value>;
}

impl<T> AxDatabaseAdapter for Box<T>
where
    T: AxDatabaseAdapter + ?Sized,
{
    fn driver(&self) -> &'static str {
        (**self).driver()
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        (**self).load(request)
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        (**self).insert(request)
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        (**self).update(request)
    }
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

pub struct AxDatabaseRuntime<A> {
    env: AxEnv,
    adapter: A,
}

impl<A> AxDatabaseRuntime<A> {
    pub fn new(env: AxEnv, adapter: A) -> Self {
        Self { env, adapter }
    }
}

impl<A> AxRuntimeEnvAccess for AxDatabaseRuntime<A> {
    fn env(&self) -> &AxEnv {
        &self.env
    }
}

impl<A> AxQueryExecutor for AxDatabaseRuntime<A>
where
    A: AxDatabaseAdapter,
{
    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        self.adapter.load(request)
    }
}

impl<A> AxMutationExecutor for AxDatabaseRuntime<A>
where
    A: AxDatabaseAdapter,
{
    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        self.adapter.insert(request)
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        self.adapter.update(request)
    }
}

impl<A> AxRevalidator for AxDatabaseRuntime<A> {
    fn revalidate(&self, _target: &str) -> AxRuntimeResult<()> {
        Ok(())
    }
}

impl<A> AxMessenger for AxDatabaseRuntime<A> {
    fn send(&self, _request: &AxSendRequest) -> AxRuntimeResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresAdapter {
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MySqlAdapter {
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteAdapter {
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryAdapter;

impl AxDatabaseAdapter for PostgresAdapter {
    fn driver(&self) -> &'static str {
        "postgres"
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        Ok(adapter_payload(self.driver(), &self.url, request.collection.clone(), serde_json::json!({
            "filters": request.filters.iter().map(query_filter_payload).collect::<Vec<_>>(),
            "orders": request.orders.iter().map(query_order_payload).collect::<Vec<_>>(),
            "limit": request.limit,
            "offset": request.offset,
        })))
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "insert", &request.collection, &request.fields))
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "update", &request.collection, &request.fields))
    }
}

impl AxDatabaseAdapter for MySqlAdapter {
    fn driver(&self) -> &'static str {
        "mysql"
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        Ok(adapter_payload(self.driver(), &self.url, request.collection.clone(), serde_json::json!({
            "filters": request.filters.iter().map(query_filter_payload).collect::<Vec<_>>(),
            "orders": request.orders.iter().map(query_order_payload).collect::<Vec<_>>(),
            "limit": request.limit,
            "offset": request.offset,
        })))
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "insert", &request.collection, &request.fields))
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "update", &request.collection, &request.fields))
    }
}

impl AxDatabaseAdapter for SqliteAdapter {
    fn driver(&self) -> &'static str {
        "sqlite"
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        Ok(adapter_payload(self.driver(), &self.url, request.collection.clone(), serde_json::json!({
            "filters": request.filters.iter().map(query_filter_payload).collect::<Vec<_>>(),
            "orders": request.orders.iter().map(query_order_payload).collect::<Vec<_>>(),
            "limit": request.limit,
            "offset": request.offset,
        })))
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "insert", &request.collection, &request.fields))
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &self.url, "update", &request.collection, &request.fields))
    }
}

impl AxDatabaseAdapter for MemoryAdapter {
    fn driver(&self) -> &'static str {
        "memory"
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        Ok(adapter_payload(self.driver(), &None, request.collection.clone(), serde_json::json!({
            "filters": request.filters.iter().map(query_filter_payload).collect::<Vec<_>>(),
            "orders": request.orders.iter().map(query_order_payload).collect::<Vec<_>>(),
            "limit": request.limit,
            "offset": request.offset,
        })))
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &None, "insert", &request.collection, &request.fields))
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), &None, "update", &request.collection, &request.fields))
    }
}

pub fn adapter_from_env(env: &AxEnv) -> AxRuntimeResult<Box<dyn AxDatabaseAdapter>> {
    let url = env.database_url().map(str::to_owned);
    match env.database_driver().trim().to_ascii_lowercase().as_str() {
        "" | "postgres" | "postgresql" => Ok(Box::new(PostgresAdapter { url })),
        "mysql" => Ok(Box::new(MySqlAdapter { url })),
        "sqlite" => Ok(Box::new(SqliteAdapter { url })),
        "memory" | "inmemory" | "in-memory" => Ok(Box::new(MemoryAdapter)),
        other => Err(AxRuntimeError::message(format!(
            "unsupported database driver `{other}`"
        ))),
    }
}

pub fn runtime_from_env(env: AxEnv) -> AxRuntimeResult<AxDatabaseRuntime<Box<dyn AxDatabaseAdapter>>> {
    let adapter = adapter_from_env(&env)?;
    Ok(AxDatabaseRuntime::new(env, adapter))
}

pub fn ok_payload() -> Value {
    serde_json::json!({ "ok": true })
}

fn adapter_payload(driver: &str, url: &Option<String>, collection: String, details: Value) -> Value {
    serde_json::json!({
        "driver": driver,
        "url": url,
        "collection": collection,
        "details": details,
    })
}

fn mutation_payload(
    driver: &str,
    url: &Option<String>,
    action: &str,
    collection: &str,
    fields: &BTreeMap<String, Value>,
) -> Value {
    serde_json::json!({
        "driver": driver,
        "url": url,
        "action": action,
        "collection": collection,
        "fields": fields,
    })
}

fn query_filter_payload(filter: &AxQueryFilterRequest) -> Value {
    serde_json::json!({
        "field": filter.field,
        "op": query_filter_op_name(filter.op),
        "value": filter.value,
    })
}

fn query_order_payload(order: &AxQueryOrderRequest) -> Value {
    serde_json::json!({
        "field": order.field,
        "direction": query_order_direction_name(order.direction),
    })
}

fn query_filter_op_name(op: AxQueryFilterOp) -> &'static str {
    match op {
        AxQueryFilterOp::Eq => "eq",
    }
}

fn query_order_direction_name(direction: AxQueryOrderDirection) -> &'static str {
    match direction {
        AxQueryOrderDirection::Asc => "asc",
        AxQueryOrderDirection::Desc => "desc",
    }
}

pub struct Db;

impl Db {
    #[allow(non_snake_case)]
    pub fn Stream(collection: String) -> Value {
        serde_json::json!({
            "source": "Db.Stream",
            "collection": collection,
        })
    }
}

pub struct Query;

impl Query {
    #[allow(non_snake_case)]
    pub fn PublishedPosts() -> Value {
        serde_json::json!({
            "query": "PublishedPosts",
        })
    }
}

pub mod backend_prelude {
    pub use super::ok_payload;
    pub use super::adapter_from_env;
    pub use super::AxBackendRuntime;
    pub use super::AxDatabaseAdapter;
    pub use super::AxDatabaseRuntime;
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
    pub use super::AxRuntimeEnvAccess;
    pub use super::AxRuntimeError;
    pub use super::AxRuntimeResult;
    pub use super::AxSendRequest;
    pub use super::AxUpdateRequest;
    pub use super::MemoryAdapter;
    pub use super::MySqlAdapter;
    pub use super::PostgresAdapter;
    pub use super::runtime_from_env;
    pub use super::SqliteAdapter;
    pub use super::Db;
    pub use super::Query;
}
