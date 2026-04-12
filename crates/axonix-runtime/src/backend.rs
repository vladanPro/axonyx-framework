use std::collections::BTreeMap;

use axonix_core::ax_backend_lowering_prelude::{
    AxAssignmentPlan, AxQueryFilterOpPlan, AxQueryFilterPlan, AxQueryOrderDirectionPlan,
    AxQueryOrderPlan, AxQueryPlan, AxQuerySourcePlan, AxRustExpr,
};
use axonix_core::ax_sql_prelude::{
    compile_insert_plan_to_sql, compile_query_plan_to_sql, compile_update_plan_to_sql,
    AxSqlDialect,
};
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
    pub filters: Vec<AxQueryFilterRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxSendRequest {
    pub target: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxDirectSqlPlan {
    pub dialect: String,
    pub sql: String,
    pub params: Vec<Value>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxApiRequestPlan {
    pub dialect: String,
    pub base_url: String,
    pub token: String,
    pub action: String,
    pub resource: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxDatabaseDriver {
    Postgres,
    MySql,
    Sqlite,
    Memory,
}

impl AxDatabaseDriver {
    pub fn parse(input: &str) -> AxRuntimeResult<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "" | "postgres" | "postgresql" => Ok(Self::Postgres),
            "mysql" => Ok(Self::MySql),
            "sqlite" => Ok(Self::Sqlite),
            "memory" | "inmemory" | "in-memory" => Ok(Self::Memory),
            other => Err(AxRuntimeError::message(format!(
                "unsupported database driver `{other}`"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
            Self::Memory => "memory",
        }
    }

    pub fn sql_dialect(&self) -> Option<AxSqlDialect> {
        match self {
            Self::Postgres => Some(AxSqlDialect::Postgres),
            Self::MySql => Some(AxSqlDialect::MySql),
            Self::Sqlite => Some(AxSqlDialect::Sqlite),
            Self::Memory => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxDataTransport {
    Direct,
    Api,
}

impl AxDataTransport {
    pub fn parse(input: &str) -> AxRuntimeResult<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "" | "direct" => Ok(Self::Direct),
            "api" => Ok(Self::Api),
            other => Err(AxRuntimeError::message(format!(
                "unsupported data transport `{other}`"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Api => "api",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxDatabaseConfig {
    pub driver: AxDatabaseDriver,
    pub transport: AxDataTransport,
    pub url: Option<String>,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
}

impl AxDatabaseConfig {
    pub fn sql_dialect(&self) -> Option<AxSqlDialect> {
        self.driver.sql_dialect()
    }

    pub fn validate(&self) -> AxRuntimeResult<()> {
        match self.transport {
            AxDataTransport::Direct => {
                if matches!(self.driver, AxDatabaseDriver::Memory) {
                    return Ok(());
                }

                if self.url.is_none() {
                    return Err(AxRuntimeError::message(
                        "missing AX_SECRET_DB_URL for direct data transport",
                    ));
                }
            }
            AxDataTransport::Api => {
                if self.api_url.is_none() {
                    return Err(AxRuntimeError::message(
                        "missing AX_PUBLIC_DATA_API_URL for api data transport",
                    ));
                }

                if self.api_key.is_none() {
                    return Err(AxRuntimeError::message(
                        "missing AX_SECRET_DATA_API_KEY for api data transport",
                    ));
                }
            }
        }

        Ok(())
    }
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

    pub fn database_driver(&self) -> AxRuntimeResult<AxDatabaseDriver> {
        match self.secret.get("db_dialect").or_else(|| self.secret.get("db_driver")) {
            Some(driver) => AxDatabaseDriver::parse(driver),
            None => Ok(AxDatabaseDriver::Postgres),
        }
    }

    pub fn data_transport(&self) -> AxRuntimeResult<AxDataTransport> {
        match self.secret.get("db_transport") {
            Some(transport) => AxDataTransport::parse(transport),
            None => Ok(AxDataTransport::Direct),
        }
    }

    pub fn database_config(&self) -> AxRuntimeResult<AxDatabaseConfig> {
        Ok(AxDatabaseConfig {
            driver: self.database_driver()?,
            transport: self.data_transport()?,
            url: self.secret.get("db_url").cloned(),
            api_url: self
                .public
                .get("data_api_url")
                .cloned()
                .or_else(|| self.public.get("supabase_url").cloned()),
            api_key: self
                .secret
                .get("data_api_key")
                .cloned()
                .or_else(|| self.secret.get("supabase_service_role_key").cloned()),
        })
    }

    pub fn sql_dialect(&self) -> AxRuntimeResult<Option<AxSqlDialect>> {
        Ok(self.database_driver()?.sql_dialect())
    }
}

fn normalize_env_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

pub trait AxRuntimeEnvAccess {
    fn env(&self) -> &AxEnv;
}

pub trait AxDatabaseAdapter {
    fn driver(&self) -> AxDatabaseDriver;
    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value>;
    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value>;
    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value>;
}

impl<T> AxDatabaseAdapter for Box<T>
where
    T: AxDatabaseAdapter + ?Sized,
{
    fn driver(&self) -> AxDatabaseDriver {
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
    pub transport: AxDataTransport,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MySqlAdapter {
    pub url: Option<String>,
    pub transport: AxDataTransport,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteAdapter {
    pub url: Option<String>,
    pub transport: AxDataTransport,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryAdapter;

impl AxDatabaseAdapter for PostgresAdapter {
    fn driver(&self) -> AxDatabaseDriver {
        AxDatabaseDriver::Postgres
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        dispatch_load(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        dispatch_insert(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        dispatch_update(self.driver(), self.transport, &self.url, &self.api_url, request)
    }
}

impl AxDatabaseAdapter for MySqlAdapter {
    fn driver(&self) -> AxDatabaseDriver {
        AxDatabaseDriver::MySql
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        dispatch_load(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        dispatch_insert(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        dispatch_update(self.driver(), self.transport, &self.url, &self.api_url, request)
    }
}

impl AxDatabaseAdapter for SqliteAdapter {
    fn driver(&self) -> AxDatabaseDriver {
        AxDatabaseDriver::Sqlite
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        dispatch_load(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        dispatch_insert(self.driver(), self.transport, &self.url, &self.api_url, request)
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        dispatch_update(self.driver(), self.transport, &self.url, &self.api_url, request)
    }
}

impl AxDatabaseAdapter for MemoryAdapter {
    fn driver(&self) -> AxDatabaseDriver {
        AxDatabaseDriver::Memory
    }

    fn load(&self, request: &AxQueryRequest) -> AxRuntimeResult<Value> {
        Ok(adapter_payload(self.driver(), AxDataTransport::Direct, &None, &None, request.collection.clone(), json!({
            "filters": request.filters,
            "orders": request.orders,
            "limit": request.limit,
            "offset": request.offset,
        })))
    }

    fn insert(&self, request: &AxInsertRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), AxDataTransport::Direct, &None, &None, "insert", &request.collection, &request.fields))
    }

    fn update(&self, request: &AxUpdateRequest) -> AxRuntimeResult<Value> {
        Ok(mutation_payload(self.driver(), AxDataTransport::Direct, &None, &None, "update", &request.collection, &request.fields))
    }
}

pub fn adapter_from_config(config: &AxDatabaseConfig) -> Box<dyn AxDatabaseAdapter> {
    match config.driver {
        AxDatabaseDriver::Postgres => Box::new(PostgresAdapter {
            url: config.url.clone(),
            transport: config.transport,
            api_url: config.api_url.clone(),
        }),
        AxDatabaseDriver::MySql => Box::new(MySqlAdapter {
            url: config.url.clone(),
            transport: config.transport,
            api_url: config.api_url.clone(),
        }),
        AxDatabaseDriver::Sqlite => Box::new(SqliteAdapter {
            url: config.url.clone(),
            transport: config.transport,
            api_url: config.api_url.clone(),
        }),
        AxDatabaseDriver::Memory => Box::new(MemoryAdapter),
    }
}

pub fn runtime_from_env(env: AxEnv) -> AxRuntimeResult<AxDatabaseRuntime<Box<dyn AxDatabaseAdapter>>> {
    let config = env.database_config()?;
    config.validate()?;
    let adapter = adapter_from_config(&config);
    Ok(AxDatabaseRuntime::new(env, adapter))
}

pub fn ok_payload() -> Value {
    json!({ "ok": true })
}

fn adapter_payload(
    driver: AxDatabaseDriver,
    transport: AxDataTransport,
    url: &Option<String>,
    api_url: &Option<String>,
    collection: String,
    details: Value,
) -> Value {
    json!({
        "driver": driver.as_str(),
        "transport": transport.as_str(),
        "url": url,
        "api_url": api_url,
        "collection": collection,
        "details": details,
    })
}

fn mutation_payload(
    driver: AxDatabaseDriver,
    transport: AxDataTransport,
    url: &Option<String>,
    api_url: &Option<String>,
    action: &str,
    collection: &str,
    fields: &BTreeMap<String, Value>,
) -> Value {
    json!({
        "driver": driver.as_str(),
        "transport": transport.as_str(),
        "url": url,
        "api_url": api_url,
        "action": action,
        "collection": collection,
        "fields": fields,
    })
}

fn dispatch_load(
    driver: AxDatabaseDriver,
    transport: AxDataTransport,
    url: &Option<String>,
    api_url: &Option<String>,
    request: &AxQueryRequest,
) -> AxRuntimeResult<Value> {
    match transport {
        AxDataTransport::Direct => direct_load_plan(driver, url, request),
        AxDataTransport::Api => api_load_plan(driver, api_url, request),
    }
}

fn dispatch_insert(
    driver: AxDatabaseDriver,
    transport: AxDataTransport,
    url: &Option<String>,
    api_url: &Option<String>,
    request: &AxInsertRequest,
) -> AxRuntimeResult<Value> {
    match transport {
        AxDataTransport::Direct => direct_insert_plan(driver, url, request),
        AxDataTransport::Api => {
            api_mutation_plan(driver, api_url, "insert", &request.collection, &request.fields, &[])
        }
    }
}

fn dispatch_update(
    driver: AxDatabaseDriver,
    transport: AxDataTransport,
    url: &Option<String>,
    api_url: &Option<String>,
    request: &AxUpdateRequest,
) -> AxRuntimeResult<Value> {
    match transport {
        AxDataTransport::Direct => direct_update_plan(driver, url, request),
        AxDataTransport::Api => api_mutation_plan(
            driver,
            api_url,
            "update",
            &request.collection,
            &request.fields,
            &request.filters,
        ),
    }
}

fn direct_load_plan(
    driver: AxDatabaseDriver,
    url: &Option<String>,
    request: &AxQueryRequest,
) -> AxRuntimeResult<Value> {
    let Some(dialect) = driver.sql_dialect() else {
        return Ok(adapter_payload(driver, AxDataTransport::Direct, url, &None, request.collection.clone(), json!({
            "filters": request.filters,
            "orders": request.orders,
            "limit": request.limit,
            "offset": request.offset,
        })));
    };

    let plan = compile_query_plan_to_sql(&query_request_to_plan(request), dialect)
        .map_err(|error| AxRuntimeError::message(error.to_string()))?;
    let execution = AxDirectSqlPlan {
        dialect: dialect.name().to_string(),
        sql: plan.sql,
        params: request.filters.iter().map(|filter| filter.value.clone()).collect(),
        url: url.clone(),
    };

    Ok(json!({
        "driver": driver.as_str(),
        "transport": "direct",
        "execution": execution,
    }))
}

fn direct_insert_plan(
    driver: AxDatabaseDriver,
    url: &Option<String>,
    request: &AxInsertRequest,
) -> AxRuntimeResult<Value> {
    let Some(dialect) = driver.sql_dialect() else {
        return Ok(mutation_payload(driver, AxDataTransport::Direct, url, &None, "insert", &request.collection, &request.fields));
    };

    let fields = fields_to_assignment_plans(&request.fields);
    let plan = compile_insert_plan_to_sql(&request.collection, &fields, dialect)
        .map_err(|error| AxRuntimeError::message(error.to_string()))?;
    let execution = AxDirectSqlPlan {
        dialect: dialect.name().to_string(),
        sql: plan.sql,
        params: request.fields.values().cloned().collect(),
        url: url.clone(),
    };

    Ok(json!({
        "driver": driver.as_str(),
        "transport": "direct",
        "action": "insert",
        "execution": execution,
    }))
}

fn direct_update_plan(
    driver: AxDatabaseDriver,
    url: &Option<String>,
    request: &AxUpdateRequest,
) -> AxRuntimeResult<Value> {
    let Some(dialect) = driver.sql_dialect() else {
        return Ok(mutation_payload(driver, AxDataTransport::Direct, url, &None, "update", &request.collection, &request.fields));
    };

    let fields = fields_to_assignment_plans(&request.fields);
    let filters = query_filters_to_plan(&request.filters);
    let plan = compile_update_plan_to_sql(&request.collection, &fields, &filters, dialect)
        .map_err(|error| AxRuntimeError::message(error.to_string()))?;
    let execution = AxDirectSqlPlan {
        dialect: dialect.name().to_string(),
        sql: plan.sql,
        params: request
            .fields
            .values()
            .cloned()
            .chain(request.filters.iter().map(|filter| filter.value.clone()))
            .collect(),
        url: url.clone(),
    };

    Ok(json!({
        "driver": driver.as_str(),
        "transport": "direct",
        "action": "update",
        "execution": execution,
    }))
}

fn api_load_plan(
    driver: AxDatabaseDriver,
    api_url: &Option<String>,
    request: &AxQueryRequest,
) -> AxRuntimeResult<Value> {
    let plan = AxApiRequestPlan {
        dialect: driver
            .sql_dialect()
            .map(|dialect| dialect.name().to_string())
            .unwrap_or_else(|| driver.as_str().to_string()),
        base_url: api_url.clone().unwrap_or_default(),
        token: "<redacted-by-runtime-config>".to_string(),
        action: "load".to_string(),
        resource: request.collection.clone(),
        payload: json!({
            "filters": request.filters,
            "orders": request.orders,
            "limit": request.limit,
            "offset": request.offset,
        }),
    };

    Ok(json!({
        "driver": driver.as_str(),
        "transport": "api",
        "request": plan,
    }))
}

fn api_mutation_plan(
    driver: AxDatabaseDriver,
    api_url: &Option<String>,
    action: &str,
    collection: &str,
    fields: &BTreeMap<String, Value>,
    filters: &[AxQueryFilterRequest],
) -> AxRuntimeResult<Value> {
    let plan = AxApiRequestPlan {
        dialect: driver
            .sql_dialect()
            .map(|dialect| dialect.name().to_string())
            .unwrap_or_else(|| driver.as_str().to_string()),
        base_url: api_url.clone().unwrap_or_default(),
        token: "<redacted-by-runtime-config>".to_string(),
        action: action.to_string(),
        resource: collection.to_string(),
        payload: json!({
            "fields": fields,
            "filters": request_filters_payload(filters),
        }),
    };

    Ok(json!({
        "driver": driver.as_str(),
        "transport": "api",
        "request": plan,
    }))
}

fn query_request_to_plan(request: &AxQueryRequest) -> AxQueryPlan {
    AxQueryPlan {
        source: AxQuerySourcePlan::Stream {
            collection: request.collection.clone(),
        },
        filters: query_filters_to_plan(&request.filters),
        orders: request
            .orders
            .iter()
            .map(|order| AxQueryOrderPlan {
                field: order.field.clone(),
                direction: match order.direction {
                    AxQueryOrderDirection::Asc => AxQueryOrderDirectionPlan::Asc,
                    AxQueryOrderDirection::Desc => AxQueryOrderDirectionPlan::Desc,
                },
            })
            .collect(),
        limit: request.limit,
        offset: request.offset,
    }
}

fn query_filters_to_plan(filters: &[AxQueryFilterRequest]) -> Vec<AxQueryFilterPlan> {
    filters
        .iter()
        .map(|filter| AxQueryFilterPlan {
            field: filter.field.clone(),
            op: match filter.op {
                AxQueryFilterOp::Eq => AxQueryFilterOpPlan::Eq,
            },
            value: json_value_to_expr(&filter.value),
        })
        .collect()
}

fn fields_to_assignment_plans(fields: &BTreeMap<String, Value>) -> Vec<AxAssignmentPlan> {
    fields
        .iter()
        .map(|(name, value)| AxAssignmentPlan {
            name: name.clone(),
            value: json_value_to_expr(value),
        })
        .collect()
}

fn json_value_to_expr(value: &Value) -> AxRustExpr {
    AxRustExpr::new(value.to_string())
}

fn request_filters_payload(filters: &[AxQueryFilterRequest]) -> Value {
    json!(filters)
}

pub mod prelude {
    pub use super::ok_payload;
    pub use super::adapter_from_config;
    pub use super::AxBackendRuntime;
    pub use super::AxDatabaseAdapter;
    pub use super::AxDatabaseConfig;
    pub use super::AxDatabaseDriver;
    pub use super::AxDatabaseRuntime;
    pub use super::AxDataTransport;
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
    pub use super::MemoryAdapter;
    pub use super::MySqlAdapter;
    pub use super::PostgresAdapter;
    pub use super::runtime_from_env;
    pub use super::SqliteAdapter;
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

    #[test]
    fn env_can_resolve_database_config_for_mysql() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "mysql")
            .with_secret("db_url", "mysql://root:root@localhost:3306/axonix");

        let config = env.database_config().expect("config should resolve");

        assert_eq!(
            config,
            AxDatabaseConfig {
                driver: AxDatabaseDriver::MySql,
                transport: AxDataTransport::Direct,
                url: Some("mysql://root:root@localhost:3306/axonix".to_string()),
                api_url: None,
                api_key: None,
            }
        );
    }

    #[test]
    fn runtime_from_env_can_select_mysql_adapter() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "mysql")
            .with_secret("db_url", "mysql://root:root@localhost:3306/axonix");
        let runtime = runtime_from_env(env).expect("runtime should initialize");

        let value = runtime
            .load(&AxQueryRequest {
                collection: "posts".to_string(),
                filters: Vec::new(),
                orders: Vec::new(),
                limit: Some(10),
                offset: None,
            })
            .expect("query should execute");

        assert_eq!(value["driver"], "mysql");
        assert_eq!(value["transport"], "direct");
        assert_eq!(value["execution"]["dialect"], "mysql");
        assert_eq!(value["execution"]["params"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn direct_transport_emits_sql_execution_plan() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "postgres")
            .with_secret("db_url", "postgres://local/axonix");
        let runtime = runtime_from_env(env).expect("runtime should initialize");

        let value = runtime
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
                limit: Some(12),
                offset: None,
            })
            .expect("query should execute");

        assert_eq!(value["transport"], "direct");
        assert_eq!(value["execution"]["dialect"], "postgres");
        assert_eq!(
            value["execution"]["sql"],
            r#"select * from "posts" where "status" = $1 order by "created_at" desc limit 12"#
        );
        assert_eq!(value["execution"]["params"][0], json!("published"));
    }

    #[test]
    fn api_transport_emits_request_plan() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "postgres")
            .with_secret("db_transport", "api")
            .with_secret("data_api_key", "secret-token")
            .with_public("data_api_url", "https://data.example.com");
        let runtime = runtime_from_env(env).expect("runtime should initialize");

        let value = runtime
            .insert(&AxInsertRequest {
                collection: "posts".to_string(),
                fields: BTreeMap::from([("title".to_string(), json!("Hello"))]),
            })
            .expect("insert should execute");

        assert_eq!(value["transport"], "api");
        assert_eq!(value["request"]["base_url"], "https://data.example.com");
        assert_eq!(value["request"]["action"], "insert");
        assert_eq!(value["request"]["resource"], "posts");
        assert_eq!(value["request"]["payload"]["fields"]["title"], json!("Hello"));
    }

    #[test]
    fn direct_update_emits_where_clause_when_filters_exist() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "postgres")
            .with_secret("db_url", "postgres://local/axonix");
        let runtime = runtime_from_env(env).expect("runtime should initialize");

        let value = runtime
            .update(&AxUpdateRequest {
                collection: "posts".to_string(),
                fields: BTreeMap::from([("title".to_string(), json!("Hello"))]),
                filters: vec![AxQueryFilterRequest {
                    field: "id".to_string(),
                    op: AxQueryFilterOp::Eq,
                    value: json!(7),
                }],
            })
            .expect("update should execute");

        assert_eq!(
            value["execution"]["sql"],
            r#"update "posts" set "title" = $1 where "id" = $2"#
        );
        assert_eq!(value["execution"]["params"][0], json!("Hello"));
        assert_eq!(value["execution"]["params"][1], json!(7));
    }

    #[test]
    fn runtime_defaults_to_postgres_when_driver_is_missing() {
        let env = AxEnv::new().with_secret("db_url", "postgres://local/axonix");
        let config = env.database_config().expect("config should resolve");

        assert_eq!(config.driver, AxDatabaseDriver::Postgres);
        assert_eq!(config.transport, AxDataTransport::Direct);
    }

    #[test]
    fn database_driver_maps_to_sql_dialect() {
        assert_eq!(
            AxDatabaseDriver::Postgres.sql_dialect(),
            Some(AxSqlDialect::Postgres)
        );
        assert_eq!(
            AxDatabaseDriver::MySql.sql_dialect(),
            Some(AxSqlDialect::MySql)
        );
        assert_eq!(
            AxDatabaseDriver::Sqlite.sql_dialect(),
            Some(AxSqlDialect::Sqlite)
        );
        assert_eq!(AxDatabaseDriver::Memory.sql_dialect(), None);
    }

    #[test]
    fn env_can_resolve_sql_dialect_from_driver() {
        let env = AxEnv::new().with_secret("db_driver", "sqlite");

        assert_eq!(
            env.sql_dialect().expect("sql dialect should resolve"),
            Some(AxSqlDialect::Sqlite)
        );
    }

    #[test]
    fn env_defaults_transport_to_direct() {
        let env = AxEnv::new().with_secret("db_url", "postgres://local/axonix");

        assert_eq!(
            env.data_transport().expect("transport should resolve"),
            AxDataTransport::Direct
        );
    }

    #[test]
    fn env_can_resolve_api_transport_config() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "postgres")
            .with_secret("db_transport", "api")
            .with_secret("data_api_key", "secret-token")
            .with_public("data_api_url", "https://data.example.com");

        let config = env.database_config().expect("config should resolve");

        assert_eq!(config.driver, AxDatabaseDriver::Postgres);
        assert_eq!(config.transport, AxDataTransport::Api);
        assert_eq!(config.api_url.as_deref(), Some("https://data.example.com"));
        assert_eq!(config.api_key.as_deref(), Some("secret-token"));
    }

    #[test]
    fn direct_transport_requires_db_url() {
        let config = AxDatabaseConfig {
            driver: AxDatabaseDriver::Postgres,
            transport: AxDataTransport::Direct,
            url: None,
            api_url: None,
            api_key: None,
        };

        let error = config.validate().expect_err("direct transport should require db url");
        assert_eq!(
            error,
            AxRuntimeError::message("missing AX_SECRET_DB_URL for direct data transport")
        );
    }

    #[test]
    fn api_transport_requires_api_fields() {
        let config = AxDatabaseConfig {
            driver: AxDatabaseDriver::Postgres,
            transport: AxDataTransport::Api,
            url: None,
            api_url: None,
            api_key: None,
        };

        let error = config.validate().expect_err("api transport should require api url");
        assert_eq!(
            error,
            AxRuntimeError::message("missing AX_PUBLIC_DATA_API_URL for api data transport")
        );
    }

    #[test]
    fn runtime_from_env_validates_api_transport_requirements() {
        let env = AxEnv::new()
            .with_secret("db_dialect", "postgres")
            .with_secret("db_transport", "api")
            .with_public("data_api_url", "https://data.example.com");

        let error = match runtime_from_env(env) {
            Ok(_) => panic!("missing api key should fail"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            AxRuntimeError::message("missing AX_SECRET_DATA_API_KEY for api data transport")
        );
    }
}
