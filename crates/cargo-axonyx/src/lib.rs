use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, OnceLock,
};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use axonyx_core::ax_ast_prelude::{AxExpr, AxImport};
use axonyx_core::ax_backend_ast_prelude::{
    AxBackendBlock, AxBackendDocument, AxBackendStmt, AxHookPhase,
};
use axonyx_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use axonyx_core::ax_backend_lowering_prelude::{
    lower_backend_document, AxBackendPlan, AxHandlerKind, AxReturnPlan, AxRustExpr, AxStepPlan,
    AxValuePlan,
};
use axonyx_core::ax_backend_parser_prelude::{parse_backend_ax, AxBackendParseError};
use axonyx_core::ax_lowering_prelude::AxValue;
use axonyx_core::ax_parser_auto_prelude::{parse_ax_auto, AxAutoParseError, AxConvertV2Error};
use axonyx_core::ax_parser_prelude::AxParseError;
use axonyx_core::ax_parser_v2_prelude::{parse_ax_v2, AxParseV2Error};
use axonyx_core::ax_semantics_v2_prelude::AxSemanticV2Error;
use axonyx_core::ax_types_prelude::{check_document_types, AxDataContext};
use axonyx_core::state_prelude::{build_state_manifest_with_scope_mapper, AxStateValue};
use axonyx_runtime::server_prelude::{
    axonyx_response_to_axum, AxHttpRequest, AxHttpResponse, AxServer, AxServerConfig, AxServerMode,
    AxSseEvent,
};
use axonyx_runtime::{
    execute_preview_action_sources, execute_preview_route_request_sources,
    preview_ax_route_with_request_context_and_imports, AxPreviewActionResult,
    AxPreviewHttpResponse, AxPreviewStatePatch, AxPreviewStore,
};
use clap::{Parser, Subcommand, ValueEnum};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
#[cfg(test)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DOCS_LAYOUT_AX: &str = include_str!("../templates/docs/app/docs/layout.ax.tpl");
const DOCS_HOME_AX: &str = include_str!("../templates/docs/app/docs/page.ax.tpl");
const DOCS_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/docs/getting-started/page.ax.tpl");
const DOCS_REFERENCE_AX: &str = include_str!("../templates/docs/app/docs/reference/page.ax.tpl");
const DOCS_EXAMPLES_AX: &str = include_str!("../templates/docs/app/docs/examples/page.ax.tpl");
const AXONYX_RUNTIME_VERSION: &str = "0.1.14";
const AXONYX_UI_VERSION: &str = "0.0.40";
const AXONYX_UI_USE_DIRECTIVE: &str = "use \"@axonyx/ui\"";
const AXONYX_UI_STYLESHEET_HREF: &str = "/_ax/pkg/axonyx-ui/index.css";
const AXONYX_UI_SCRIPT_HREF: &str = "/_ax/pkg/axonyx-ui/js/index.js";
const AXONYX_UI_PACKAGE_NAME: &str = "axonyx-ui";
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 2;
const DEFAULT_SHUTDOWN_GRACE_SECONDS: u64 = 5;
const DEFAULT_MAX_CONNECTIONS: usize = 1024;
const DEFAULT_COMPRESSION_ENABLED: bool = true;
const DEFAULT_SECURITY_HEADERS_ENABLED: bool = true;
const DEFAULT_REQUEST_LOGGING_ENABLED: bool = true;
const DEFAULT_LOG_FORMAT: &str = "text";
const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
static CARGO_PACKAGE_ROOT_CACHE: OnceLock<Mutex<std::collections::HashMap<String, PathBuf>>> =
    OnceLock::new();

#[derive(Debug, Parser)]
#[command(name = "ax")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Add(AddArgs),
    Actions(ActionsArgs),
    Api(ApiArgs),
    Build(BuildArgs),
    Check(CheckArgs),
    Content(ContentArgs),
    Dev(DevArgs),
    Doctor(DoctorArgs),
    Graph(GraphArgs),
    Melt(MeltArgs),
    Routes(RoutesArgs),
    Run(RunArgs),
    Schema(SchemaArgs),
    State(StateArgs),
    Stream(DevArgs),
    Test(TestArgs),
    Upgrade,
}

#[derive(Debug, Parser)]
struct ApiArgs {
    /// Output format for the API contract report.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Render API request contracts as .ax type declarations.
    #[arg(long)]
    schema: bool,

    /// Render API contracts as an OpenAPI-compatible JSON document.
    #[arg(long)]
    openapi: bool,

    /// Write the rendered API output to a file instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ActionsArgs {
    /// Output format for the action manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Show only actions for a single route path, for example /feedback.
    #[arg(long)]
    route: Option<String>,

    /// Show only an action with this name.
    #[arg(long)]
    name: Option<String>,

    /// Render action input contracts as .ax type declarations.
    #[arg(long)]
    schema: bool,
}

#[derive(Debug, Parser)]
struct AddArgs {
    #[arg(value_enum)]
    module: ModuleKind,
}

#[derive(Debug, Parser, Default)]
struct BuildArgs {
    /// Output directory for static HTML and public assets.
    #[arg(long, default_value = "dist")]
    out_dir: PathBuf,

    /// Remove the output directory before generating build artifacts.
    #[arg(long)]
    clean: bool,
}

#[derive(Debug, Parser)]
struct CheckArgs {
    /// Check a single .ax file instead of all app/routes/jobs sources.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Output format for diagnostics.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct ContentArgs {
    /// Output format for the content manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct DevArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind. `run start` falls back to the PORT environment variable.
    #[arg(long)]
    port: Option<u16>,

    /// HTTP transport implementation. Tokio is the default; std is kept as a fallback.
    #[arg(long, value_enum, default_value_t = ServerTransport::Tokio)]
    transport: ServerTransport,

    /// Use the production-server path. Kept for deploy scripts; Tokio is now the default transport.
    #[arg(long)]
    production_server: bool,
}

impl DevArgs {
    fn effective_transport(&self) -> ServerTransport {
        if self.production_server {
            ServerTransport::Tokio
        } else {
            self.transport
        }
    }
}

#[derive(Debug, Parser)]
struct DoctorArgs {
    /// Output format for health checks.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Include platform-specific deployment checks.
    #[arg(long, value_enum)]
    deploy: Option<DeployTarget>,

    /// Exit with a non-zero status when warnings are present.
    #[arg(long)]
    deny_warnings: bool,
}

#[derive(Debug, Parser)]
struct GraphArgs {
    /// Output format for the application graph.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct MeltArgs {
    /// Output format for the Melt project graph.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Only verify that the Melt graph can be collected without diagnostics.
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Parser)]
struct RoutesArgs {
    /// Output format for the route manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct RunArgs {
    #[command(subcommand)]
    command: RunCommands,
}

#[derive(Debug, Parser)]
struct SchemaArgs {
    #[command(subcommand)]
    command: SchemaCommands,
}

#[derive(Debug, Parser)]
struct StateArgs {
    /// Output format for the state manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct TestArgs {
    /// Aegis fast-test config file.
    #[arg(long, default_value = "aegis.toml")]
    config: PathBuf,

    /// Output format passed through to Aegis.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Stop at the first failing Aegis check.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    fail_fast: bool,

    #[command(subcommand)]
    command: Option<TestCommands>,
}

#[derive(Debug, Subcommand)]
enum RunCommands {
    Dev(DevArgs),
    Start(DevArgs),
}

#[derive(Debug, Subcommand)]
enum SchemaCommands {
    Pull(SchemaPullArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
enum TestCommands {
    Components,
    Routes,
    Browser,
}

#[derive(Debug, Parser)]
struct SchemaPullArgs {
    /// JSON file path, inline JSON, or local http:// endpoint to inspect.
    source: String,

    /// Root record name for generated .ax output.
    #[arg(long, default_value = "Item")]
    name: String,

    /// Output format for the inferred schema.
    #[arg(long, value_enum, default_value_t = SchemaFormat::Ax)]
    format: SchemaFormat,

    /// Write output to a file instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CheckFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SchemaFormat {
    Ax,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ServerTransport {
    Std,
    Tokio,
}

impl ServerTransport {
    fn label(self) -> &'static str {
        match self {
            Self::Std => "std",
            Self::Tokio => "tokio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleKind {
    Blockbit,
    Cms,
    Docs,
    Ui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DeployTarget {
    Render,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRoute {
    request_path: String,
    request_target: String,
    page_path: PathBuf,
    layout_paths: Vec<PathBuf>,
    loader_path: Option<PathBuf>,
    actions_path: Option<PathBuf>,
    params: std::collections::BTreeMap<String, String>,
}

struct StaticAsset {
    content_type: &'static str,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum DoctorSeverity {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DoctorCheck {
    code: &'static str,
    severity: DoctorSeverity,
    message: String,
    hint: Option<&'static str>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct DoctorSummary {
    ok: usize,
    warn: usize,
    error: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrerenderRoute {
    route: String,
    params: Vec<std::collections::BTreeMap<String, String>>,
}

struct DevServerState {
    root: PathBuf,
    preview_store: Mutex<AxPreviewStore>,
    runtime_config: AxServerRuntimeConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AxServerRuntimeConfig {
    max_body_bytes: usize,
    request_timeout: Duration,
    shutdown_grace: Duration,
    max_connections: usize,
    compression: bool,
    security_headers: bool,
    request_logging: bool,
    log_format: AxServerLogFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxServerLogFormat {
    Text,
    Json,
}

impl AxServerLogFormat {
    fn label(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}

#[derive(Clone)]
struct TokioConnectionTracker {
    active: Arc<AtomicUsize>,
    grace_period: Duration,
    max_connections: usize,
}

impl TokioConnectionTracker {
    fn new(grace_period: Duration, max_connections: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            grace_period,
            max_connections,
        }
    }

    fn try_track(&self) -> Option<TokioConnectionGuard> {
        let mut current = self.active.load(Ordering::SeqCst);
        loop {
            if current >= self.max_connections {
                return None;
            }

            match self.active.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    return Some(TokioConnectionGuard {
                        active: Arc::clone(&self.active),
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn active_count(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }
}

struct TokioConnectionGuard {
    active: Arc<AtomicUsize>,
}

impl Drop for TokioConnectionGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl AxServerRuntimeConfig {
    fn from_root(root: &Path) -> std::result::Result<Self, String> {
        Ok(Self {
            max_body_bytes: configured_max_request_body_bytes(root)?,
            request_timeout: configured_request_timeout_duration(root)?,
            shutdown_grace: configured_shutdown_grace_duration(root)?,
            max_connections: configured_max_connections(root)?,
            compression: configured_server_bool(root, "compression", DEFAULT_COMPRESSION_ENABLED)?,
            security_headers: configured_server_bool(
                root,
                "security_headers",
                DEFAULT_SECURITY_HEADERS_ENABLED,
            )?,
            request_logging: configured_server_bool(
                root,
                "request_logging",
                DEFAULT_REQUEST_LOGGING_ENABLED,
            )?,
            log_format: configured_server_log_format(root)?,
        })
    }
}

impl Default for AxServerRuntimeConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: MAX_REQUEST_BODY_BYTES,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS),
            shutdown_grace: Duration::from_secs(DEFAULT_SHUTDOWN_GRACE_SECONDS),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            compression: DEFAULT_COMPRESSION_ENABLED,
            security_headers: DEFAULT_SECURITY_HEADERS_ENABLED,
            request_logging: DEFAULT_REQUEST_LOGGING_ENABLED,
            log_format: AxServerLogFormat::Text,
        }
    }
}

struct StdNetAxServer {
    config: AxServerConfig,
    state: Arc<DevServerState>,
}

struct TokioAxServer {
    config: AxServerConfig,
    state: Arc<DevServerState>,
}

#[derive(Clone)]
struct AxumServerState {
    dev: Arc<DevServerState>,
    mode: AxServerMode,
    tracker: TokioConnectionTracker,
}

impl AxServer for StdNetAxServer {
    fn config(&self) -> &AxServerConfig {
        &self.config
    }

    fn serve(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bind = self.config.bind_addr();
        let listener = TcpListener::bind(&bind)
            .with_context(|| format!("failed to bind Axonyx server at {bind}"))?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Err(error) = handle_connection(stream, &self.state, self.config.mode) {
                        eprintln!(
                            "Axonyx {} server error: {error:#}",
                            self.config.mode.label()
                        );
                    }
                }
                Err(error) => {
                    eprintln!(
                        "Axonyx {} server connection error: {error}",
                        self.config.mode.label()
                    );
                }
            }
        }

        Ok(())
    }
}

impl AxServer for TokioAxServer {
    fn config(&self) -> &AxServerConfig {
        &self.config
    }

    fn serve(&self) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()
            .context("failed to build Tokio runtime")?;
        let config = self.config.clone();
        let state = Arc::clone(&self.state);

        runtime.block_on(async move { serve_axum_tokio(config, state).await })?;
        Ok(())
    }
}

#[derive(Debug)]
enum BackendBuildStatus {
    Generated {
        source_count: usize,
        output_path: PathBuf,
    },
    NoSources {
        output_path: PathBuf,
    },
}

#[derive(Debug)]
enum StaticBuildStatus {
    Generated {
        route_count: usize,
        prerendered_count: usize,
        skipped_dynamic_count: usize,
        content_collection_count: usize,
        melt_graph_written: bool,
        output_dir: PathBuf,
    },
    NoPages {
        skipped_dynamic_count: usize,
        content_collection_count: usize,
        melt_graph_written: bool,
        output_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize)]
struct CheckDiagnostic {
    file: String,
    line: usize,
    column: usize,
    severity: &'static str,
    code: &'static str,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct RouteManifestItem {
    kind: &'static str,
    route: String,
    method: Option<String>,
    returns: Option<String>,
    file: String,
    layouts: Vec<String>,
    loader: Option<String>,
    actions: Option<String>,
    params: Vec<String>,
    inputs: Vec<ActionInputReport>,
    hooks: Vec<RouteHookReport>,
}

#[derive(Debug, Clone, Serialize)]
struct RoutesReport {
    stream_pages: bool,
    routes: Vec<RouteManifestItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ApiReport {
    routes: Vec<ApiRouteReport>,
    schemas: Vec<ApiSchemaReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ApiRouteReport {
    method: String,
    route: String,
    returns: Option<String>,
    file: String,
    params: Vec<String>,
    inputs: Vec<ActionInputReport>,
    hooks: Vec<RouteHookReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RouteHookReport {
    phase: &'static str,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ApiSchemaReport {
    name: String,
    fields: Vec<ApiSchemaFieldReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ApiSchemaFieldReport {
    name: String,
    ty: String,
    optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActionReport {
    routes: Vec<ActionRouteReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActionRouteReport {
    route: String,
    file: String,
    actions: Vec<ActionItemReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActionItemReport {
    name: String,
    returns: Option<String>,
    inputs: Vec<ActionInputReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActionInputReport {
    name: String,
    ty: String,
    optional: bool,
    default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct StateReport {
    files: Vec<StateReportFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct StateReportFile {
    file: String,
    signals: Vec<StateReportSignal>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct StateReportSignal {
    name: String,
    key: String,
    scope: String,
    owner: String,
    ty: String,
    initial: AxStateValue,
}

#[derive(Debug, Clone, Serialize)]
struct MeltReport {
    app: MeltAppReport,
    layers: Vec<MeltLayerReport>,
    routes: RoutesReport,
    api: ApiReport,
    actions: ActionReport,
    state: StateReport,
    content: ContentManifest,
    diagnostics: Vec<CheckDiagnostic>,
    summary: MeltSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MeltAppReport {
    name: String,
    root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MeltLayerReport {
    name: &'static str,
    status: &'static str,
    detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct MeltSummary {
    page_routes: usize,
    api_routes: usize,
    action_routes: usize,
    actions: usize,
    state_signals: usize,
    content_collections: usize,
    content_entries: usize,
    diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentManifest {
    collections: Vec<ContentCollectionManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentCollectionManifest {
    name: String,
    path: String,
    extensions: Vec<String>,
    entries: Vec<ContentEntryManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentEntryManifest {
    path: String,
    slug: String,
    extension: String,
    content_type: String,
    bytes: u64,
    title: String,
    excerpt: String,
    word_count: usize,
    frontmatter: std::collections::BTreeMap<String, String>,
    body: String,
    html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredSchema {
    root_type: String,
    records: Vec<InferredRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredRecord {
    name: String,
    fields: Vec<InferredField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredField {
    name: String,
    ty: String,
    optional: bool,
}

pub fn main_entry() {
    if let Err(error) = run() {
        print_cli_error(&error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse_from(normalized_cli_args());

    match cli.command {
        Commands::Add(args) => add_module(args.module),
        Commands::Actions(args) => actions_command(args),
        Commands::Api(args) => api_command(args),
        Commands::Build(args) => build_command(args),
        Commands::Check(args) => check_command(args),
        Commands::Content(args) => content_command(args),
        Commands::Dev(args) => run_dev_server(args),
        Commands::Doctor(args) => doctor_command(args),
        Commands::Graph(args) => graph_command(args),
        Commands::Melt(args) => melt_command(args),
        Commands::Routes(args) => routes_command(args),
        Commands::Run(args) => run_command(args),
        Commands::Schema(args) => schema_command(args),
        Commands::State(args) => state_command(args),
        Commands::Stream(args) => run_stream_server(args),
        Commands::Test(args) => test_command(args),
        Commands::Upgrade => upgrade_command(),
    }
}

fn normalized_cli_args() -> Vec<OsString> {
    let mut args = std::env::args_os().collect::<Vec<_>>();

    if args
        .get(1)
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value, "ax" | "axonyx"))
    {
        args.remove(1);
    }

    args
}

fn actions_command(args: ActionsArgs) -> Result<()> {
    let root = app_root()?;
    let report = filter_action_report(collect_action_report(&root)?, &args);

    match args.format {
        CheckFormat::Text => {
            if args.schema {
                print_actions_schema_text(&report);
            } else {
                print_actions_text(&report);
            }
        }
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn filter_action_report(mut report: ActionReport, args: &ActionsArgs) -> ActionReport {
    if let Some(route_filter) = args.route.as_deref() {
        report.routes.retain(|route| route.route == route_filter);
    }

    if let Some(name_filter) = args.name.as_deref() {
        for route in &mut report.routes {
            route.actions.retain(|action| action.name == name_filter);
        }
        report.routes.retain(|route| !route.actions.is_empty());
    }

    report
}

fn build_command(args: BuildArgs) -> Result<()> {
    let root = app_root()?;
    ensure_no_melt_diagnostics_for(&root, "build")?;
    let status = compile_backend_from_app_root(&root)?;
    let static_status = build_static_site_from_app_root(&root, &args.out_dir, args.clean)?;
    print_backend_build_status(&status);
    print_static_build_status(&static_status);
    Ok(())
}

fn ensure_no_check_diagnostics_for(root: &Path, phase: &str) -> Result<()> {
    let diagnostics = check_app_sources(root)?;
    ensure_no_diagnostics_for_phase(&diagnostics, phase)
}

fn ensure_no_melt_diagnostics_for(root: &Path, phase: &str) -> Result<()> {
    let report = collect_melt_report(root)?;
    ensure_no_diagnostics_for_phase(&report.diagnostics, phase)
}

fn ensure_no_diagnostics_for_phase(diagnostics: &[CheckDiagnostic], phase: &str) -> Result<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }

    let mut message = format!("Axonyx diagnostics failed before {phase}:\n");
    for diagnostic in diagnostics {
        message.push_str("  ");
        message.push_str(&format_check_diagnostic(diagnostic));
        message.push('\n');
    }

    bail!("{}", message.trim_end());
}

fn check_command(args: CheckArgs) -> Result<()> {
    let diagnostics = if let Some(file) = args.file {
        check_ax_file(&file)?
    } else {
        let root = app_root()?;
        check_app_sources(&root)?
    };

    match args.format {
        CheckFormat::Text => print_check_text(&diagnostics),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&diagnostics)?);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_command(args: RunArgs) -> Result<()> {
    match args.command {
        RunCommands::Dev(args) => run_dev_server(args),
        RunCommands::Start(args) => run_start_server(args),
    }
}

fn upgrade_command() -> Result<()> {
    let root = app_root()?;
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.exists() {
        bail!("Cargo.toml is missing; run this command from an Axonyx app root");
    }

    let mut changes = Vec::new();
    if upgrade_cargo_dependency_version(&cargo_toml, "axonyx-runtime", AXONYX_RUNTIME_VERSION)? {
        changes.push(format!("axonyx-runtime = \"{AXONYX_RUNTIME_VERSION}\""));
    }

    let axonyx_source = fs::read_to_string(root.join("Axonyx.toml")).ok();
    let ui_enabled = axonyx_source
        .as_deref()
        .is_some_and(|source| source.contains("\"ui\"") || source.contains("'ui'"));
    if (ui_enabled || cargo_manifest_has_dependency_file(&cargo_toml, "axonyx-ui")?)
        && upgrade_cargo_dependency_version(&cargo_toml, "axonyx-ui", AXONYX_UI_VERSION)?
    {
        changes.push(format!("axonyx-ui = \"{AXONYX_UI_VERSION}\""));
    }

    if ui_enabled || cargo_manifest_has_dependency_file(&cargo_toml, "axonyx-ui")? {
        let layout_changed = ensure_ui_layout_setup(&root)?;
        if layout_changed {
            changes.push("app/layout.ax UI stylesheet/runtime setup".to_string());
        }
    }

    if changes.is_empty() {
        println!("Axonyx packages are already current or use path/git dependencies.");
    } else {
        println!("Updated Cargo.toml:");
        for change in changes {
            println!("  {change}");
        }
        println!("Next: cargo update");
    }

    Ok(())
}

fn doctor_command(args: DoctorArgs) -> Result<()> {
    let root = app_root()?;
    let checks = doctor_checks(&root, args.deploy);

    match args.format {
        CheckFormat::Text => print_doctor_text(&checks),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&checks)?);
        }
    }

    if doctor_should_fail(&checks, args.deny_warnings) {
        std::process::exit(1);
    }

    Ok(())
}

fn doctor_checks(root: &Path, deploy: Option<DeployTarget>) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    checks.push(doctor_file_check(
        root.join("Axonyx.toml").exists(),
        "axonyx-config",
        "Axonyx.toml found.",
        "Axonyx.toml is missing.",
        Some("Run this command from an Axonyx app root or create an app with create-axonyx."),
    ));
    checks.push(doctor_file_check(
        root.join("Cargo.toml").exists(),
        "cargo-manifest",
        "Cargo.toml found.",
        "Cargo.toml is missing.",
        Some("Axonyx apps need a Cargo manifest for runtime and package dependencies."),
    ));
    checks.push(doctor_file_check(
        root.join("app").exists(),
        "app-directory",
        "app/ directory found.",
        "app/ directory is missing.",
        Some("Create app/page.ax or scaffold a template with create-axonyx."),
    ));

    let cargo_source = fs::read_to_string(root.join("Cargo.toml")).ok();
    checks.push(match cargo_source.as_deref() {
        Some(source) if cargo_manifest_has_dependency(source, "axonyx-runtime") => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Ok,
            message: "axonyx-runtime dependency found.".to_string(),
            hint: None,
        },
        Some(_) => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Error,
            message: "axonyx-runtime dependency is missing.".to_string(),
            hint: Some("Add axonyx-runtime to Cargo.toml or recreate the app with create-axonyx."),
        },
        None => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect runtime dependency because Cargo.toml is missing."
                .to_string(),
            hint: None,
        },
    });
    if let Some(source) = cargo_source.as_deref() {
        checks.push(doctor_dependency_version_check(
            source,
            "axonyx-runtime",
            AXONYX_RUNTIME_VERSION,
            "cargo update -p axonyx-runtime",
        ));
    }
    checks.push(doctor_server_streaming_check(root));
    checks.push(doctor_server_body_limit_check(root));
    checks.push(doctor_server_request_timeout_check(root));
    checks.push(doctor_server_shutdown_grace_check(root));
    checks.push(doctor_server_max_connections_check(root));
    checks.push(doctor_server_compression_check(root));
    checks.push(doctor_server_security_headers_check(root));
    checks.push(doctor_server_request_logging_check(root));
    checks.push(doctor_aegis_config_check(root));

    let axonyx_source = fs::read_to_string(root.join("Axonyx.toml")).ok();
    let ui_enabled = axonyx_source
        .as_deref()
        .is_some_and(|source| source.contains("\"ui\"") || source.contains("'ui'"));
    if ui_enabled || root.join("vendor/axonyx-ui").exists() {
        checks.extend(doctor_ui_checks(root, cargo_source.as_deref()));
    }

    checks.push(doctor_state_manifest_check(root));
    checks.push(doctor_ax_sources_check(root));
    checks.push(doctor_melt_graph_check(root));
    if let Some(target) = deploy {
        checks.extend(doctor_deploy_checks(root, target));
    }

    checks
}

fn doctor_server_body_limit_check(root: &Path) -> DoctorCheck {
    match configured_max_request_body_bytes(root) {
        Ok(limit) => DoctorCheck {
            code: "server-body-limit",
            severity: DoctorSeverity::Ok,
            message: format!("Request body limit is {}.", format_bytes(limit)),
            hint: None,
        },
        Err(message) => DoctorCheck {
            code: "server-body-limit",
            severity: DoctorSeverity::Error,
            message,
            hint: Some(
                "Set [server].max_body_bytes to a positive number, or a string such as \"1mb\".",
            ),
        },
    }
}

fn doctor_server_request_timeout_check(root: &Path) -> DoctorCheck {
    match configured_request_timeout_duration(root) {
        Ok(timeout) => DoctorCheck {
            code: "server-request-timeout",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Request read timeout resolves to {} second{}.",
                timeout.as_secs(),
                if timeout.as_secs() == 1 { "" } else { "s" }
            ),
            hint: Some(
                "Tune [server].request_timeout_seconds for slow clients or upload-heavy apps.",
            ),
        },
        Err(message) => DoctorCheck {
            code: "server-request-timeout",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].request_timeout_seconds to a positive integer."),
        },
    }
}

fn doctor_server_shutdown_grace_check(root: &Path) -> DoctorCheck {
    match configured_shutdown_grace_duration(root) {
        Ok(grace) => DoctorCheck {
            code: "server-shutdown-grace",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Shutdown grace period resolves to {} second{}.",
                grace.as_secs(),
                if grace.as_secs() == 1 { "" } else { "s" }
            ),
            hint: Some("Tune [server].shutdown_grace_seconds for hosted deploy restarts."),
        },
        Err(message) => DoctorCheck {
            code: "server-shutdown-grace",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].shutdown_grace_seconds to a positive integer."),
        },
    }
}

fn doctor_server_max_connections_check(root: &Path) -> DoctorCheck {
    match configured_max_connections(root) {
        Ok(limit) => DoctorCheck {
            code: "server-max-connections",
            severity: DoctorSeverity::Ok,
            message: format!("Tokio max connections resolves to {limit}."),
            hint: Some("Tune [server].max_connections for hosted capacity and abuse protection."),
        },
        Err(message) => DoctorCheck {
            code: "server-max-connections",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].max_connections to a positive integer."),
        },
    }
}

fn doctor_server_compression_check(root: &Path) -> DoctorCheck {
    match configured_server_bool(root, "compression", DEFAULT_COMPRESSION_ENABLED) {
        Ok(enabled) => DoctorCheck {
            code: "server-compression",
            severity: DoctorSeverity::Ok,
            message: format!(
                "HTTP compression is {}.",
                if enabled { "enabled" } else { "disabled" }
            ),
            hint: Some("Tune [server].compression for hosted response size/performance."),
        },
        Err(message) => DoctorCheck {
            code: "server-compression",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].compression to true or false."),
        },
    }
}

fn doctor_server_security_headers_check(root: &Path) -> DoctorCheck {
    match configured_server_bool(root, "security_headers", DEFAULT_SECURITY_HEADERS_ENABLED) {
        Ok(enabled) => DoctorCheck {
            code: "server-security-headers",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Baseline security headers are {}.",
                if enabled { "enabled" } else { "disabled" }
            ),
            hint: Some("Tune [server].security_headers before custom edge/security setups."),
        },
        Err(message) => DoctorCheck {
            code: "server-security-headers",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].security_headers to true or false."),
        },
    }
}

fn doctor_server_request_logging_check(root: &Path) -> DoctorCheck {
    match (
        configured_server_bool(root, "request_logging", DEFAULT_REQUEST_LOGGING_ENABLED),
        configured_server_log_format(root),
    ) {
        (Ok(enabled), Ok(format)) => DoctorCheck {
            code: "server-request-logging",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Request logging is {} using {} format.",
                if enabled { "enabled" } else { "disabled" },
                format.label()
            ),
            hint: Some("Axonyx writes request logs to stdout for Render, Docker, and systemd."),
        },
        (Err(message), _) | (_, Err(message)) => DoctorCheck {
            code: "server-request-logging",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].request_logging to true/false and [server].log_format to \"text\" or \"json\"."),
        },
    }
}

fn doctor_aegis_config_check(root: &Path) -> DoctorCheck {
    if root.join("aegis.toml").exists() {
        DoctorCheck {
            code: "aegis-config",
            severity: DoctorSeverity::Ok,
            message: "aegis.toml found; `cargo ax test` can run fast route QA.".to_string(),
            hint: None,
        }
    } else {
        DoctorCheck {
            code: "aegis-config",
            severity: DoctorSeverity::Warn,
            message: "aegis.toml is missing; `cargo ax test` has no route QA config.".to_string(),
            hint: Some("Run `aegis init` or recreate the starter with the latest create-axonyx."),
        }
    }
}

fn doctor_server_streaming_check(root: &Path) -> DoctorCheck {
    let enabled = axonyx_config_bool(root, "server", "stream_pages").unwrap_or(false);
    DoctorCheck {
        code: "server-stream-pages",
        severity: DoctorSeverity::Ok,
        message: if enabled {
            "Page route streaming is enabled via [server].stream_pages.".to_string()
        } else {
            "Page route streaming is disabled; use ?__ax_stream=1 or [server].stream_pages = true to test it.".to_string()
        },
        hint: None,
    }
}

fn doctor_state_manifest_check(root: &Path) -> DoctorCheck {
    match collect_state_report(root) {
        Ok(report) => {
            let signal_count = report
                .files
                .iter()
                .map(|file| file.signals.len())
                .sum::<usize>();
            DoctorCheck {
                code: "state-manifest",
                severity: DoctorSeverity::Ok,
                message: if signal_count == 0 {
                    "No app state declarations found.".to_string()
                } else {
                    format!(
                        "State manifest builds successfully for {signal_count} signal{}.",
                        if signal_count == 1 { "" } else { "s" }
                    )
                },
                hint: None,
            }
        }
        Err(error) => DoctorCheck {
            code: "state-manifest",
            severity: DoctorSeverity::Error,
            message: format!("State manifest failed: {error}"),
            hint: Some("Run `cargo ax state` to inspect state declarations and manifest output."),
        },
    }
}

fn doctor_melt_graph_check(root: &Path) -> DoctorCheck {
    match collect_melt_report(root) {
        Ok(report) if report.diagnostics.is_empty() => DoctorCheck {
            code: "melt-graph",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Melt graph collected: {} page route(s), {} API route(s), {} action(s), {} state signal(s), {} content entr{}.",
                report.summary.page_routes,
                report.summary.api_routes,
                report.summary.actions,
                report.summary.state_signals,
                report.summary.content_entries,
                if report.summary.content_entries == 1 { "y" } else { "ies" }
            ),
            hint: None,
        },
        Ok(report) => DoctorCheck {
            code: "melt-graph",
            severity: DoctorSeverity::Error,
            message: format!(
                "Melt graph collected with {} source diagnostic(s).",
                report.summary.diagnostics
            ),
            hint: Some("Run `cargo ax melt` or `cargo ax check` to inspect the project graph."),
        },
        Err(error) => DoctorCheck {
            code: "melt-graph",
            severity: DoctorSeverity::Error,
            message: format!("Melt graph failed: {error}"),
            hint: Some("Run `cargo ax melt` to inspect the project graph failure."),
        },
    }
}

fn doctor_deploy_checks(root: &Path, target: DeployTarget) -> Vec<DoctorCheck> {
    match target {
        DeployTarget::Render => doctor_render_deploy_checks(root),
    }
}

fn doctor_render_deploy_checks(root: &Path) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    checks.push(DoctorCheck {
        code: "deploy-render-service",
        severity: DoctorSeverity::Ok,
        message: "Render target expects a Web Service with Cargo build/start commands."
            .to_string(),
        hint: Some(
            "Build command: cargo ax build --clean; start command: cargo ax run start --host 0.0.0.0 --port $PORT",
        ),
    });

    checks.push(DoctorCheck {
        code: "deploy-render-port",
        severity: DoctorSeverity::Ok,
        message:
            "`cargo ax run start` is PORT-aware when --port is omitted or passed from the platform."
                .to_string(),
        hint: Some("Render start command should pass --port $PORT for explicit hosted binding."),
    });

    checks.push(DoctorCheck {
        code: "deploy-render-production-server",
        severity: DoctorSeverity::Ok,
        message: "Render deploy uses the Tokio production path by default.".to_string(),
        hint: Some(
            "Use `cargo ax run start --host 0.0.0.0 --port $PORT`. Add `--transport std` only as a fallback.",
        ),
    });

    checks.push(DoctorCheck {
        code: "deploy-render-health",
        severity: DoctorSeverity::Ok,
        message: "Render health checks can use the built-in Axonyx health probe.".to_string(),
        hint: Some("Health check path: /__axonyx/health"),
    });

    checks.push(match configured_max_request_body_bytes(root) {
        Ok(limit) => DoctorCheck {
            code: "deploy-render-body-limit",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Render deploy request body limit resolves to {}.",
                format_bytes(limit)
            ),
            hint: Some("Tune [server].max_body_bytes before enabling large uploads."),
        },
        Err(message) => DoctorCheck {
            code: "deploy-render-body-limit",
            severity: DoctorSeverity::Error,
            message,
            hint: Some("Set [server].max_body_bytes to a positive number before deploying."),
        },
    });

    checks.push(match collect_melt_report(root) {
        Ok(report) if report.diagnostics.is_empty() => DoctorCheck {
            code: "deploy-render-melt",
            severity: DoctorSeverity::Ok,
            message: format!(
                "Render deploy graph is clean: {} page route(s), {} API route(s), {} action(s).",
                report.summary.page_routes, report.summary.api_routes, report.summary.actions
            ),
            hint: Some("Run `cargo ax graph` to inspect the server/state route map."),
        },
        Ok(report) => DoctorCheck {
            code: "deploy-render-melt",
            severity: DoctorSeverity::Error,
            message: format!(
                "Render deploy graph has {} diagnostic(s).",
                report.summary.diagnostics
            ),
            hint: Some("Run `cargo ax check` before deploying."),
        },
        Err(_) => DoctorCheck {
            code: "deploy-render-melt",
            severity: DoctorSeverity::Error,
            message: "Render deploy graph could not be collected.".to_string(),
            hint: Some("Run `cargo ax melt` for the full graph error."),
        },
    });

    checks
}

fn doctor_file_check(
    condition: bool,
    code: &'static str,
    ok_message: &str,
    error_message: &str,
    hint: Option<&'static str>,
) -> DoctorCheck {
    if condition {
        DoctorCheck {
            code,
            severity: DoctorSeverity::Ok,
            message: ok_message.to_string(),
            hint: None,
        }
    } else {
        DoctorCheck {
            code,
            severity: DoctorSeverity::Error,
            message: error_message.to_string(),
            hint,
        }
    }
}

fn doctor_ui_checks(root: &Path, cargo_source: Option<&str>) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    let package_root = resolve_package_asset_root(root, "axonyx-ui");
    let uses_interactive_foundry = app_uses_interactive_foundry(root).unwrap_or(false);

    checks.push(match package_root.as_ref() {
        Some(root) => DoctorCheck {
            code: "ui-package",
            severity: DoctorSeverity::Ok,
            message: format!("axonyx-ui package resolved at '{}'.", root.display()),
            hint: None,
        },
        None => DoctorCheck {
            code: "ui-package",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI package could not be resolved.".to_string(),
            hint: Some("Run `cargo ax add ui` or add axonyx-ui to Cargo.toml."),
        },
    });

    checks.push(match cargo_source {
        Some(source) if cargo_manifest_has_dependency(source, "axonyx-ui") => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Ok,
            message: "axonyx-ui Cargo dependency found.".to_string(),
            hint: None,
        },
        Some(_) => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Warn,
            message: "UI module is present but axonyx-ui is not listed in Cargo.toml.".to_string(),
            hint: Some("Run `cargo ax add ui` to add the published axonyx-ui dependency."),
        },
        None => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect axonyx-ui dependency because Cargo.toml is missing."
                .to_string(),
            hint: None,
        },
    });
    if let Some(source) = cargo_source {
        checks.push(doctor_dependency_version_check(
            source,
            "axonyx-ui",
            AXONYX_UI_VERSION,
            "cargo update -p axonyx-ui",
        ));
    }

    checks.push(
        match package_root
            .as_deref()
            .and_then(|root| cargo_package_ax_root(root, "@axonyx/ui"))
        {
            Some(_) => DoctorCheck {
                code: "ui-package-metadata",
                severity: DoctorSeverity::Ok,
                message: "Axonyx UI package metadata found.".to_string(),
                hint: None,
            },
            None => DoctorCheck {
                code: "ui-package-metadata",
                severity: DoctorSeverity::Warn,
                message: "Axonyx UI package metadata was not found or did not match @axonyx/ui."
                    .to_string(),
                hint: Some("Update axonyx-ui or rerun `cargo ax add ui`."),
            },
        },
    );

    let layout_source = fs::read_to_string(root.join("app/layout.ax")).ok();
    let layout_uses_axonyx_ui = layout_source
        .as_deref()
        .is_some_and(|source| source_uses_package(source, "@axonyx/ui"));
    checks.push(match layout_source.as_deref() {
        Some(_) if layout_uses_axonyx_ui => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Ok,
            message: "Axonyx UI package use directive found; stylesheet will be injected."
                .to_string(),
            hint: None,
        },
        Some(source) if source.contains(AXONYX_UI_STYLESHEET_HREF) => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Ok,
            message: "Canonical Axonyx UI stylesheet link found.".to_string(),
            hint: None,
        },
        Some(source) if source.contains("/css/axonyx-ui/index.css") => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Legacy Axonyx UI stylesheet link found.".to_string(),
            hint: Some("Prefer /_ax/pkg/axonyx-ui/index.css for package-served CSS."),
        },
        Some(_) => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI stylesheet link is missing from app/layout.ax.".to_string(),
            hint: Some("Run `cargo ax add ui` or add /_ax/pkg/axonyx-ui/index.css to <Head>."),
        },
        None => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect UI stylesheet because app/layout.ax is missing."
                .to_string(),
            hint: None,
        },
    });

    checks.push(match layout_source.as_deref() {
        Some(_) if layout_uses_axonyx_ui => DoctorCheck {
            code: "ui-script",
            severity: DoctorSeverity::Ok,
            message: "Axonyx UI package use directive found; behavior runtime will be injected."
                .to_string(),
            hint: None,
        },
        Some(source) if source.contains(AXONYX_UI_SCRIPT_HREF) => DoctorCheck {
            code: "ui-script",
            severity: DoctorSeverity::Ok,
            message: "Canonical Axonyx UI behavior script found.".to_string(),
            hint: None,
        },
        Some(_) if uses_interactive_foundry => DoctorCheck {
            code: "ui-script",
            severity: DoctorSeverity::Warn,
            message: "Interactive Foundry components were detected, but the Axonyx UI behavior script is missing from app/layout.ax.".to_string(),
            hint: Some(
                "Run `cargo ax add ui` or add /_ax/pkg/axonyx-ui/js/index.js to <Head>.",
            ),
        },
        Some(_) => DoctorCheck {
            code: "ui-script",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI behavior script is missing from app/layout.ax.".to_string(),
            hint: Some("Run `cargo ax add ui` or add /_ax/pkg/axonyx-ui/js/index.js to <Head>."),
        },
        None => DoctorCheck {
            code: "ui-script",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect UI behavior script because app/layout.ax is missing."
                .to_string(),
            hint: None,
        },
    });

    checks.push(match load_package_asset(root, AXONYX_UI_STYLESHEET_HREF) {
        Ok(Some(_)) => DoctorCheck {
            code: "ui-package-css",
            severity: DoctorSeverity::Ok,
            message: "Axonyx UI package CSS can be served.".to_string(),
            hint: None,
        },
        Ok(None) => DoctorCheck {
            code: "ui-package-css",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI package CSS could not be found.".to_string(),
            hint: Some("Run `cargo ax add ui` or check that axonyx-ui exposes src/css/index.css."),
        },
        Err(error) => DoctorCheck {
            code: "ui-package-css",
            severity: DoctorSeverity::Warn,
            message: format!("Axonyx UI package CSS check failed: {error}"),
            hint: Some("Check the package asset path and Axonyx UI package metadata."),
        },
    });

    checks.push(match load_package_asset(root, AXONYX_UI_SCRIPT_HREF) {
        Ok(Some(_)) => DoctorCheck {
            code: "ui-package-js",
            severity: DoctorSeverity::Ok,
            message: "Axonyx UI package JavaScript can be served.".to_string(),
            hint: None,
        },
        Ok(None) => DoctorCheck {
            code: "ui-package-js",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI package JavaScript could not be found.".to_string(),
            hint: Some("Run `cargo ax add ui` or check that axonyx-ui exposes src/js/index.js."),
        },
        Err(error) => DoctorCheck {
            code: "ui-package-js",
            severity: DoctorSeverity::Warn,
            message: format!("Axonyx UI package JavaScript check failed: {error}"),
            hint: Some("Check the package asset path and Axonyx UI package metadata."),
        },
    });

    checks
}

fn app_uses_interactive_foundry(root: &Path) -> Result<bool> {
    let mut files = Vec::new();
    collect_ax_files(&root.join("app"), &mut files)?;

    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("failed to read .ax file '{}'", file.display()))?;
        if source_uses_interactive_foundry(&source) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn source_uses_interactive_foundry(source: &str) -> bool {
    const INTERACTIVE_COMPONENTS: &[&str] = &[
        "Accordion",
        "AccordionItem",
        "CodeBlock",
        "Command",
        "Dialog",
        "Drawer",
        "DropdownMenu",
        "Popover",
        "Tabs",
        "Tab",
        "ThemeSwitcher",
    ];

    INTERACTIVE_COMPONENTS.iter().any(|component| {
        source.contains(&format!("@axonyx/ui/foundry/{component}.ax"))
            || source.contains(&format!("<{component}"))
    })
}

fn doctor_ax_sources_check(root: &Path) -> DoctorCheck {
    match check_app_sources(root) {
        Ok(diagnostics) if diagnostics.is_empty() => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Ok,
            message: "Axonyx source diagnostics passed.".to_string(),
            hint: None,
        },
        Ok(diagnostics) => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Error,
            message: format!(
                "{} Axonyx source diagnostic{} found.",
                diagnostics.len(),
                if diagnostics.len() == 1 { "" } else { "s" }
            ),
            hint: Some("Run `cargo ax check` to see file-level diagnostics."),
        },
        Err(error) => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Error,
            message: format!("Axonyx source diagnostics failed: {error}"),
            hint: Some("Run `cargo ax check` for more details."),
        },
    }
}

fn cargo_manifest_has_dependency(source: &str, dependency_name: &str) -> bool {
    source
        .parse::<toml::Value>()
        .ok()
        .and_then(|value| {
            value
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .cloned()
        })
        .is_some_and(|dependencies| dependencies.contains_key(dependency_name))
}

fn cargo_manifest_has_dependency_file(cargo_toml: &Path, dependency_name: &str) -> Result<bool> {
    if !cargo_toml.exists() {
        return Ok(false);
    }

    let source = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read '{}'", cargo_toml.display()))?;
    Ok(cargo_manifest_has_dependency(&source, dependency_name))
}

fn doctor_dependency_version_check(
    cargo_source: &str,
    dependency_name: &'static str,
    expected_version: &'static str,
    update_command: &'static str,
) -> DoctorCheck {
    let code = if dependency_name == "axonyx-runtime" {
        "runtime-version"
    } else {
        "ui-version"
    };

    let Some(version) = cargo_manifest_dependency_version(cargo_source, dependency_name) else {
        return DoctorCheck {
            code,
            severity: DoctorSeverity::Ok,
            message: format!(
                "{dependency_name} uses a path/git dependency or has no pinned registry version."
            ),
            hint: None,
        };
    };

    if version == expected_version {
        return DoctorCheck {
            code,
            severity: DoctorSeverity::Ok,
            message: format!("{dependency_name} {version} is current."),
            hint: None,
        };
    }

    let severity = if is_version_older(&version, expected_version) {
        DoctorSeverity::Warn
    } else {
        DoctorSeverity::Ok
    };
    let message = if severity == DoctorSeverity::Warn {
        format!("{dependency_name} {version} is older than expected {expected_version}.")
    } else {
        format!("{dependency_name} {version} is newer than expected {expected_version}.")
    };
    let hint = if severity == DoctorSeverity::Warn {
        Some(update_command)
    } else {
        None
    };

    DoctorCheck {
        code,
        severity,
        message,
        hint,
    }
}

fn cargo_manifest_dependency_version(source: &str, dependency_name: &str) -> Option<String> {
    let value = source.parse::<toml::Value>().ok()?;
    let dependencies = value.get("dependencies")?.as_table()?;
    let dependency = dependencies.get(dependency_name)?;

    match dependency {
        toml::Value::String(version) => Some(version.clone()),
        toml::Value::Table(table) => table
            .get("version")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

fn is_version_older(found: &str, expected: &str) -> bool {
    parse_version_tuple(found) < parse_version_tuple(expected)
}

fn parse_version_tuple(version: &str) -> (u64, u64, u64) {
    let trimmed = version
        .trim()
        .trim_start_matches('^')
        .trim_start_matches('=')
        .trim();
    let mut parts = trimmed.split('.');
    let major = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or(0);
    (major, minor, patch)
}

fn print_doctor_text(checks: &[DoctorCheck]) {
    println!("Axonyx doctor");
    for check in checks {
        let label = match check.severity {
            DoctorSeverity::Ok => "ok",
            DoctorSeverity::Warn => "warn",
            DoctorSeverity::Error => "error",
        };
        println!("[{label}] {}: {}", check.code, check.message);
        if let Some(hint) = check.hint {
            println!("       hint: {hint}");
        }
    }

    println!();
    println!("Framework layers:");
    for line in doctor_framework_layer_status_lines(checks) {
        println!("  {line}");
    }

    let summary = doctor_summary(checks);
    println!();
    println!(
        "Summary: {} ok, {} warning{}, {} error{}",
        summary.ok,
        summary.warn,
        if summary.warn == 1 { "" } else { "s" },
        summary.error,
        if summary.error == 1 { "" } else { "s" }
    );
}

fn doctor_framework_layer_status_lines(checks: &[DoctorCheck]) -> Vec<String> {
    vec![
        doctor_layer_line(
            "Axonyx Pages",
            "ax-sources",
            checks,
            ".ax pages, layouts, route params, and API source diagnostics pass",
            ".ax page/route source diagnostics need attention",
            ".ax page/route source diagnostics could not be fully checked",
        ),
        doctor_layer_line(
            "Axonyx Server",
            "server-body-limit",
            checks,
            "request limits, streaming config, and hosted start checks are visible",
            "server config needs attention",
            "server config could not be fully checked",
        ),
        doctor_layer_line(
            "Axonyx State",
            "state-manifest",
            checks,
            "state manifest can be built",
            "state manifest needs attention",
            "state manifest is not fully configured",
        ),
        doctor_optional_layer_line(
            "Axonyx Foundry",
            "ui-package",
            checks,
            "Foundry UI/theme package resolves",
            "run `cargo ax add ui` when this app needs Foundry components",
        ),
        doctor_layer_line(
            "Axonyx Melt",
            "melt-graph",
            checks,
            "project graph can be collected across framework layers",
            "project graph needs attention before build/deploy",
            "project graph could not be fully checked",
        ),
    ]
}

fn doctor_layer_line(
    name: &str,
    check_code: &str,
    checks: &[DoctorCheck],
    ok: &str,
    error: &str,
    warn: &str,
) -> String {
    match doctor_check_severity(checks, check_code) {
        Some(DoctorSeverity::Ok) => format!("{name}: ready - {ok}."),
        Some(DoctorSeverity::Warn) => format!("{name}: optional - {warn}."),
        Some(DoctorSeverity::Error) => format!("{name}: attention - {error}."),
        None => format!("{name}: optional - not enabled for this app yet."),
    }
}

fn doctor_optional_layer_line(
    name: &str,
    check_code: &str,
    checks: &[DoctorCheck],
    ok: &str,
    optional: &str,
) -> String {
    match doctor_check_severity(checks, check_code) {
        Some(DoctorSeverity::Ok) => format!("{name}: ready - {ok}."),
        Some(DoctorSeverity::Error) => {
            format!("{name}: attention - package setup needs attention.")
        }
        _ => format!("{name}: optional - {optional}."),
    }
}

fn doctor_check_severity(checks: &[DoctorCheck], code: &str) -> Option<DoctorSeverity> {
    checks
        .iter()
        .find(|check| check.code == code)
        .map(|check| check.severity)
}

fn doctor_should_fail(checks: &[DoctorCheck], deny_warnings: bool) -> bool {
    checks
        .iter()
        .any(|check| check.severity == DoctorSeverity::Error)
        || (deny_warnings
            && checks
                .iter()
                .any(|check| check.severity == DoctorSeverity::Warn))
}

fn doctor_summary(checks: &[DoctorCheck]) -> DoctorSummary {
    checks
        .iter()
        .fold(DoctorSummary::default(), |mut summary, check| {
            match check.severity {
                DoctorSeverity::Ok => summary.ok += 1,
                DoctorSeverity::Warn => summary.warn += 1,
                DoctorSeverity::Error => summary.error += 1,
            }
            summary
        })
}

fn routes_command(args: RoutesArgs) -> Result<()> {
    let root = app_root()?;
    let report = routes_report(&root)?;

    match args.format {
        CheckFormat::Text => print_routes_text(&report),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn graph_command(args: GraphArgs) -> Result<()> {
    let root = app_root()?;
    let report = collect_melt_report(&root)?;

    match args.format {
        CheckFormat::Text => print_graph_text(&report),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    if !report.diagnostics.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn melt_command(args: MeltArgs) -> Result<()> {
    let root = app_root()?;
    let report = collect_melt_report(&root)?;

    if args.check {
        if report.diagnostics.is_empty() {
            println!(
                "Melt graph ok: {} page route(s), {} API route(s), {} action(s), {} state signal(s), {} content entr{}.",
                report.summary.page_routes,
                report.summary.api_routes,
                report.summary.actions,
                report.summary.state_signals,
                report.summary.content_entries,
                if report.summary.content_entries == 1 { "y" } else { "ies" }
            );
            return Ok(());
        }

        for diagnostic in &report.diagnostics {
            eprintln!("{}", format_check_diagnostic(diagnostic));
        }
        std::process::exit(1);
    }

    match args.format {
        CheckFormat::Text => print_melt_text(&report),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    if !report.diagnostics.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn collect_melt_report(root: &Path) -> Result<MeltReport> {
    let routes = routes_report(root)?;
    let api = collect_api_report(root)?;
    let actions = collect_action_report(root)?;
    let state = collect_state_report(root)?;
    let content = collect_content_manifest(root)?;
    let diagnostics = check_app_sources(root)?;
    let summary = melt_summary(&routes, &api, &actions, &state, &content, &diagnostics);
    let layers = melt_layer_reports(root, &summary);

    Ok(MeltReport {
        app: MeltAppReport {
            name: axonyx_config_string(root, "app", "name")
                .unwrap_or_else(|| "axonyx-app".to_string()),
            root: display_path(root),
        },
        layers,
        routes,
        api,
        actions,
        state,
        content,
        diagnostics,
        summary,
    })
}

fn melt_summary(
    routes: &RoutesReport,
    api: &ApiReport,
    actions: &ActionReport,
    state: &StateReport,
    content: &ContentManifest,
    diagnostics: &[CheckDiagnostic],
) -> MeltSummary {
    MeltSummary {
        page_routes: routes
            .routes
            .iter()
            .filter(|route| route.kind == "page")
            .count(),
        api_routes: api.routes.len(),
        action_routes: actions.routes.len(),
        actions: actions.routes.iter().map(|route| route.actions.len()).sum(),
        state_signals: state.files.iter().map(|file| file.signals.len()).sum(),
        content_collections: content.collections.len(),
        content_entries: content
            .collections
            .iter()
            .map(|collection| collection.entries.len())
            .sum(),
        diagnostics: diagnostics.len(),
    }
}

fn melt_layer_reports(root: &Path, summary: &MeltSummary) -> Vec<MeltLayerReport> {
    let foundry_ready = resolve_package_asset_root(root, "axonyx-ui").is_some()
        || cargo_manifest_has_dependency_file(&root.join("Cargo.toml"), "axonyx-ui")
            .unwrap_or(false);

    vec![
        MeltLayerReport {
            name: "Axonyx Pages",
            status: if summary.page_routes > 0 {
                "ready"
            } else {
                "empty"
            },
            detail: format!("{} page route(s) discovered.", summary.page_routes),
        },
        MeltLayerReport {
            name: "Axonyx Server",
            status: "ready",
            detail: format!(
                "{} API route(s), {} action route(s), stream_pages={}.",
                summary.api_routes,
                summary.action_routes,
                axonyx_config_bool(root, "server", "stream_pages").unwrap_or(false)
            ),
        },
        MeltLayerReport {
            name: "Axonyx State",
            status: if summary.state_signals > 0 {
                "ready"
            } else {
                "empty"
            },
            detail: format!("{} state signal(s) declared.", summary.state_signals),
        },
        MeltLayerReport {
            name: "Axonyx Foundry",
            status: if foundry_ready { "ready" } else { "optional" },
            detail: if foundry_ready {
                "Foundry UI package is available.".to_string()
            } else {
                "Run `cargo ax add ui` when this app needs Foundry components.".to_string()
            },
        },
        MeltLayerReport {
            name: "Axonyx Melt",
            status: if summary.diagnostics == 0 {
                "ready"
            } else {
                "attention"
            },
            detail: if summary.diagnostics == 0 {
                "Project graph collected without source diagnostics.".to_string()
            } else {
                format!(
                    "{} source diagnostic(s) must be fixed.",
                    summary.diagnostics
                )
            },
        },
    ]
}

fn routes_report(root: &Path) -> Result<RoutesReport> {
    Ok(RoutesReport {
        stream_pages: axonyx_config_bool(root, "server", "stream_pages").unwrap_or(false),
        routes: collect_app_route_manifest(root)?,
    })
}

fn api_command(args: ApiArgs) -> Result<()> {
    let root = app_root()?;
    let report = collect_api_report(&root)?;

    if args.schema && args.openapi {
        bail!("choose either --schema or --openapi, not both");
    }

    if args.openapi {
        let output = serde_json::to_string_pretty(&api_report_openapi_value(&report))?;
        write_or_print_api_output(args.out.as_deref(), &output)?;
        return Ok(());
    }

    if args.schema {
        if args.out.is_some() {
            bail!("--out is currently supported only with --openapi");
        }
        print_api_schema_text(&report);
        return Ok(());
    }

    if args.out.is_some() {
        bail!("--out is currently supported only with --openapi");
    }

    match args.format {
        CheckFormat::Text => print_api_text(&report),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn write_or_print_api_output(out: Option<&Path>, output: &str) -> Result<()> {
    let Some(out) = out else {
        println!("{output}");
        return Ok(());
    };

    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory '{}'", parent.display()))?;
    }

    fs::write(out, format!("{output}\n"))
        .with_context(|| format!("failed to write API output '{}'", out.display()))?;
    println!("Wrote API contract to {}", display_path(out));
    Ok(())
}

fn collect_api_report(root: &Path) -> Result<ApiReport> {
    let routes = collect_app_route_manifest(root)?
        .into_iter()
        .filter(|route| route.kind == "api")
        .map(|route| ApiRouteReport {
            method: route.method.unwrap_or_else(|| "*".to_string()),
            route: route.route,
            returns: route.returns,
            file: route.file,
            params: route.params,
            inputs: route.inputs,
            hooks: route.hooks,
        })
        .collect();

    Ok(ApiReport {
        routes,
        schemas: collect_project_type_schemas(root)?,
    })
}

fn collect_action_report(root: &Path) -> Result<ActionReport> {
    let mut routes = Vec::new();

    for route in collect_page_route_manifest(root)? {
        let Some(actions_file) = &route.actions else {
            continue;
        };
        let actions_path = root.join(actions_file);
        let source = fs::read_to_string(&actions_path)
            .with_context(|| format!("failed to read '{}'", actions_path.display()))?;
        let document = parse_backend_ax(&source).with_context(|| {
            format!("failed to parse action source '{}'", actions_path.display())
        })?;

        let actions = document
            .blocks
            .into_iter()
            .filter_map(|block| {
                let AxBackendBlock::Action(action) = block else {
                    return None;
                };

                let inputs = action
                    .input
                    .into_iter()
                    .map(|field| ActionInputReport {
                        name: field.name,
                        ty: field.ty,
                        optional: field.optional,
                        default: field.default.as_ref().map(format_ax_expr),
                    })
                    .collect();

                Some(ActionItemReport {
                    name: action.name,
                    returns: action.returns,
                    inputs,
                })
            })
            .collect::<Vec<_>>();

        if !actions.is_empty() {
            routes.push(ActionRouteReport {
                route: route.route,
                file: actions_file.clone(),
                actions,
            });
        }
    }

    Ok(ActionReport { routes })
}

fn content_command(args: ContentArgs) -> Result<()> {
    let root = app_root()?;
    let manifest = collect_content_manifest(&root)?;

    match args.format {
        CheckFormat::Text => print_content_text(&manifest),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
    }

    Ok(())
}

fn schema_command(args: SchemaArgs) -> Result<()> {
    match args.command {
        SchemaCommands::Pull(args) => schema_pull_command(args),
    }
}

fn state_command(args: StateArgs) -> Result<()> {
    let root = app_root()?;
    let report = collect_state_report(&root)?;

    match args.format {
        CheckFormat::Text => print_state_text(&report),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn test_command(args: TestArgs) -> Result<()> {
    let mode = args.command.unwrap_or(TestCommands::Routes);

    match mode {
        TestCommands::Routes => run_aegis_fast_tests(&args),
        TestCommands::Components => {
            println!("Aegis component tests are reserved for a future Axonyx release.");
            println!("Run route smoke checks today with:");
            println!("  cargo ax test");
            Ok(())
        }
        TestCommands::Browser => {
            println!("Aegis browser tests are reserved for a future Axonyx release.");
            println!("Run route smoke checks today with:");
            println!("  cargo ax test");
            Ok(())
        }
    }
}

fn run_aegis_fast_tests(args: &TestArgs) -> Result<()> {
    let root = app_root()?;
    let config_path = if args.config.is_absolute() {
        args.config.clone()
    } else {
        root.join(&args.config)
    };

    if !config_path.exists() {
        bail!(
            "Aegis config '{}' was not found.\nCreate one with `aegis init`, or scaffold a new app with the latest `create-axonyx`.",
            config_path.display()
        );
    }

    let mut command = Command::new("aegis");
    command
        .arg("fast")
        .arg("--config")
        .arg(&config_path)
        .arg("--format")
        .arg(match args.format {
            CheckFormat::Text => "text",
            CheckFormat::Json => "json",
        })
        .arg("--fail-fast")
        .arg(if args.fail_fast { "true" } else { "false" })
        .current_dir(&root);

    println!("Running Aegis fast QA from cargo ax test");
    println!("Config: {}", config_path.display());

    let status = command
        .status()
        .context("failed to start `aegis`; install it with `cargo install axonyx-aegis --force`")?;

    if !status.success() {
        bail!("Aegis fast QA failed with status {status}");
    }

    Ok(())
}

fn schema_pull_command(args: SchemaPullArgs) -> Result<()> {
    let source = read_schema_source(&args.source)?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .with_context(|| format!("failed to parse schema source '{}' as JSON", args.source))?;
    let schema = match schema_from_typed_envelope(&args.name, &value)? {
        Some(schema) => schema,
        None => infer_schema_from_json(&args.name, &value)
            .with_context(|| "failed to infer schema from JSON source")?,
    };

    let rendered = match args.format {
        SchemaFormat::Ax => render_schema_as_ax(&schema),
        SchemaFormat::Json => serde_json::to_string_pretty(&schema)?,
    };

    if let Some(path) = args.out {
        fs::write(&path, rendered)
            .with_context(|| format!("failed to write schema output '{}'", path.display()))?;
    } else {
        println!("{rendered}");
    }

    Ok(())
}

fn read_schema_source(source: &str) -> Result<String> {
    if source.starts_with("http://") {
        return read_http_text(source);
    }

    let path = Path::new(source);
    if path.exists() {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read schema source '{}'", path.display()));
    }

    if source.trim_start().starts_with('{') || source.trim_start().starts_with('[') {
        return Ok(source.to_string());
    }

    bail!(
        "schema source '{}' is not a file, inline JSON, or http:// endpoint",
        source
    )
}

fn read_http_text(url: &str) -> Result<String> {
    let request = parse_http_url(url)?;
    let mut stream = TcpStream::connect((request.host.as_str(), request.port))
        .with_context(|| format!("failed to connect to {}", request.authority))?;
    let request_text = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        request.path, request.authority
    );
    stream
        .write_all(request_text.as_bytes())
        .with_context(|| format!("failed to send request to {url}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("failed to read response from {url}"))?;

    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        bail!("invalid HTTP response from {url}");
    };
    let status = head.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") {
        bail!("schema endpoint returned non-200 status: {status}");
    }

    Ok(body.to_string())
}

struct ParsedHttpUrl {
    authority: String,
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> Result<ParsedHttpUrl> {
    let Some(rest) = url.strip_prefix("http://") else {
        bail!("only http:// schema endpoints are supported in this draft")
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() {
        bail!("schema endpoint host is empty")
    }
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        (
            host.to_string(),
            port.parse::<u16>()
                .with_context(|| format!("invalid schema endpoint port '{port}'"))?,
        )
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() {
        bail!("schema endpoint host is empty")
    }

    Ok(ParsedHttpUrl {
        authority: authority.to_string(),
        host,
        port,
        path: format!("/{path}"),
    })
}

fn infer_schema_from_json(root_name: &str, value: &serde_json::Value) -> Result<InferredSchema> {
    let mut records = Vec::new();
    let root_type = infer_value_type(root_name, value, &mut records)?;
    Ok(InferredSchema { root_type, records })
}

fn schema_from_typed_envelope(
    root_name: &str,
    value: &serde_json::Value,
) -> Result<Option<InferredSchema>> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some(schema_value) = object.get("schema") else {
        return Ok(None);
    };
    let schema_object = schema_value
        .as_object()
        .with_context(|| "typed schema envelope field 'schema' must be an object")?;

    let mut records = Vec::new();
    for (record_name, fields_value) in schema_object {
        let fields_object = fields_value.as_object().with_context(|| {
            format!("typed schema record '{record_name}' must be an object of fields")
        })?;
        let mut fields = Vec::new();
        for (field_name, field_value) in fields_object {
            let (ty, optional) = parse_schema_field_type(field_name, field_value)?;
            fields.push(InferredField {
                name: field_name.to_string(),
                ty,
                optional,
            });
        }
        records.push(InferredRecord {
            name: sanitize_type_name(record_name),
            fields,
        });
    }

    let root_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            object
                .get("data")
                .and_then(|data| infer_schema_from_json(root_name, data).ok())
                .map(|schema| schema.root_type)
        })
        .unwrap_or_else(|| sanitize_type_name(root_name));

    Ok(Some(InferredSchema { root_type, records }))
}

fn parse_schema_field_type(field_name: &str, value: &serde_json::Value) -> Result<(String, bool)> {
    if let Some(ty) = value.as_str() {
        return Ok(normalize_schema_type(ty));
    }

    let Some(object) = value.as_object() else {
        bail!("typed schema field '{field_name}' must be a type string or object")
    };
    let ty = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("typed schema field '{field_name}' is missing string 'type'"))?;
    let (ty, wrapped_optional) = normalize_schema_type(ty);
    let optional = object
        .get("optional")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(wrapped_optional);
    Ok((ty, optional))
}

fn normalize_schema_type(ty: &str) -> (String, bool) {
    let trimmed = ty.trim();
    if let Some(inner) = trimmed
        .strip_prefix("Optional<")
        .and_then(|value| value.strip_suffix('>'))
    {
        (inner.trim().to_string(), true)
    } else {
        (trimmed.to_string(), false)
    }
}

fn infer_value_type(
    name_hint: &str,
    value: &serde_json::Value,
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    Ok(match value {
        serde_json::Value::Null => "Unknown".to_string(),
        serde_json::Value::Bool(_) => "Bool".to_string(),
        serde_json::Value::Number(_) => "Number".to_string(),
        serde_json::Value::String(_) => "String".to_string(),
        serde_json::Value::Array(items) => {
            let item_name = singular_type_name(name_hint);
            let item_type = infer_array_item_type(&item_name, items, records)?;
            format!("List<{item_type}>")
        }
        serde_json::Value::Object(map) => {
            let record_name = sanitize_type_name(name_hint);
            let objects = vec![map];
            let record = infer_record(&record_name, &objects, records)?;
            records.push(record);
            record_name
        }
    })
}

fn infer_array_item_type(
    name_hint: &str,
    items: &[serde_json::Value],
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    let objects = items
        .iter()
        .filter_map(serde_json::Value::as_object)
        .collect::<Vec<_>>();
    if !objects.is_empty() && objects.len() == items.len() {
        let record_name = sanitize_type_name(name_hint);
        let record = infer_record(&record_name, &objects, records)?;
        records.push(record);
        return Ok(record_name);
    }

    let mut ty = None;
    let mut optional = false;
    for item in items {
        if item.is_null() {
            optional = true;
            continue;
        }
        let next = infer_value_type(name_hint, item, records)?;
        ty = Some(match ty {
            None => next,
            Some(current) if current == next => current,
            Some(_) => "Unknown".to_string(),
        });
    }

    let ty = ty.unwrap_or_else(|| "Unknown".to_string());
    Ok(if optional {
        format!("Optional<{ty}>")
    } else {
        ty
    })
}

fn infer_record(
    record_name: &str,
    objects: &[&serde_json::Map<String, serde_json::Value>],
    records: &mut Vec<InferredRecord>,
) -> Result<InferredRecord> {
    let mut keys = std::collections::BTreeSet::new();
    for object in objects {
        keys.extend(object.keys().cloned());
    }

    let mut fields = Vec::new();
    for key in keys {
        let mut values = Vec::new();
        let mut optional = false;
        for object in objects {
            match object.get(&key) {
                Some(value) if value.is_null() => optional = true,
                Some(value) => values.push(value),
                None => optional = true,
            }
        }

        let field_type = infer_field_type(record_name, &key, &values, records)?;
        fields.push(InferredField {
            name: key,
            ty: field_type,
            optional,
        });
    }

    Ok(InferredRecord {
        name: record_name.to_string(),
        fields,
    })
}

fn infer_field_type(
    record_name: &str,
    field_name: &str,
    values: &[&serde_json::Value],
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    if values.is_empty() {
        return Ok("Unknown".to_string());
    }

    let nested_name = format!("{record_name}{}", sanitize_type_name(field_name));
    let mut ty = None;
    for value in values {
        let next = infer_value_type(&nested_name, value, records)?;
        ty = Some(match ty {
            None => next,
            Some(current) if current == next => current,
            Some(_) => "Unknown".to_string(),
        });
    }
    Ok(ty.unwrap_or_else(|| "Unknown".to_string()))
}

fn render_schema_as_ax(schema: &InferredSchema) -> String {
    let mut out = String::new();
    for record in &schema.records {
        out.push_str(&format!("type {} {{\n", record.name));
        for field in &record.fields {
            if field.optional {
                out.push_str(&format!("  {}?: {}\n", field.name, field.ty));
            } else {
                out.push_str(&format!("  {}: {}\n", field.name, field.ty));
            }
        }
        out.push_str("}\n\n");
    }
    out.push_str(&format!("// root: {}\n", schema.root_type));
    out.trim_end().to_string()
}

fn sanitize_type_name(input: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if capitalize_next {
                out.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                out.push(ch);
            }
        } else {
            capitalize_next = true;
        }
    }
    if out.is_empty() {
        "Item".to_string()
    } else if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        format!("T{out}")
    } else {
        out
    }
}

fn singular_type_name(input: &str) -> String {
    let name = sanitize_type_name(input);
    name.strip_suffix('s').unwrap_or(&name).to_string()
}

fn print_content_text(manifest: &ContentManifest) {
    if manifest.collections.is_empty() {
        println!("No content collections configured.");
        println!("Add [content.collections.<name>] entries to Axonyx.toml.");
        return;
    }

    println!("Content collections:");
    for collection in &manifest.collections {
        println!(
            "  {:<18} path={} entries={} extensions={}",
            collection.name,
            collection.path,
            collection.entries.len(),
            collection.extensions.join(",")
        );

        for entry in &collection.entries {
            println!(
                "    {:<32} slug={} title=\"{}\" words={} bytes={}",
                entry.path, entry.slug, entry.title, entry.word_count, entry.bytes
            );
            if !entry.excerpt.is_empty() {
                println!("      {}", entry.excerpt);
            }
        }
    }
}

fn collect_content_manifest(root: &Path) -> Result<ContentManifest> {
    let configs = load_content_collection_configs(root)?;
    let mut collections = Vec::new();

    for config in configs {
        let entries = collect_content_entries(root, &config)?;
        collections.push(ContentCollectionManifest {
            name: config.name,
            path: display_relative_path(root, &config.path),
            extensions: config.extensions,
            entries,
        });
    }

    Ok(ContentManifest { collections })
}

fn preview_store_from_content(root: &Path) -> Result<AxPreviewStore> {
    let manifest = collect_content_manifest(root)?;
    let mut store = AxPreviewStore::default();

    for collection in manifest.collections {
        let items = collection
            .entries
            .into_iter()
            .map(content_entry_to_record)
            .collect();
        store = store.with_collection(collection.name, items);
    }

    Ok(store)
}

fn content_entry_to_record(entry: ContentEntryManifest) -> AxValue {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("path".to_string(), AxValue::from(entry.path));
    fields.insert("slug".to_string(), AxValue::from(entry.slug));
    fields.insert("extension".to_string(), AxValue::from(entry.extension));
    fields.insert(
        "content_type".to_string(),
        AxValue::from(entry.content_type),
    );
    fields.insert("bytes".to_string(), AxValue::from(entry.bytes as i64));
    fields.insert("title".to_string(), AxValue::from(entry.title));
    fields.insert("excerpt".to_string(), AxValue::from(entry.excerpt));
    fields.insert(
        "word_count".to_string(),
        AxValue::from(entry.word_count as i64),
    );
    fields.insert("body".to_string(), AxValue::from(entry.body));
    fields.insert("html".to_string(), AxValue::from(entry.html));
    for (key, value) in entry.frontmatter {
        fields.insert(key, AxValue::from(value));
    }
    AxValue::Record(fields)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContentCollectionConfig {
    name: String,
    path: PathBuf,
    extensions: Vec<String>,
}

fn load_content_collection_configs(root: &Path) -> Result<Vec<ContentCollectionConfig>> {
    let Some(collections_value) = axonyx_config_value(root, "content", "collections") else {
        return Ok(Vec::new());
    };

    let collections = collections_value
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("[content].collections must be a TOML table"))?;
    let mut out = Vec::new();

    for (name, value) in collections {
        let table = value
            .as_table()
            .ok_or_else(|| anyhow::anyhow!("[content.collections.{name}] must be a TOML table"))?;
        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("[content.collections.{name}] is missing path"))?;
        let path = resolve_content_collection_path(root, path)?;
        let extensions = table
            .get("extensions")
            .map(content_extensions_from_value)
            .transpose()?
            .unwrap_or_else(|| vec!["md".to_string(), "mdx".to_string()]);

        out.push(ContentCollectionConfig {
            name: name.to_string(),
            path,
            extensions,
        });
    }

    out.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(out)
}

fn content_extensions_from_value(value: &toml::Value) -> Result<Vec<String>> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("content collection extensions must be an array"))?;
    let mut extensions = Vec::new();

    for value in values {
        let extension = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("content collection extension must be a string"))?
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if extension.is_empty()
            || extension.contains('/')
            || extension.contains('\\')
            || extension == "."
            || extension == ".."
        {
            bail!("invalid content collection extension '{extension}'");
        }
        if !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }

    Ok(extensions)
}

fn resolve_content_collection_path(root: &Path, path: &str) -> Result<PathBuf> {
    if path.contains('\0') {
        bail!("content collection path contains an invalid null byte");
    }

    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };

    let normalized = normalize_content_path(&resolved)?;
    let root = normalize_content_path(root)?;
    if !normalized.starts_with(&root) {
        bail!(
            "content collection path '{}' must stay inside app root '{}'",
            normalized.display(),
            root.display()
        );
    }

    Ok(normalized)
}

fn normalize_content_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    bail!("content collection path cannot escape app root");
                }
            }
            std::path::Component::Normal(segment) => normalized.push(segment),
        }
    }
    Ok(normalized)
}

fn collect_content_entries(
    root: &Path,
    config: &ContentCollectionConfig,
) -> Result<Vec<ContentEntryManifest>> {
    if !config.path.exists() {
        return Ok(Vec::new());
    }
    if !config.path.is_dir() {
        bail!(
            "content collection '{}' path '{}' is not a directory",
            config.name,
            config.path.display()
        );
    }

    let mut entries = Vec::new();
    collect_content_entries_in_dir(root, config, &config.path, &mut entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_content_entries_in_dir(
    root: &Path,
    config: &ContentCollectionConfig,
    dir: &Path,
    out: &mut Vec<ContentEntryManifest>,
) -> Result<()> {
    let mut children = fs::read_dir(dir)
        .with_context(|| format!("failed to read content directory '{}'", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read content directory '{}'", dir.display()))?;

    children.sort_by_key(|entry| entry.path());

    for entry in children {
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name.to_string_lossy().starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_content_entries_in_dir(root, config, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .unwrap_or_default();
        if !config.extensions.contains(&extension) {
            continue;
        }

        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect content file '{}'", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read content file '{}'", path.display()))?;
        let (frontmatter, body) = parse_content_frontmatter(&source);
        let title = content_title(&frontmatter, &body, &path);
        let excerpt = content_excerpt(&frontmatter, &body);
        let html = render_content_html(&body, &extension);
        out.push(ContentEntryManifest {
            path: display_relative_path(root, &path),
            slug: content_slug(&config.path, &path),
            content_type: content_type_for_extension(&extension).to_string(),
            extension,
            bytes: metadata.len(),
            title,
            excerpt,
            word_count: content_word_count(&body),
            frontmatter,
            body,
            html,
        });
    }

    Ok(())
}

fn parse_content_frontmatter(source: &str) -> (std::collections::BTreeMap<String, String>, String) {
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return (std::collections::BTreeMap::new(), source.to_string());
    };

    let Some((frontmatter_source, body)) = split_frontmatter_body(rest) else {
        return (std::collections::BTreeMap::new(), source.to_string());
    };

    let mut frontmatter = std::collections::BTreeMap::new();
    for line in frontmatter_source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !is_content_frontmatter_key(key) {
            continue;
        }
        frontmatter.insert(key.to_string(), trim_frontmatter_value(value));
    }

    (frontmatter, body.to_string())
}

fn split_frontmatter_body(source: &str) -> Option<(&str, &str)> {
    if let Some((frontmatter, body)) = source.split_once("\n---\n") {
        return Some((frontmatter, body));
    }
    if let Some((frontmatter, body)) = source.split_once("\r\n---\r\n") {
        return Some((frontmatter, body));
    }
    if let Some((frontmatter, body)) = source.split_once("\n---\r\n") {
        return Some((frontmatter, body));
    }
    source
        .split_once("\r\n---\n")
        .map(|(frontmatter, body)| (frontmatter, body))
}

fn is_content_frontmatter_key(key: &str) -> bool {
    key.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn trim_frontmatter_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len().saturating_sub(1)].to_string()
    } else {
        value.to_string()
    }
}

fn content_type_for_extension(extension: &str) -> &'static str {
    match extension {
        "md" | "mdx" => "markdown",
        "html" | "htm" => "html",
        "json" => "json",
        _ => "text",
    }
}

fn render_content_html(body: &str, extension: &str) -> String {
    match extension {
        "md" | "mdx" => render_markdown_html(body),
        "html" | "htm" => body.to_string(),
        _ => format!("<pre><code>{}</code></pre>", html_escape(body)),
    }
}

fn render_markdown_html(source: &str) -> String {
    let mut html = String::new();
    let mut paragraph = Vec::<String>::new();
    let mut list_open = false;
    let mut code_open = false;
    let mut code = String::new();

    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if code_open {
                html.push_str("<pre><code>");
                html.push_str(&html_escape(code.trim_end_matches('\n')));
                html.push_str("</code></pre>");
                code.clear();
                code_open = false;
            } else {
                flush_markdown_paragraph(&mut html, &mut paragraph);
                if list_open {
                    html.push_str("</ul>");
                    list_open = false;
                }
                code_open = true;
            }
            continue;
        }

        if code_open {
            code.push_str(line);
            code.push('\n');
            continue;
        }

        if trimmed.is_empty() {
            flush_markdown_paragraph(&mut html, &mut paragraph);
            if list_open {
                html.push_str("</ul>");
                list_open = false;
            }
            continue;
        }

        if let Some((level, text)) = markdown_heading(trimmed) {
            flush_markdown_paragraph(&mut html, &mut paragraph);
            if list_open {
                html.push_str("</ul>");
                list_open = false;
            }
            html.push_str(&format!(
                "<h{level}>{}</h{level}>",
                render_markdown_inline(text.trim())
            ));
            continue;
        }

        if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            flush_markdown_paragraph(&mut html, &mut paragraph);
            if !list_open {
                html.push_str("<ul>");
                list_open = true;
            }
            html.push_str("<li>");
            html.push_str(&render_markdown_inline(item.trim()));
            html.push_str("</li>");
            continue;
        }

        paragraph.push(trimmed.to_string());
    }

    if code_open {
        html.push_str("<pre><code>");
        html.push_str(&html_escape(code.trim_end_matches('\n')));
        html.push_str("</code></pre>");
    }
    flush_markdown_paragraph(&mut html, &mut paragraph);
    if list_open {
        html.push_str("</ul>");
    }

    html
}

fn flush_markdown_paragraph(html: &mut String, paragraph: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join(" ");
    html.push_str("<p>");
    html.push_str(&render_markdown_inline(&text));
    html.push_str("</p>");
    paragraph.clear();
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line.get(hashes..)?;
    rest.strip_prefix(' ').map(|text| (hashes, text))
}

fn render_markdown_inline(value: &str) -> String {
    let escaped = html_escape(value);
    let with_code = replace_inline_markers(&escaped, "`", "code");
    let with_strong = replace_inline_markers(&with_code, "**", "strong");
    render_markdown_links(&with_strong)
}

fn replace_inline_markers(value: &str, marker: &str, tag: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    let mut open = false;
    while let Some(index) = rest.find(marker) {
        out.push_str(&rest[..index]);
        if open {
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        } else {
            out.push('<');
            out.push_str(tag);
            out.push('>');
        }
        open = !open;
        rest = &rest[index + marker.len()..];
    }
    out.push_str(rest);
    if open {
        out.push_str(marker);
    }
    out
}

fn render_markdown_links(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('[') {
        let before = &rest[..start];
        let candidate = &rest[start + 1..];
        let Some(label_end) = candidate.find("](") else {
            break;
        };
        let after_label = &candidate[label_end + 2..];
        let Some(url_end) = after_label.find(')') else {
            break;
        };
        let label = &candidate[..label_end];
        let url = &after_label[..url_end];
        if !is_safe_markdown_url(url) {
            break;
        }
        out.push_str(before);
        out.push_str("<a href=\"");
        out.push_str(&html_escape(url));
        out.push_str("\">");
        out.push_str(label);
        out.push_str("</a>");
        rest = &after_label[url_end + 1..];
    }
    out.push_str(rest);
    out
}

fn is_safe_markdown_url(url: &str) -> bool {
    url.starts_with('/')
        || url.starts_with('#')
        || url.starts_with("https://")
        || url.starts_with("http://")
}

fn content_title(
    frontmatter: &std::collections::BTreeMap<String, String>,
    body: &str,
    path: &Path,
) -> String {
    if let Some(title) = frontmatter
        .get("title")
        .filter(|value| !value.trim().is_empty())
    {
        return title.trim().to_string();
    }

    if let Some(heading) = first_markdown_heading(body) {
        return heading;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(humanize_slug)
        .unwrap_or_default()
}

fn content_excerpt(frontmatter: &std::collections::BTreeMap<String, String>, body: &str) -> String {
    for key in ["excerpt", "summary", "description"] {
        if let Some(value) = frontmatter
            .get(key)
            .filter(|value| !value.trim().is_empty())
        {
            return collapse_ws(value);
        }
    }

    body.lines()
        .map(strip_markdown_line)
        .map(|line| collapse_ws(&line))
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

fn content_word_count(body: &str) -> usize {
    strip_markdown_line(body)
        .split_whitespace()
        .filter(|word| !word.trim().is_empty())
        .count()
}

fn first_markdown_heading(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let line = line.trim();
        let heading = line.strip_prefix("# ")?;
        let heading = collapse_ws(heading);
        (!heading.is_empty()).then_some(heading)
    })
}

fn strip_markdown_line(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_html_tag = false;
    for ch in value.chars() {
        if in_html_tag {
            if ch == '>' {
                in_html_tag = false;
                out.push(' ');
            }
            continue;
        }
        match ch {
            '#' | '*' | '_' | '`' | '>' | '[' | ']' | '(' | ')' => out.push(' '),
            '<' => in_html_tag = true,
            _ => out.push(ch),
        }
    }
    out
}

fn collapse_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn humanize_slug(value: &str) -> String {
    value
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().collect::<String>();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn content_slug(collection_root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(collection_root).unwrap_or(path);
    let mut segments = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if let Some(last) = segments.last_mut() {
        if let Some((stem, _)) = last.rsplit_once('.') {
            *last = stem.to_string();
        }
    }

    segments.join("/")
}

fn check_app_sources(root: &Path) -> Result<Vec<CheckDiagnostic>> {
    let mut files = Vec::new();
    collect_ax_files(&root.join("app"), &mut files)?;
    collect_ax_files(&root.join("routes"), &mut files)?;
    collect_ax_files(&root.join("jobs"), &mut files)?;

    let mut diagnostics = Vec::new();
    for file in files {
        diagnostics.extend(check_ax_file_with_root(&file, Some(root))?);
    }
    diagnostics.extend(check_axonyx_config(root)?);
    diagnostics.extend(check_route_manifest(root)?);

    Ok(diagnostics)
}

fn check_axonyx_config(root: &Path) -> Result<Vec<CheckDiagnostic>> {
    let path = root.join("Axonyx.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read '{}'", path.display()))?;
    let value = match source.parse::<toml::Value>() {
        Ok(value) => value,
        Err(error) => {
            return Ok(vec![CheckDiagnostic {
                file: display_path(&path),
                line: 1,
                column: 1,
                severity: "error",
                code: "axonyx-config",
                message: format!("failed to parse Axonyx.toml: {error}"),
            }]);
        }
    };
    let mut diagnostics = Vec::new();
    if let Some(stream_pages) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("stream_pages"))
    {
        let valid = match stream_pages {
            toml::Value::Boolean(_) => true,
            toml::Value::String(value) => parse_boolish_strict(value).is_some(),
            _ => false,
        };

        if !valid {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "stream_pages"),
                column: 1,
                severity: "error",
                code: "axonyx-config-stream-pages",
                message:
                    "[server].stream_pages must be a boolean or one of true/false/1/0/yes/no/on/off."
                        .to_string(),
            });
        }
    }

    if let Some(max_body_bytes) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("max_body_bytes"))
    {
        if parse_max_body_bytes_value(max_body_bytes).is_err() {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "max_body_bytes"),
                column: 1,
                severity: "error",
                code: "axonyx-config-max-body-bytes",
                message: "[server].max_body_bytes must be a positive integer or a string such as \"512kb\", \"1mb\", or \"2gb\"."
                    .to_string(),
            });
        }
    }

    if let Some(request_timeout) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("request_timeout_seconds"))
    {
        if parse_request_timeout_seconds_value(request_timeout).is_err() {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "request_timeout_seconds"),
                column: 1,
                severity: "error",
                code: "axonyx-config-request-timeout",
                message: "[server].request_timeout_seconds must be a positive integer.".to_string(),
            });
        }
    }

    if let Some(shutdown_grace) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("shutdown_grace_seconds"))
    {
        if parse_shutdown_grace_seconds_value(shutdown_grace).is_err() {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "shutdown_grace_seconds"),
                column: 1,
                severity: "error",
                code: "axonyx-config-shutdown-grace",
                message: "[server].shutdown_grace_seconds must be a positive integer.".to_string(),
            });
        }
    }

    if let Some(max_connections) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("max_connections"))
    {
        if parse_max_connections_value(max_connections).is_err() {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "max_connections"),
                column: 1,
                severity: "error",
                code: "axonyx-config-max-connections",
                message: "[server].max_connections must be a positive integer.".to_string(),
            });
        }
    }

    for key in ["compression", "security_headers", "request_logging"] {
        if let Some(value) = value
            .get("server")
            .and_then(toml::Value::as_table)
            .and_then(|server| server.get(key))
        {
            if parse_bool_config_value(value).is_err() {
                diagnostics.push(CheckDiagnostic {
                    file: display_path(&path),
                    line: line_for_config_key(&source, key),
                    column: 1,
                    severity: "error",
                    code: match key {
                        "compression" => "axonyx-config-compression",
                        "security_headers" => "axonyx-config-security-headers",
                        _ => "axonyx-config-request-logging",
                    },
                    message: format!(
                        "[server].{key} must be a boolean or one of true/false/1/0/yes/no/on/off."
                    ),
                });
            }
        }
    }

    if let Some(log_format) = value
        .get("server")
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("log_format"))
    {
        if parse_server_log_format_value(log_format).is_err() {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&path),
                line: line_for_config_key(&source, "log_format"),
                column: 1,
                severity: "error",
                code: "axonyx-config-log-format",
                message: "[server].log_format must be \"text\" or \"json\".".to_string(),
            });
        }
    }

    Ok(diagnostics)
}

fn line_for_config_key(source: &str, key: &str) -> usize {
    source
        .lines()
        .position(|line| line.trim_start().starts_with(key))
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn check_route_manifest(root: &Path) -> Result<Vec<CheckDiagnostic>> {
    let routes = collect_app_route_manifest(root)?;
    let mut seen = std::collections::BTreeMap::<String, RouteManifestItem>::new();
    let mut diagnostics = Vec::new();

    for route in routes {
        let key = route_conflict_key(&route);
        if let Some(existing) = seen.get(&key) {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&root.join(&route.file)),
                line: 1,
                column: 1,
                severity: "error",
                code: "axonyx-route-duplicate",
                message: format!(
                    "duplicate {} route `{}` also defined in '{}'",
                    route.kind,
                    route_display_name(&route),
                    existing.file
                ),
            });
        } else {
            seen.insert(key, route);
        }
    }

    Ok(diagnostics)
}

fn route_conflict_key(route: &RouteManifestItem) -> String {
    let canonical_route = canonical_route_conflict_pattern(&route.route);
    match route.kind {
        "api" => format!(
            "api:{}:{}",
            route.method.as_deref().unwrap_or("*"),
            canonical_route
        ),
        _ => format!("page:{canonical_route}"),
    }
}

fn canonical_route_conflict_pattern(route: &str) -> String {
    route
        .split('/')
        .map(|segment| {
            if segment.starts_with(':') && segment.len() > 1 {
                ":param"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn route_display_name(route: &RouteManifestItem) -> String {
    route
        .method
        .as_ref()
        .map(|method| format!("{method} {}", route.route))
        .unwrap_or_else(|| route.route.clone())
}

fn collect_ax_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read '{}'", dir.display()))? {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_ax_files(&path, out)?;
            continue;
        }

        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("ax") {
            out.push(path);
        }
    }

    Ok(())
}

fn check_ax_file(path: &Path) -> Result<Vec<CheckDiagnostic>> {
    let root = find_app_root_for_path(path);
    check_ax_file_with_root(path, root.as_deref())
}

fn check_ax_file_with_root(path: &Path, root: Option<&Path>) -> Result<Vec<CheckDiagnostic>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read .ax file '{}'", path.display()))?;
    Ok(check_ax_source_with_root(path, &source, root))
}

fn check_ax_source_with_root(
    path: &Path,
    source: &str,
    root: Option<&Path>,
) -> Vec<CheckDiagnostic> {
    if looks_like_backend_ax(source) {
        return match parse_backend_ax(source) {
            Ok(document) => check_backend_requirements(path, source, root, &document),
            Err(error) => vec![diagnostic_from_parse_error(
                path,
                CheckParseError::Backend(error),
            )],
        };
    }

    let document = match parse_ax_auto(source) {
        Ok(document) => document,
        Err(error) => {
            return vec![diagnostic_from_parse_error(
                path,
                CheckParseError::Page(error),
            )]
        }
    };

    let mut diagnostics = Vec::new();
    diagnostics.extend(check_type_annotations(path, source, &document));
    if let Some(root) = root {
        diagnostics.extend(check_imports(root, path, source, &document.imports));
    }
    diagnostics
}

fn check_backend_requirements(
    path: &Path,
    source: &str,
    root: Option<&Path>,
    document: &AxBackendDocument,
) -> Vec<CheckDiagnostic> {
    let plan = match lower_backend_document(document) {
        Ok(plan) => plan,
        Err(error) => {
            return vec![CheckDiagnostic {
                file: display_path(path),
                line: 1,
                column: 1,
                severity: "error",
                code: "axonyx-backend-lower",
                message: error.to_string(),
            }]
        }
    };

    let type_names = root.and_then(|root| collect_project_type_names(root).ok());
    let mut diagnostics = check_backend_route_inputs(path, source, document, &plan);
    diagnostics.extend(check_backend_return_contracts(
        path,
        source,
        &plan,
        type_names.as_ref(),
    ));

    if !backend_plan_uses_signed_session(&plan)
        || auth_secret_configured(root, "AX_SECRET_SESSION_KEY")
    {
        return diagnostics;
    }

    diagnostics.push(CheckDiagnostic {
        file: display_path(path),
        line: line_for_source_pattern(source, "Auth.signedSession"),
        column: 1,
        severity: "error",
        code: "axonyx-auth-secret",
        message: "Auth.signedSession requires AX_SECRET_SESSION_KEY. Set it in the environment or local .env file.".to_string(),
    });

    diagnostics
}

fn check_backend_route_inputs(
    path: &Path,
    source: &str,
    document: &AxBackendDocument,
    plan: &AxBackendPlan,
) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();

    for block in &document.blocks {
        let AxBackendBlock::Route(route) = block else {
            continue;
        };

        let mut seen = std::collections::BTreeSet::new();
        for field in &route.input {
            if !is_supported_route_input_type(&field.ty) {
                diagnostics.push(CheckDiagnostic {
                    file: display_path(path),
                    line: line_for_source_pattern(source, &format!("{}:", field.name)),
                    column: 1,
                    severity: "error",
                    code: "axonyx-route-input-type",
                    message: format!(
                        "route input `{}` uses unsupported type `{}`. Supported route input types are string, bool, i64, u64, and f64.",
                        field.name, field.ty
                    ),
                });
            }

            if !seen.insert(field.name.clone()) {
                diagnostics.push(CheckDiagnostic {
                    file: display_path(path),
                    line: line_for_repeated_source_pattern(source, &format!("{}:", field.name), 2),
                    column: 1,
                    severity: "error",
                    code: "axonyx-route-input-duplicate",
                    message: format!("route input `{}` is declared more than once.", field.name),
                });
            }
        }
    }

    for handler in &plan.handlers {
        let AxHandlerKind::Route { input, .. } = &handler.kind else {
            continue;
        };

        if input.is_empty() && handler_steps_use_input_scope(&handler.steps) {
            diagnostics.push(CheckDiagnostic {
                file: display_path(path),
                line: line_for_source_pattern(source, "input."),
                column: 1,
                severity: "error",
                code: "axonyx-route-input-missing",
                message:
                    "route uses `input.*` but has no `input:` section. Add typed route input fields or read request.form/request.json explicitly."
                        .to_string(),
            });
        }
    }

    diagnostics
}

fn check_backend_return_contracts(
    path: &Path,
    source: &str,
    plan: &AxBackendPlan,
    type_names: Option<&std::collections::BTreeSet<String>>,
) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();

    for handler in &plan.handlers {
        let returns = match &handler.kind {
            AxHandlerKind::Route { returns, .. }
            | AxHandlerKind::Loader { returns }
            | AxHandlerKind::Action { returns, .. } => returns,
            AxHandlerKind::Job => continue,
        };

        let Some(returns) = returns else {
            continue;
        };

        if !is_supported_backend_return_contract(returns) {
            diagnostics.push(CheckDiagnostic {
                file: display_path(path),
                line: line_for_source_pattern(source, &format!("-> {returns}")),
                column: 1,
                severity: "error",
                code: "axonyx-return-contract-type",
                message: format!(
                    "backend return contract `{returns}` is invalid. Use a named type such as Post, an array such as Post[], or a generic such as List<Post>."
                ),
            });
            continue;
        }

        let Some(type_names) = type_names else {
            continue;
        };

        for named_type in backend_return_contract_named_types(returns) {
            if type_names.contains(named_type) {
                continue;
            }

            diagnostics.push(CheckDiagnostic {
                file: display_path(path),
                line: line_for_source_pattern(source, &format!("-> {returns}")),
                column: 1,
                severity: "error",
                code: "axonyx-return-contract-unknown-type",
                message: format!(
                    "backend return contract `{returns}` references unknown type `{named_type}`. Add `type {named_type} {{ ... }}` to a parsed .ax file or use a built-in return type."
                ),
            });
        }
    }

    diagnostics
}

fn collect_project_type_names(root: &Path) -> Result<std::collections::BTreeSet<String>> {
    Ok(collect_project_type_schemas(root)?
        .into_iter()
        .map(|schema| schema.name)
        .collect())
}

fn collect_project_type_schemas(root: &Path) -> Result<Vec<ApiSchemaReport>> {
    let mut files = Vec::new();
    collect_ax_files(&root.join("app"), &mut files)?;
    collect_ax_files(&root.join("routes"), &mut files)?;
    collect_ax_files(&root.join("jobs"), &mut files)?;

    let mut schemas = std::collections::BTreeMap::new();
    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("failed to read .ax file '{}'", file.display()))?;
        let Ok(document) = parse_ax_v2(&source) else {
            continue;
        };

        for ty in document.types {
            schemas
                .entry(ty.name.clone())
                .or_insert_with(|| ApiSchemaReport {
                    name: ty.name,
                    fields: ty
                        .fields
                        .into_iter()
                        .map(|field| {
                            let (ty, optional) = normalize_api_schema_field_type(&field.ty);
                            ApiSchemaFieldReport {
                                name: field.name,
                                ty,
                                optional,
                            }
                        })
                        .collect(),
                });
        }
    }

    Ok(schemas.into_values().collect())
}

fn normalize_api_schema_field_type(ty: &str) -> (String, bool) {
    let ty = ty.trim();
    if let Some(inner) = ty
        .strip_prefix("Optional<")
        .and_then(|remaining| remaining.strip_suffix('>'))
    {
        return (inner.trim().to_string(), true);
    }

    (ty.to_string(), false)
}

fn is_supported_backend_return_contract(ty: &str) -> bool {
    let ty = ty.trim();
    if ty.is_empty() || ty.contains(char::is_whitespace) {
        return false;
    }

    if let Some(inner) = ty.strip_suffix("[]") {
        return is_supported_backend_return_contract(inner);
    }

    for wrapper in ["List", "Optional"] {
        let prefix = format!("{wrapper}<");
        if let Some(inner) = ty
            .strip_prefix(&prefix)
            .and_then(|remaining| remaining.strip_suffix('>'))
        {
            return !inner.is_empty()
                && !inner.contains(',')
                && is_supported_backend_return_contract(inner);
        }
    }

    matches!(
        ty,
        "String"
            | "Bool"
            | "Boolean"
            | "Number"
            | "Json"
            | "Null"
            | "string"
            | "bool"
            | "boolean"
            | "i64"
            | "u64"
            | "f64"
            | "int"
            | "integer"
            | "float"
            | "number"
    ) || is_ax_type_identifier(ty)
}

fn backend_return_contract_named_types(ty: &str) -> Vec<&str> {
    let ty = ty.trim();

    if let Some(inner) = ty.strip_suffix("[]") {
        return backend_return_contract_named_types(inner);
    }

    for wrapper in ["List", "Optional"] {
        let prefix = format!("{wrapper}<");
        if let Some(inner) = ty
            .strip_prefix(&prefix)
            .and_then(|remaining| remaining.strip_suffix('>'))
        {
            return backend_return_contract_named_types(inner);
        }
    }

    if is_builtin_backend_return_type(ty) {
        Vec::new()
    } else {
        vec![ty]
    }
}

fn is_builtin_backend_return_type(ty: &str) -> bool {
    matches!(
        ty,
        "String"
            | "Bool"
            | "Boolean"
            | "Number"
            | "Json"
            | "Null"
            | "string"
            | "bool"
            | "boolean"
            | "i64"
            | "u64"
            | "f64"
            | "int"
            | "integer"
            | "float"
            | "number"
    )
}

fn is_ax_type_identifier(ty: &str) -> bool {
    let mut chars = ty.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn is_supported_route_input_type(ty: &str) -> bool {
    matches!(
        ty.trim().to_ascii_lowercase().as_str(),
        "string"
            | "bool"
            | "boolean"
            | "i64"
            | "int"
            | "integer"
            | "u64"
            | "f64"
            | "float"
            | "number"
    )
}

fn handler_steps_use_input_scope(steps: &[AxStepPlan]) -> bool {
    steps.iter().any(|step| match step {
        AxStepPlan::Let {
            value: AxValuePlan::Expr(expr),
            ..
        } => expr_uses_input_scope(expr),
        AxStepPlan::Let {
            value: AxValuePlan::Query(query),
            ..
        } => query_uses_input_scope(query),
        AxStepPlan::Require { value, .. } => expr_uses_input_scope(value),
        AxStepPlan::Return(AxReturnPlan::Expr(expr) | AxReturnPlan::Json(expr)) => {
            expr_uses_input_scope(expr)
        }
        AxStepPlan::Patch { signal, value } => {
            expr_uses_input_scope(signal) || expr_uses_input_scope(value)
        }
        AxStepPlan::Header { name, value } | AxStepPlan::Cookie { name, value } => {
            expr_uses_input_scope(name) || expr_uses_input_scope(value)
        }
        AxStepPlan::Hook { value, .. } => expr_uses_input_scope(value),
        AxStepPlan::ClearCookie { name } => expr_uses_input_scope(name),
        AxStepPlan::Revalidate { target } => expr_uses_input_scope(target),
        AxStepPlan::Insert { fields, .. } => fields
            .iter()
            .any(|field| expr_uses_input_scope(&field.value)),
        AxStepPlan::Update {
            fields, filters, ..
        } => {
            fields
                .iter()
                .any(|field| expr_uses_input_scope(&field.value))
                || filters
                    .iter()
                    .any(|filter| expr_uses_input_scope(&filter.value))
        }
        AxStepPlan::Delete { filters, .. } => filters
            .iter()
            .any(|filter| expr_uses_input_scope(&filter.value)),
        AxStepPlan::Send { payload, .. } => expr_uses_input_scope(payload),
        AxStepPlan::Return(_) => false,
    })
}

fn expr_uses_input_scope(expr: &AxRustExpr) -> bool {
    expr.code == "input" || expr.code.starts_with("input.")
}

fn query_uses_input_scope(query: &axonyx_core::ax_backend_lowering_prelude::AxQueryPlan) -> bool {
    query
        .filters
        .iter()
        .any(|filter| expr_uses_input_scope(&filter.value))
}

fn backend_plan_uses_signed_session(plan: &AxBackendPlan) -> bool {
    plan.handlers.iter().any(|handler| {
        handler.steps.iter().any(|step| match step {
            AxStepPlan::Let {
                value: AxValuePlan::Expr(expr),
                ..
            }
            | AxStepPlan::Require { value: expr, .. }
            | AxStepPlan::Return(
                axonyx_core::ax_backend_lowering_prelude::AxReturnPlan::Expr(expr)
                | axonyx_core::ax_backend_lowering_prelude::AxReturnPlan::Json(expr),
            )
            | AxStepPlan::Patch { value: expr, .. }
            | AxStepPlan::Header { value: expr, .. }
            | AxStepPlan::Cookie { value: expr, .. }
            | AxStepPlan::Hook { value: expr, .. }
            | AxStepPlan::ClearCookie { name: expr }
            | AxStepPlan::Revalidate { target: expr } => expr.code.contains("Auth.signedSession"),
            AxStepPlan::Insert { fields, .. } | AxStepPlan::Update { fields, .. } => fields
                .iter()
                .any(|field| field.value.code.contains("Auth.signedSession")),
            AxStepPlan::Send { payload, .. } => payload.code.contains("Auth.signedSession"),
            AxStepPlan::Let { .. } | AxStepPlan::Delete { .. } | AxStepPlan::Return(_) => false,
        })
    })
}

fn auth_secret_configured(root: Option<&Path>, key: &str) -> bool {
    std::env::var_os(key).is_some()
        || root
            .map(|root| {
                env_file_defines_key(&root.join(".env"), key)
                    || env_file_defines_key(&root.join(".env.local"), key)
            })
            .unwrap_or(false)
}

fn env_file_defines_key(path: &Path, key: &str) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };

    source.lines().any(|line| {
        let line = line.trim();
        !line.is_empty()
            && !line.starts_with('#')
            && line
                .split_once('=')
                .map(|(name, value)| name.trim() == key && !value.trim().is_empty())
                .unwrap_or(false)
    })
}

fn line_for_source_pattern(source: &str, pattern: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(pattern))
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn line_for_repeated_source_pattern(source: &str, pattern: &str, occurrence: usize) -> usize {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(pattern))
        .nth(occurrence.saturating_sub(1))
        .map(|(index, _)| index + 1)
        .unwrap_or_else(|| line_for_source_pattern(source, pattern))
}

fn check_type_annotations(
    path: &Path,
    source: &str,
    document: &axonyx_core::ax_ast_prelude::AxDocument,
) -> Vec<CheckDiagnostic> {
    let Ok(file) = parse_ax_v2(source) else {
        return Vec::new();
    };
    if file.types.is_empty() && file.lets.iter().all(|binding| binding.ty.is_none()) {
        return Vec::new();
    }

    let context = match AxDataContext::from_v2_let_types(&file) {
        Ok(context) => context,
        Err(error) => {
            return vec![CheckDiagnostic {
                file: display_path(path),
                line: typed_let_line(source).unwrap_or(1),
                column: 1,
                severity: "error",
                code: "axonyx-type",
                message: error.to_string(),
            }];
        }
    };

    check_document_types(document, &context)
        .errors
        .into_iter()
        .map(|error| CheckDiagnostic {
            file: display_path(path),
            line: type_error_line(source, error.expression.as_deref())
                .or_else(|| typed_let_line(source))
                .unwrap_or(1),
            column: 1,
            severity: "error",
            code: "axonyx-type",
            message: match error.expression {
                Some(expression) => format!("`{expression}`: {}", error.message),
                None => format!("{}: {}", error.location, error.message),
            },
        })
        .collect()
}

fn type_error_line(source: &str, expression: Option<&str>) -> Option<usize> {
    let expression = expression?;
    source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(expression))
        .map(|(index, _)| index + 1)
}

fn typed_let_line(source: &str) -> Option<usize> {
    source
        .lines()
        .enumerate()
        .find(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with("let ") && trimmed.contains(':')
        })
        .map(|(index, _)| index + 1)
}

fn find_app_root_for_path(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;

    loop {
        if current.join("Axonyx.toml").exists() {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

fn check_imports(
    root: &Path,
    path: &Path,
    source: &str,
    imports: &[AxImport],
) -> Vec<CheckDiagnostic> {
    imports
        .iter()
        .filter_map(|import_decl| {
            let resolved = resolve_preview_import_path(root, &import_decl.source);
            let line = import_source_line(source, &import_decl.source);

            if let Some(import_path) = resolved.as_ref().filter(|path| path.exists()) {
                return validate_import_target(root, path, line, &import_decl.source, import_path);
            }

            let detail = resolved
                .as_ref()
                .map(|path| format!(" expected '{}'", display_path(path)))
                .unwrap_or_default();

            Some(CheckDiagnostic {
                file: display_path(path),
                line,
                column: 1,
                severity: "error",
                code: "axonyx-import",
                message: format!(
                    "unable to resolve import `{}`{}",
                    import_decl.source, detail
                ),
            })
        })
        .collect()
}

fn validate_import_target(
    root: &Path,
    importing_path: &Path,
    import_line: usize,
    import_source: &str,
    import_path: &Path,
) -> Option<CheckDiagnostic> {
    let mut stack = vec![canonical_path(importing_path)];
    let error = validate_import_path_recursive(root, import_path, &mut stack).err()?;

    let (code, message) = match error {
        ImportValidationError::Parse {
            path,
            line,
            message,
        } if same_path(&path, import_path) => (
            "axonyx-import-parse",
            format!(
                "import `{}` resolved to '{}' but that file is not valid .ax (line {}: {})",
                import_source,
                display_path(import_path),
                line,
                message
            ),
        ),
        ImportValidationError::Parse {
            path,
            line,
            message,
        } => (
            "axonyx-import-chain",
            format!(
                "import `{}` resolved to '{}' but its import chain is broken: '{}' is not valid .ax (line {}: {})",
                import_source,
                display_path(import_path),
                display_path(&path),
                line,
                message
            ),
        ),
        ImportValidationError::Missing {
            from_path,
            import_source: nested_source,
            expected,
        } => {
            let detail = expected
                .as_ref()
                .map(|path| format!(" expected '{}'", display_path(path)))
                .unwrap_or_default();
            (
                "axonyx-import-chain",
                format!(
                    "import `{}` resolved to '{}' but its import chain is broken: '{}' imports `{}`{}",
                    import_source,
                    display_path(import_path),
                    display_path(&from_path),
                    nested_source,
                    detail
                ),
            )
        }
        ImportValidationError::Cycle { chain } => (
            "axonyx-import-cycle",
            format!(
                "import `{}` resolved to '{}' but its import chain contains a cycle: {}",
                import_source,
                display_path(import_path),
                chain
                    .iter()
                    .map(|path| format!("'{}'", display_path(path)))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        ),
    };

    Some(CheckDiagnostic {
        file: display_path(importing_path),
        line: import_line,
        column: 1,
        severity: "error",
        code,
        message,
    })
}

#[derive(Debug)]
enum ImportValidationError {
    Missing {
        from_path: PathBuf,
        import_source: String,
        expected: Option<PathBuf>,
    },
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    Cycle {
        chain: Vec<PathBuf>,
    },
}

fn validate_import_path_recursive(
    root: &Path,
    path: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<(), ImportValidationError> {
    let canonical = canonical_path(path);
    if let Some(index) = stack.iter().position(|entry| same_path(entry, &canonical)) {
        let mut chain = stack[index..].to_vec();
        chain.push(canonical);
        return Err(ImportValidationError::Cycle { chain });
    }

    let source = fs::read_to_string(path).map_err(|_| ImportValidationError::Missing {
        from_path: path.to_path_buf(),
        import_source: String::new(),
        expected: Some(path.to_path_buf()),
    })?;
    let document = parse_ax_auto(&source).map_err(|error| ImportValidationError::Parse {
        path: path.to_path_buf(),
        line: line_from_auto_parse_error(&error).unwrap_or(1),
        message: message_from_auto_parse_error(&error),
    })?;

    stack.push(canonical);

    for import_decl in document.imports {
        let resolved = resolve_preview_import_path(root, &import_decl.source);
        let Some(import_path) = resolved.as_ref().filter(|path| path.exists()) else {
            stack.pop();
            return Err(ImportValidationError::Missing {
                from_path: path.to_path_buf(),
                import_source: import_decl.source,
                expected: resolved,
            });
        };

        if let Err(error) = validate_import_path_recursive(root, import_path, stack) {
            stack.pop();
            return Err(error);
        }
    }

    stack.pop();
    Ok(())
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn same_path(left: &Path, right: &Path) -> bool {
    canonical_path(left) == canonical_path(right)
}

fn import_source_line(source: &str, import_source: &str) -> usize {
    let double_quoted = format!("\"{import_source}\"");
    let single_quoted = format!("'{import_source}'");

    source
        .lines()
        .position(|line| line.contains(&double_quoted) || line.contains(&single_quoted))
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn looks_like_backend_ax(source: &str) -> bool {
    source.lines().map(str::trim_start).any(|line| {
        line.starts_with("route ")
            || line.starts_with("loader ")
            || line.starts_with("action ")
            || line.starts_with("job ")
    })
}

enum CheckParseError {
    Page(AxAutoParseError),
    Backend(AxBackendParseError),
}

fn diagnostic_from_parse_error(path: &Path, error: CheckParseError) -> CheckDiagnostic {
    let (line, code, message) = match error {
        CheckParseError::Page(error) => (
            line_from_auto_parse_error(&error).unwrap_or(1),
            "axonyx-parse",
            message_from_auto_parse_error(&error),
        ),
        CheckParseError::Backend(error) => (
            line_from_backend_parse_error(&error).unwrap_or(1),
            "axonyx-backend-parse",
            error.to_string(),
        ),
    };

    CheckDiagnostic {
        file: display_path(path),
        line,
        column: 1,
        severity: "error",
        code,
        message,
    }
}

fn message_from_auto_parse_error(error: &AxAutoParseError) -> String {
    match error {
        AxAutoParseError::V1(error) => error.to_string(),
        AxAutoParseError::V2(error) => error.to_string(),
        AxAutoParseError::Semantic(error) => error.to_string(),
        AxAutoParseError::Convert(error) => error.to_string(),
    }
}

fn line_from_auto_parse_error(error: &AxAutoParseError) -> Option<usize> {
    match error {
        AxAutoParseError::V1(error) => line_from_ax_parse_error(error),
        AxAutoParseError::V2(error) => line_from_ax_parse_v2_error(error),
        AxAutoParseError::Semantic(error) => line_from_semantic_error(error),
        AxAutoParseError::Convert(error) => line_from_convert_error(error),
    }
}

fn line_from_ax_parse_error(error: &AxParseError) -> Option<usize> {
    match error {
        AxParseError::EmptyDocument => Some(1),
        AxParseError::TabsNotSupported { line }
        | AxParseError::InvalidIndentation { line }
        | AxParseError::InvalidPage { line }
        | AxParseError::UnexpectedIndentation { line }
        | AxParseError::InvalidDataBinding { line }
        | AxParseError::InvalidEach { line }
        | AxParseError::InvalidPipelineStage { line }
        | AxParseError::InvalidComponent { line }
        | AxParseError::InvalidTitle { line }
        | AxParseError::InvalidTheme { line }
        | AxParseError::InvalidHeadTag { line, .. }
        | AxParseError::InvalidExpression { line, .. } => Some(*line),
    }
}

fn line_from_ax_parse_v2_error(error: &AxParseV2Error) -> Option<usize> {
    match error {
        AxParseV2Error::EmptyDocument | AxParseV2Error::MissingPage => Some(1),
        AxParseV2Error::InvalidImport { line }
        | AxParseV2Error::InvalidUse { line }
        | AxParseV2Error::MissingImportFrom { line }
        | AxParseV2Error::EmptyImportList { line }
        | AxParseV2Error::InvalidPage { line }
        | AxParseV2Error::InvalidType { line }
        | AxParseV2Error::InvalidLet { line }
        | AxParseV2Error::InvalidState { line }
        | AxParseV2Error::InvalidFunction { line }
        | AxParseV2Error::InvalidComponent { line }
        | AxParseV2Error::DuplicatePage { line }
        | AxParseV2Error::InvalidTag { line }
        | AxParseV2Error::UnterminatedTag { line }
        | AxParseV2Error::UnterminatedString { line }
        | AxParseV2Error::UnterminatedExpression { line }
        | AxParseV2Error::UnexpectedClosingTag { line, .. }
        | AxParseV2Error::MismatchedClosingTag { line, .. }
        | AxParseV2Error::MissingAttributeValue { line, .. } => Some(*line),
    }
}

fn line_from_convert_error(error: &AxConvertV2Error) -> Option<usize> {
    match error {
        AxConvertV2Error::InvalidExpression { error, .. } => line_from_ax_parse_error(error),
        AxConvertV2Error::MissingControlAttr { .. }
        | AxConvertV2Error::InvalidBindingAttr { .. }
        | AxConvertV2Error::ControlBranchAttrsNotSupported { .. }
        | AxConvertV2Error::DuplicateControlBranch { .. }
        | AxConvertV2Error::ControlBranchMustBeLast { .. }
        | AxConvertV2Error::UnexpectedControlBranch { .. }
        | AxConvertV2Error::InvalidHeadChild
        | AxConvertV2Error::UnsupportedHeadTag { .. }
        | AxConvertV2Error::HeadValueAttrsNotSupported { .. }
        | AxConvertV2Error::HeadValueRequiresSingleChild { .. }
        | AxConvertV2Error::HeadValueInvalidChild { .. }
        | AxConvertV2Error::HeadTagChildrenNotSupported { .. }
        | AxConvertV2Error::InvalidStateInitializer { .. }
        | AxConvertV2Error::UnknownStateBinding { .. }
        | AxConvertV2Error::InvalidStateBinding { .. } => Some(1),
    }
}

fn line_from_semantic_error(error: &AxSemanticV2Error) -> Option<usize> {
    match error {
        AxSemanticV2Error::ReservedImportName { .. }
        | AxSemanticV2Error::ReservedComponentName { .. }
        | AxSemanticV2Error::DuplicateComponentName { .. }
        | AxSemanticV2Error::ComponentNameConflictsWithImport { .. }
        | AxSemanticV2Error::HeadTagOutsideHead { .. }
        | AxSemanticV2Error::HeadOutsideTopLevel => Some(1),
    }
}

fn line_from_backend_parse_error(error: &AxBackendParseError) -> Option<usize> {
    match error {
        AxBackendParseError::EmptyDocument => Some(1),
        AxBackendParseError::TabsNotSupported { line }
        | AxBackendParseError::InvalidIndentation { line }
        | AxBackendParseError::UnexpectedIndentation { line }
        | AxBackendParseError::InvalidBlock { line }
        | AxBackendParseError::InvalidDataBinding { line }
        | AxBackendParseError::InvalidInputSection { line }
        | AxBackendParseError::InvalidField { line }
        | AxBackendParseError::InvalidMutation { line }
        | AxBackendParseError::InvalidAssignment { line }
        | AxBackendParseError::InvalidHeader { line }
        | AxBackendParseError::InvalidCookie { line }
        | AxBackendParseError::InvalidHook { line }
        | AxBackendParseError::InvalidRequirement { line }
        | AxBackendParseError::InvalidReturn { line }
        | AxBackendParseError::InvalidSend { line }
        | AxBackendParseError::InvalidQuerySource { line }
        | AxBackendParseError::InvalidQueryClause { line }
        | AxBackendParseError::InvalidQueryNumber { line }
        | AxBackendParseError::InvalidExpression { line, .. } => Some(*line),
    }
}

fn print_check_text(diagnostics: &[CheckDiagnostic]) {
    if diagnostics.is_empty() {
        println!("Axonyx check passed.");
        return;
    }

    for diagnostic in diagnostics {
        println!("{}", format_check_diagnostic(diagnostic));
    }
}

fn format_check_diagnostic(diagnostic: &CheckDiagnostic) -> String {
    format!(
        "{}:{}:{}: {}[{}]: {}",
        diagnostic.file,
        diagnostic.line,
        diagnostic.column,
        diagnostic.severity,
        diagnostic.code,
        diagnostic.message
    )
}

fn print_cli_error(error: &anyhow::Error) {
    let message = error.to_string();

    eprintln!("Axonyx could not finish this command.");
    eprintln!();
    eprintln!("Problem:");
    eprintln!("  {}", translate_error_message(&message));

    if let Some(hint) = hint_for_error(error) {
        eprintln!();
        eprintln!("Hint:");
        eprintln!("  {hint}");
    }

    let mut chain = error.chain();
    let _ = chain.next();
    let details = chain.map(ToString::to_string).collect::<Vec<_>>();
    if !details.is_empty() {
        eprintln!();
        eprintln!("Details:");
        for detail in details {
            eprintln!("  - {}", translate_error_message(&detail));
        }
    }
}

fn translate_error_message(message: &str) -> String {
    message
        .replace("preview", "Axonyx")
        .replace("AxPreview", "Axonyx")
}

fn hint_for_error(error: &anyhow::Error) -> Option<&'static str> {
    let combined = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    if combined.contains("Axonyx.toml was not found") {
        return Some("Run the command from an Axonyx app root, or create one with create-axonyx.");
    }

    if combined.contains("Axonyx diagnostics failed") {
        return Some(
            "Run `cargo ax check` to see the same file-level diagnostics before building or starting production.",
        );
    }

    if combined.contains("unable to resolve import") || combined.contains("failed to import") {
        return Some(
            "Check the import path, package_overrides, and whether the target .ax file exists.",
        );
    }

    if combined.contains("[prerender]")
        || combined.contains("prerender route")
        || combined.contains("missing prerender param")
    {
        return Some(
            "Check the [prerender] routes in Axonyx.toml. Dynamic params must be strings.",
        );
    }

    if combined.contains("--clean refuses") {
        return Some("Choose an output directory inside the app root, for example `cargo ax build --out-dir dist --clean`.");
    }

    if combined.contains("failed to render route") {
        return Some("Run `cargo ax check`, then inspect the route's page.ax, layout.ax, loader.ax, and imports.");
    }

    None
}

fn display_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn app_root() -> Result<PathBuf> {
    let root = std::env::current_dir().context("unable to resolve current directory")?;
    let axonyx_toml = root.join("Axonyx.toml");

    if !axonyx_toml.exists() {
        bail!(
            "Axonyx.toml was not found in '{}'; run this command from an Axonyx app root",
            root.display()
        );
    }

    Ok(root)
}

fn add_module(module: ModuleKind) -> Result<()> {
    let root = app_root()?;
    let axonyx_toml = root.join("Axonyx.toml");

    match module {
        ModuleKind::Docs => {
            scaffold_docs_module(&root)?;
            enable_module(&axonyx_toml, "docs")?;
            println!("Added docs module.");
        }
        ModuleKind::Ui => {
            add_ui_module(&root, &axonyx_toml)?;
            println!("Added ui module.");
        }
        ModuleKind::Cms | ModuleKind::Blockbit => add_reserved_cms_module()?,
    }

    Ok(())
}

fn add_reserved_cms_module() -> Result<()> {
    bail!(
        "Blockbit CMS is a future Axonyx module, not part of framework core yet. \
         For now, build with Axonyx primitives: routes, loaders, actions, state patches, content collections, and UI."
    )
}

fn run_dev_server(args: DevArgs) -> Result<()> {
    run_http_server(args, AxServerMode::Dev, false)
}

fn run_start_server(args: DevArgs) -> Result<()> {
    run_http_server(args, AxServerMode::Start, false)
}

fn run_stream_server(args: DevArgs) -> Result<()> {
    run_http_server(args, AxServerMode::Dev, true)
}

fn run_http_server(args: DevArgs, mode: AxServerMode, stream_probe: bool) -> Result<()> {
    let root = app_root()?;
    if mode == AxServerMode::Start {
        ensure_no_check_diagnostics_for(&root, "production start")?;
    }

    let backend_status = compile_backend_from_app_root(&root)?;
    let runtime_config = AxServerRuntimeConfig::from_root(&root).map_err(anyhow::Error::msg)?;
    let env_port = std::env::var("PORT").ok();
    let port = resolve_server_port(mode, args.port, env_port.as_deref())?;
    let uses_env_port = mode == AxServerMode::Start
        && args.port.is_none()
        && env_port
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

    let production_server = args.production_server;
    let transport = args.effective_transport();
    let server_config = AxServerConfig::new(args.host, port, mode);
    let bind = server_config.bind_addr();
    let preview_store = preview_store_from_content(&root)?;
    let shared_state = Arc::new(DevServerState {
        root,
        preview_store: Mutex::new(preview_store),
        runtime_config,
    });

    print_backend_build_status(&backend_status);
    println!(
        "Axonyx {} server listening at http://{bind} using {} transport",
        mode.label(),
        transport.label()
    );
    if uses_env_port {
        println!("Using PORT environment variable for hosted production start.");
    }
    if production_server {
        println!("Production server preview is enabled.");
    }
    if transport == ServerTransport::Tokio {
        println!("Tokio graceful shutdown is enabled for Ctrl+C.");
        println!(
            "Shutdown grace period: {} seconds.",
            runtime_config.shutdown_grace.as_secs()
        );
        println!("Tokio max connections: {}.", runtime_config.max_connections);
    }
    println!(
        "Routes come from app/**/page.ax with nested layouts, route-local loader.ax, actions.ax POST handling, and routes/**/*.ax API endpoints."
    );
    println!(
        "Request body limit: {}",
        format_bytes(runtime_config.max_body_bytes)
    );
    println!(
        "Request read timeout: {} second{}",
        runtime_config.request_timeout.as_secs(),
        if runtime_config.request_timeout.as_secs() == 1 {
            ""
        } else {
            "s"
        }
    );
    println!(
        "Compression: {}.",
        if runtime_config.compression {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "Security headers: {}.",
        if runtime_config.security_headers {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "Request logging: {} ({}) to stdout.",
        if runtime_config.request_logging {
            "enabled"
        } else {
            "disabled"
        },
        runtime_config.log_format.label()
    );
    if mode == AxServerMode::Dev {
        println!("Live reload polling is enabled.");
    }
    if stream_probe {
        println!("Streaming probe: http://{bind}/__axonyx/stream");
    }
    println!("Press Ctrl+C to stop.");

    match transport {
        ServerTransport::Std => {
            let server = StdNetAxServer {
                config: server_config,
                state: shared_state,
            };
            server.serve().map_err(|error| anyhow::anyhow!("{error}"))
        }
        ServerTransport::Tokio => {
            let server = TokioAxServer {
                config: server_config,
                state: shared_state,
            };
            server.serve().map_err(|error| anyhow::anyhow!("{error}"))
        }
    }
}

fn resolve_server_port(
    mode: AxServerMode,
    cli_port: Option<u16>,
    env_port: Option<&str>,
) -> Result<u16> {
    if let Some(port) = cli_port {
        return Ok(port);
    }

    if mode == AxServerMode::Start {
        if let Some(port) = env_port.filter(|value| !value.trim().is_empty()) {
            return port
                .trim()
                .parse::<u16>()
                .with_context(|| format!("invalid PORT environment value '{port}'"));
        }
    }

    Ok(3000)
}

fn compile_backend_from_app_root(root: &Path) -> Result<BackendBuildStatus> {
    let mut sources = Vec::new();
    collect_backend_sources(root, &mut sources)?;

    let output_path = root.join("src").join("generated").join("backend.rs");

    if sources.is_empty() {
        return Ok(BackendBuildStatus::NoSources { output_path });
    }

    let source_refs = sources
        .iter()
        .map(|(name, source)| (name.as_str(), source.as_str()))
        .collect::<Vec<_>>();

    let module = compile_backend_sources_to_module(&source_refs)
        .with_context(|| "failed to compile backend .ax sources")?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create generated backend directory '{}'",
                parent.display()
            )
        })?;
    }

    fs::write(&output_path, module).with_context(|| {
        format!(
            "failed to write generated backend module '{}'",
            output_path.display()
        )
    })?;

    Ok(BackendBuildStatus::Generated {
        source_count: source_refs.len(),
        output_path,
    })
}

fn print_backend_build_status(status: &BackendBuildStatus) {
    match status {
        BackendBuildStatus::Generated {
            source_count,
            output_path,
        } => {
            println!(
                "Generated backend from {source_count} .ax source(s) into {}",
                output_path.display()
            );
        }
        BackendBuildStatus::NoSources { output_path } => {
            println!(
                "No backend .ax sources found; leaving generated backend at {}",
                output_path.display()
            );
        }
    }
}

fn build_static_site_from_app_root(
    root: &Path,
    out_dir: &Path,
    clean: bool,
) -> Result<StaticBuildStatus> {
    let output_dir = resolve_output_dir(root, out_dir);
    let routes = collect_page_route_manifest(root)?;
    let static_routes = routes
        .iter()
        .filter(|route| route.params.is_empty())
        .collect::<Vec<_>>();
    let dynamic_routes = routes
        .iter()
        .filter(|route| !route.params.is_empty())
        .collect::<Vec<_>>();
    let prerender_routes = load_prerender_routes(root)?;

    if clean {
        clean_output_dir(root, &output_dir)?;
    }

    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create '{}'", output_dir.display()))?;

    copy_public_assets_to_dist(root, &output_dir)?;
    copy_package_assets_to_dist(root, &output_dir)?;
    let content_collection_count = write_content_manifest_to_dist(root, &output_dir)?;
    let melt_graph_written = write_melt_graph_to_dist(root, &output_dir)?;

    if static_routes.is_empty() && prerender_routes.is_empty() {
        return Ok(StaticBuildStatus::NoPages {
            skipped_dynamic_count: dynamic_routes.len(),
            content_collection_count,
            melt_graph_written,
            output_dir,
        });
    }

    let state = DevServerState {
        root: root.to_path_buf(),
        preview_store: Mutex::new(preview_store_from_content(root)?),
        runtime_config: AxServerRuntimeConfig::from_root(root).map_err(anyhow::Error::msg)?,
    };

    for route in &static_routes {
        let resolved = resolve_route(root, &route.route)?
            .ok_or_else(|| anyhow::anyhow!("failed to resolve route '{}'", route.route))?;
        let html = render_route_html(&state, &resolved)?;
        let output_path = static_route_output_path(&output_dir, &route.route)?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create '{}'", parent.display()))?;
        }

        fs::write(&output_path, html)
            .with_context(|| format!("failed to write '{}'", output_path.display()))?;
    }

    let prerendered_count = build_prerendered_routes(
        root,
        &output_dir,
        &state,
        &dynamic_routes,
        &prerender_routes,
    )?;
    let skipped_dynamic_count = dynamic_routes.len().saturating_sub(
        prerender_routes
            .iter()
            .filter(|entry| {
                dynamic_routes
                    .iter()
                    .any(|route| route.route == entry.route)
            })
            .count(),
    );

    Ok(StaticBuildStatus::Generated {
        route_count: static_routes.len(),
        prerendered_count,
        skipped_dynamic_count,
        content_collection_count,
        melt_graph_written,
        output_dir,
    })
}

fn clean_output_dir(root: &Path, output_dir: &Path) -> Result<()> {
    if !output_dir.exists() {
        return Ok(());
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve app root '{}'", root.display()))?;
    let output_dir = output_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve output dir '{}'", output_dir.display()))?;

    if output_dir == root || !output_dir.starts_with(&root) {
        bail!(
            "--clean refuses to remove '{}' because it is not a child of app root '{}'",
            output_dir.display(),
            root.display()
        );
    }

    fs::remove_dir_all(&output_dir)
        .with_context(|| format!("failed to clean '{}'", output_dir.display()))?;
    Ok(())
}

fn build_prerendered_routes(
    root: &Path,
    output_dir: &Path,
    state: &DevServerState,
    dynamic_routes: &[&RouteManifestItem],
    prerender_routes: &[PrerenderRoute],
) -> Result<usize> {
    let mut count = 0;

    for entry in prerender_routes {
        let Some(route) = dynamic_routes
            .iter()
            .find(|route| route.route == entry.route)
        else {
            bail!(
                "prerender route '{}' does not match a dynamic page route",
                entry.route
            );
        };

        for params in &entry.params {
            let concrete_route = concrete_route_from_params(&route.route, params)?;
            let resolved = resolve_route(root, &concrete_route)?.ok_or_else(|| {
                anyhow::anyhow!("failed to resolve prerender route '{}'", concrete_route)
            })?;
            let html = render_route_html(state, &resolved)?;
            let output_path = static_route_output_path(output_dir, &concrete_route)?;

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create '{}'", parent.display()))?;
            }

            fs::write(&output_path, html)
                .with_context(|| format!("failed to write '{}'", output_path.display()))?;
            count += 1;
        }
    }

    Ok(count)
}

fn concrete_route_from_params(
    route: &str,
    params: &std::collections::BTreeMap<String, String>,
) -> Result<String> {
    let mut segments = Vec::new();

    for segment in route.trim_start_matches('/').split('/') {
        if let Some(name) = segment.strip_prefix(':').filter(|name| !name.is_empty()) {
            let value = params
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("missing prerender param '{name}' for '{route}'"))?;
            let value = value.trim_matches('/');
            if value.is_empty() || value.contains('/') || value == "." || value == ".." {
                bail!("invalid prerender param '{name}' value '{value}' for '{route}'");
            }
            segments.push(value.to_string());
        } else if !segment.is_empty() {
            segments.push(segment.to_string());
        }
    }

    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

fn load_prerender_routes(root: &Path) -> Result<Vec<PrerenderRoute>> {
    let mut out = load_explicit_prerender_routes(root)?;
    out.extend(load_content_prerender_routes(root)?);
    Ok(out)
}

fn load_explicit_prerender_routes(root: &Path) -> Result<Vec<PrerenderRoute>> {
    let Some(routes_value) = axonyx_config_value(root, "prerender", "routes") else {
        return Ok(Vec::new());
    };

    let routes = routes_value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("[prerender].routes must be an array"))?;
    let mut out = Vec::new();

    for route_value in routes {
        let table = route_value
            .as_table()
            .ok_or_else(|| anyhow::anyhow!("each [prerender].routes item must be a table"))?;
        let route = table
            .get("route")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("prerender route item is missing route"))?
            .trim()
            .to_string();
        let params_value = table
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("prerender route '{route}' is missing params"))?;
        let params_array = params_value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("prerender route '{route}' params must be an array"))?;
        let mut params = Vec::new();

        for params_value in params_array {
            let params_table = params_value.as_table().ok_or_else(|| {
                anyhow::anyhow!("prerender route '{route}' params entries must be tables")
            })?;
            let mut params_map = std::collections::BTreeMap::new();

            for (name, value) in params_table {
                let value = value.as_str().ok_or_else(|| {
                    anyhow::anyhow!("prerender route '{route}' param '{name}' must be a string")
                })?;
                params_map.insert(name.to_string(), value.to_string());
            }

            params.push(params_map);
        }

        out.push(PrerenderRoute { route, params });
    }

    Ok(out)
}

fn load_content_prerender_routes(root: &Path) -> Result<Vec<PrerenderRoute>> {
    let Some(collections_value) = axonyx_config_value(root, "prerender", "collections") else {
        return Ok(Vec::new());
    };

    let collections = collections_value
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("[prerender].collections must be a TOML table"))?;
    let content = collect_content_manifest(root)?;
    let mut out = Vec::new();

    for (collection_name, value) in collections {
        let table = value.as_table().ok_or_else(|| {
            anyhow::anyhow!("[prerender.collections.{collection_name}] must be a TOML table")
        })?;
        let route = table
            .get("route")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("[prerender.collections.{collection_name}] is missing route")
            })?
            .trim()
            .to_string();
        let param = table
            .get("param")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("slug")
            .trim()
            .to_string();
        let field = table
            .get("field")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&param)
            .trim()
            .to_string();
        let collection = content
            .collections
            .iter()
            .find(|collection| collection.name == *collection_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "prerender collection '{collection_name}' does not match a configured content collection"
                )
            })?;
        let mut params = Vec::new();

        for entry in &collection.entries {
            let Some(value) = content_entry_field(entry, &field) else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            params.push(std::collections::BTreeMap::from([(
                param.clone(),
                value.to_string(),
            )]));
        }

        out.push(PrerenderRoute { route, params });
    }

    Ok(out)
}

fn content_entry_field<'a>(entry: &'a ContentEntryManifest, field: &str) -> Option<&'a str> {
    match field {
        "path" => Some(&entry.path),
        "slug" => Some(&entry.slug),
        "extension" => Some(&entry.extension),
        "content_type" => Some(&entry.content_type),
        "title" => Some(&entry.title),
        "excerpt" => Some(&entry.excerpt),
        "body" => Some(&entry.body),
        "html" => Some(&entry.html),
        other => entry.frontmatter.get(other).map(String::as_str),
    }
}

fn resolve_output_dir(root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        root.join(out_dir)
    }
}

fn copy_public_assets_to_dist(root: &Path, output_dir: &Path) -> Result<()> {
    let public_dir = root.join("public");
    if !public_dir.exists() {
        return Ok(());
    }

    copy_dir_all_filtered(&public_dir, output_dir, |_| false)
}

fn copy_package_assets_to_dist(root: &Path, output_dir: &Path) -> Result<()> {
    let Some(package_root) = resolve_package_asset_root(root, AXONYX_UI_PACKAGE_NAME) else {
        return Ok(());
    };

    let target = output_dir
        .join("_ax")
        .join("pkg")
        .join(AXONYX_UI_PACKAGE_NAME);

    let css_root = package_css_root(&package_root);
    if css_root.exists() {
        copy_dir_all_filtered(&css_root, &target, |_| false)?;
    }

    let js_root = package_js_root(&package_root);
    if js_root.exists() {
        copy_dir_all_filtered(&js_root, &target.join("js"), |_| false)?;
    }

    copy_hashed_package_entry(&package_css_entry(&package_root), &target)?;
    copy_hashed_package_entry(&package_js_entry(&package_root), &target.join("js"))?;

    Ok(())
}

fn copy_hashed_package_entry(entry: &Path, target_dir: &Path) -> Result<()> {
    if !entry.exists() || !entry.is_file() {
        return Ok(());
    }

    let Some(file_name) = hashed_asset_file_name(entry)? else {
        return Ok(());
    };

    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create '{}'", target_dir.display()))?;
    fs::copy(entry, target_dir.join(file_name))
        .with_context(|| format!("failed to copy hashed package asset '{}'", entry.display()))?;

    Ok(())
}

fn write_content_manifest_to_dist(root: &Path, output_dir: &Path) -> Result<usize> {
    let manifest = collect_content_manifest(root)?;
    let count = manifest.collections.len();
    if count == 0 {
        return Ok(0);
    }

    let target = output_dir.join("_ax").join("content").join("manifest.json");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&manifest)
        .context("failed to render content manifest as JSON")?;
    fs::write(&target, json)
        .with_context(|| format!("failed to write content manifest to '{}'", target.display()))?;

    Ok(count)
}

fn write_melt_graph_to_dist(root: &Path, output_dir: &Path) -> Result<bool> {
    let report = collect_melt_report(root)?;
    let target = output_dir.join("_ax").join("melt").join("graph.json");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    let json =
        serde_json::to_string_pretty(&report).context("failed to render Melt graph as JSON")?;
    fs::write(&target, json)
        .with_context(|| format!("failed to write Melt graph to '{}'", target.display()))?;

    Ok(true)
}

fn static_route_output_path(output_dir: &Path, route: &str) -> Result<PathBuf> {
    let normalized = normalize_request_path(route)?;
    let segments = path_segments(&normalized);

    if segments.is_empty() {
        return Ok(output_dir.join("index.html"));
    }

    Ok(segments
        .iter()
        .fold(output_dir.to_path_buf(), |current, segment| {
            current.join(segment)
        })
        .join("index.html"))
}

fn print_static_build_status(status: &StaticBuildStatus) {
    match status {
        StaticBuildStatus::Generated {
            route_count,
            prerendered_count,
            skipped_dynamic_count,
            content_collection_count,
            melt_graph_written,
            output_dir,
        } => {
            println!(
                "Generated static site from {route_count} page route(s) into {}",
                output_dir.display()
            );
            if *prerendered_count > 0 {
                println!("Prerendered {prerendered_count} dynamic page variant(s).");
            }
            if *skipped_dynamic_count > 0 {
                println!(
                    "Skipped {skipped_dynamic_count} dynamic page route(s); provide params through a future prerender config."
                );
            }
            if *content_collection_count > 0 {
                println!(
                    "Wrote content manifest for {content_collection_count} collection(s) into {}/_ax/content/manifest.json",
                    output_dir.display()
                );
            }
            if *melt_graph_written {
                println!(
                    "Wrote Melt graph into {}/_ax/melt/graph.json",
                    output_dir.display()
                );
            }
        }
        StaticBuildStatus::NoPages {
            skipped_dynamic_count,
            content_collection_count,
            melt_graph_written,
            output_dir,
        } => {
            println!(
                "No static page routes found; copied public assets into {}",
                output_dir.display()
            );
            if *skipped_dynamic_count > 0 {
                println!(
                    "Skipped {skipped_dynamic_count} dynamic page route(s); provide params through a future prerender config."
                );
            }
            if *content_collection_count > 0 {
                println!(
                    "Wrote content manifest for {content_collection_count} collection(s) into {}/_ax/content/manifest.json",
                    output_dir.display()
                );
            }
            if *melt_graph_written {
                println!(
                    "Wrote Melt graph into {}/_ax/melt/graph.json",
                    output_dir.display()
                );
            }
        }
    }
}

fn collect_app_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let mut routes = collect_page_route_manifest(root)?;
    routes.extend(collect_backend_route_manifest(root)?);
    routes.sort_by(|left, right| {
        left.route
            .cmp(&right.route)
            .then_with(|| left.kind.cmp(right.kind))
            .then_with(|| left.method.cmp(&right.method))
    });
    Ok(routes)
}

fn collect_page_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let app_root = root.join("app");
    if !app_root.exists() {
        return Ok(Vec::new());
    }

    let mut routes = Vec::new();
    collect_app_route_manifest_from(root, &app_root, &app_root, &mut routes)?;
    Ok(routes)
}

fn collect_app_route_manifest_from(
    root: &Path,
    app_root: &Path,
    dir: &Path,
    out: &mut Vec<RouteManifestItem>,
) -> Result<()> {
    let page_path = dir.join("page.ax");
    if page_path.exists() {
        out.push(app_route_manifest_item(root, app_root, dir, &page_path)?);
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read '{}'", dir.display()))? {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_app_route_manifest_from(root, app_root, &path, out)?;
        }
    }

    Ok(())
}

fn app_route_manifest_item(
    root: &Path,
    app_root: &Path,
    page_dir: &Path,
    page_path: &Path,
) -> Result<RouteManifestItem> {
    let relative_dir = page_dir.strip_prefix(app_root).unwrap_or(page_dir);
    let segments = relative_dir
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let route = route_pattern_from_segments(&segments);
    let params = segments
        .iter()
        .filter_map(|segment| parse_dynamic_app_segment(segment).map(str::to_string))
        .collect::<Vec<_>>();

    let mut layouts = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layouts.push(display_relative_path(root, &root_layout));
    }

    let mut current = app_root.to_path_buf();
    for segment in &segments {
        current = current.join(segment);
        let layout_path = current.join("layout.ax");
        if layout_path.exists() {
            layouts.push(display_relative_path(root, &layout_path));
        }
    }

    let loader_path = page_dir.join("loader.ax");
    let actions_path = page_dir.join("actions.ax");

    Ok(RouteManifestItem {
        kind: "page",
        route,
        method: None,
        returns: None,
        file: display_relative_path(root, page_path),
        layouts,
        loader: loader_path
            .exists()
            .then(|| display_relative_path(root, &loader_path)),
        actions: actions_path
            .exists()
            .then(|| display_relative_path(root, &actions_path)),
        params,
        inputs: Vec::new(),
        hooks: Vec::new(),
    })
}

fn collect_backend_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let routes_root = root.join("routes");
    if !routes_root.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    collect_backend_sources_in_dir(&routes_root, &routes_root, &mut sources, true)?;

    let mut routes = Vec::new();
    for (relative_path, source) in sources {
        let document = parse_backend_ax(&source).with_context(|| {
            format!(
                "failed to parse backend route source '{}'",
                routes_root.join(&relative_path).display()
            )
        })?;

        for block in document.blocks {
            let AxBackendBlock::Route(route) = block else {
                continue;
            };

            let hooks = route_hooks_from_body(&route.body);
            routes.push(RouteManifestItem {
                kind: "api",
                route: route.path.clone(),
                method: Some(route.method),
                returns: route.returns,
                file: format!("routes/{relative_path}"),
                layouts: Vec::new(),
                loader: None,
                actions: None,
                params: route_params_from_pattern(&route.path),
                inputs: route
                    .input
                    .into_iter()
                    .map(|field| ActionInputReport {
                        name: field.name,
                        ty: field.ty,
                        optional: field.optional,
                        default: field.default.as_ref().map(format_ax_expr),
                    })
                    .collect(),
                hooks,
            });
        }
    }

    Ok(routes)
}

fn route_hooks_from_body(body: &[AxBackendStmt]) -> Vec<RouteHookReport> {
    body.iter()
        .filter_map(|stmt| {
            let AxBackendStmt::Hook(hook) = stmt else {
                return None;
            };
            let phase = match hook.phase {
                AxHookPhase::Before => "before",
                AxHookPhase::After => "after",
            };
            Some(RouteHookReport {
                phase,
                value: format_ax_expr(&hook.value),
            })
        })
        .collect()
}

fn route_pattern_from_segments(segments: &[&str]) -> String {
    if segments.is_empty() {
        return "/".to_string();
    }

    let route = segments
        .iter()
        .map(|segment| {
            parse_dynamic_app_segment(segment)
                .map(|param| format!(":{param}"))
                .unwrap_or_else(|| (*segment).to_string())
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("/{route}")
}

fn route_params_from_pattern(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter_map(|segment| {
            segment
                .strip_prefix(':')
                .filter(|name| !name.is_empty())
                .or_else(|| parse_dynamic_app_segment(segment))
                .map(str::to_string)
        })
        .collect()
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    display_path(path.strip_prefix(root).unwrap_or(path))
}

fn print_melt_text(report: &MeltReport) {
    println!("Axonyx Melt");
    println!("  app={} root={}", report.app.name, report.app.root);
    println!(
        "  pages={} api={} action_routes={} actions={} state_signals={} content_collections={} content_entries={} diagnostics={}",
        report.summary.page_routes,
        report.summary.api_routes,
        report.summary.action_routes,
        report.summary.actions,
        report.summary.state_signals,
        report.summary.content_collections,
        report.summary.content_entries,
        report.summary.diagnostics
    );

    println!();
    println!("Framework layers:");
    for layer in &report.layers {
        println!("  {:<16} {:<9} {}", layer.name, layer.status, layer.detail);
    }

    println!();
    print_routes_text(&report.routes);

    if !report.content.collections.is_empty() {
        println!();
        print_content_text(&report.content);
    }

    if !report.state.files.is_empty() {
        println!();
        print_state_text(&report.state);
    }

    if !report.diagnostics.is_empty() {
        println!();
        println!("Diagnostics:");
        for diagnostic in &report.diagnostics {
            println!("  {}", format_check_diagnostic(diagnostic));
        }
    }
}

fn print_graph_text(report: &MeltReport) {
    println!("Axonyx App Graph");
    println!("  app={} root={}", report.app.name, report.app.root);
    println!(
        "  pages={} api={} actions={} state_signals={} diagnostics={}",
        report.summary.page_routes,
        report.summary.api_routes,
        report.summary.actions,
        report.summary.state_signals,
        report.summary.diagnostics
    );

    println!();
    println!("Production server:");
    println!("  transport tokio=default std=fallback");
    println!(
        "  stream_pages={} max_body_bytes={}",
        if report.routes.stream_pages {
            "true"
        } else {
            "false"
        },
        format_max_body_bytes_for_root(Path::new(&report.app.root))
    );
    println!("  hosted_start=PORT-aware via `cargo ax run start`");

    println!();
    println!("Route/state graph:");
    let page_routes = report
        .routes
        .routes
        .iter()
        .filter(|route| route.kind == "page")
        .collect::<Vec<_>>();
    if page_routes.is_empty() {
        println!("  No page routes found.");
    }
    for route in page_routes {
        let signals = state_signal_labels_for_route(&report.state, &route.route);
        let mut details = vec![format!("file={}", route.file)];
        if route.loader.is_some() {
            details.push("loader".to_string());
        }
        if route.actions.is_some() {
            details.push("actions".to_string());
        }
        if !route.params.is_empty() {
            details.push(format!("params={}", route.params.join(",")));
        }
        if signals.is_empty() {
            details.push("state=none".to_string());
        } else {
            details.push(format!("state={}", signals.join(",")));
        }
        println!("  {:<28} {}", route.route, details.join(" "));
    }

    if !report.actions.routes.is_empty() {
        println!();
        println!("Action patch surface:");
        for route in &report.actions.routes {
            let action_names = route
                .actions
                .iter()
                .map(|action| action.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            println!("  {:<28} actions={}", route.route, action_names);
        }
    }

    if !report.diagnostics.is_empty() {
        println!();
        println!("Diagnostics:");
        for diagnostic in &report.diagnostics {
            println!("  {}", format_check_diagnostic(diagnostic));
        }
    }
}

fn print_routes_text(report: &RoutesReport) {
    if report.routes.is_empty() {
        println!("No routes found in app/**/page.ax or routes/**/*.ax.");
        return;
    }

    println!("Routes:");
    println!(
        "  server stream_pages={}",
        if report.stream_pages { "true" } else { "false" }
    );
    for route in &report.routes {
        let mut details = vec![format!("kind={}", route.kind)];
        if let Some(method) = &route.method {
            details.push(format!("method={method}"));
        }
        if let Some(returns) = &route.returns {
            details.push(format!("returns={returns}"));
        }
        details.push(format!("file={}", route.file));
        if !route.layouts.is_empty() {
            details.push(format!("layouts={}", route.layouts.len()));
        }
        if route.loader.is_some() {
            details.push("loader".to_string());
        }
        if route.actions.is_some() {
            details.push("actions".to_string());
        }
        if !route.params.is_empty() {
            details.push(format!("params={}", route.params.join(",")));
        }
        if !route.inputs.is_empty() {
            details.push(format!(
                "inputs={}",
                route
                    .inputs
                    .iter()
                    .map(route_input_label)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !route.hooks.is_empty() {
            details.push(format!(
                "hooks={}",
                route
                    .hooks
                    .iter()
                    .map(route_hook_label)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }

        println!("  {:<28} {}", route.route, details.join(" "));
    }
}

fn route_hook_label(hook: &RouteHookReport) -> String {
    format!("{}:{}", hook.phase, hook.value)
}

fn route_input_label(input: &ActionInputReport) -> String {
    let marker = if input.optional { "?" } else { "" };
    input
        .default
        .as_ref()
        .map(|default| format!("{}{marker}:{}={default}", input.name, input.ty))
        .unwrap_or_else(|| format!("{}{marker}:{}", input.name, input.ty))
}

fn print_api_text(report: &ApiReport) {
    if report.routes.is_empty() {
        println!("No API routes found in routes/**/*.ax.");
        return;
    }

    println!("API:");
    for route in &report.routes {
        let mut details = vec![format!("file={}", route.file)];
        if let Some(returns) = &route.returns {
            details.push(format!("returns={returns}"));
        }
        if !route.params.is_empty() {
            details.push(format!("params={}", route.params.join(",")));
        }
        if route.inputs.is_empty() {
            details.push("inputs=none".to_string());
        } else {
            details.push(format!(
                "inputs={}",
                route
                    .inputs
                    .iter()
                    .map(route_input_label)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !route.hooks.is_empty() {
            details.push(format!(
                "hooks={}",
                route
                    .hooks
                    .iter()
                    .map(route_hook_label)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }

        println!(
            "  {:<6} {:<28} {}",
            route.method,
            route.route,
            details.join(" ")
        );
    }
}

fn print_api_schema_text(report: &ApiReport) {
    if report.routes.is_empty() {
        println!("// No API routes found in routes/**/*.ax.");
        return;
    }

    for route in &report.routes {
        println!("// {} {}", route.method, route.route);
        if let Some(returns) = &route.returns {
            println!("// response: {}", ax_return_schema_type(returns));
        }
        println!("type {}Request {{", api_route_type_name(route));
        if route.inputs.is_empty() {
            println!("  // no input body");
        } else {
            for input in &route.inputs {
                let marker = if input.optional { "?" } else { "" };
                let default = input
                    .default
                    .as_ref()
                    .map(|value| format!(" = {value}"))
                    .unwrap_or_default();
                println!(
                    "  {}{}: {}{}",
                    input.name,
                    marker,
                    ax_schema_type(&input.ty),
                    default
                );
            }
        }
        println!("}}\n");
    }
}

fn api_report_openapi_value(report: &ApiReport) -> serde_json::Value {
    let mut paths = serde_json::Map::new();

    for route in &report.routes {
        let path = openapi_path(&route.route);
        let method = route.method.to_ascii_lowercase();
        let operation = openapi_operation_for_route(route);

        let path_item = paths
            .entry(path)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(methods) = path_item {
            methods.insert(method, operation);
        }
    }

    let mut document = serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Axonyx API",
            "version": "0.1.0"
        },
        "paths": paths
    });

    if !report.schemas.is_empty() {
        document["components"] = serde_json::json!({
            "schemas": openapi_components_schemas(&report.schemas)
        });
    }

    document
}

fn openapi_operation_for_route(route: &ApiRouteReport) -> serde_json::Value {
    let mut operation = serde_json::Map::new();
    operation.insert(
        "operationId".to_string(),
        serde_json::Value::String(api_route_type_name(route)),
    );

    let parameters = openapi_parameters_for_route(route);
    if !parameters.is_empty() {
        operation.insert(
            "parameters".to_string(),
            serde_json::Value::Array(parameters),
        );
    }

    if !route.inputs.is_empty() {
        operation.insert(
            "requestBody".to_string(),
            serde_json::json!({
                "required": true,
                "content": {
                    "application/json": {
                        "schema": openapi_input_object_schema(&route.inputs)
                    }
                }
            }),
        );
    }

    let response_schema = route
        .returns
        .as_deref()
        .map(openapi_schema_for_ax_type)
        .unwrap_or_else(|| serde_json::json!({ "type": "object" }));

    operation.insert(
        "responses".to_string(),
        serde_json::json!({
            "200": {
                "description": "OK",
                "content": {
                    "application/json": {
                        "schema": response_schema
                    }
                }
            }
        }),
    );

    serde_json::Value::Object(operation)
}

fn openapi_parameters_for_route(route: &ApiRouteReport) -> Vec<serde_json::Value> {
    route
        .params
        .iter()
        .map(|param| {
            serde_json::json!({
                "name": param,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            })
        })
        .collect()
}

fn openapi_input_object_schema(inputs: &[ActionInputReport]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for input in inputs {
        properties.insert(input.name.clone(), openapi_schema_for_ax_type(&input.ty));
        if !input.optional && input.default.is_none() {
            required.push(serde_json::Value::String(input.name.clone()));
        }
    }

    let mut schema = serde_json::Map::new();
    schema.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    schema.insert(
        "properties".to_string(),
        serde_json::Value::Object(properties),
    );
    if !required.is_empty() {
        schema.insert("required".to_string(), serde_json::Value::Array(required));
    }

    serde_json::Value::Object(schema)
}

fn openapi_components_schemas(schemas: &[ApiSchemaReport]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for schema in schemas {
        out.insert(schema.name.clone(), openapi_component_schema(schema));
    }
    serde_json::Value::Object(out)
}

fn openapi_component_schema(schema: &ApiSchemaReport) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in &schema.fields {
        properties.insert(field.name.clone(), openapi_schema_for_ax_type(&field.ty));
        if !field.optional {
            required.push(serde_json::Value::String(field.name.clone()));
        }
    }

    let mut object = serde_json::Map::new();
    object.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    object.insert(
        "properties".to_string(),
        serde_json::Value::Object(properties),
    );
    if !required.is_empty() {
        object.insert("required".to_string(), serde_json::Value::Array(required));
    }

    serde_json::Value::Object(object)
}

fn openapi_schema_for_ax_type(ty: &str) -> serde_json::Value {
    let ty = ty.trim();

    if let Some(inner) = ty.strip_suffix("[]") {
        return serde_json::json!({
            "type": "array",
            "items": openapi_schema_for_ax_type(inner)
        });
    }

    if let Some(inner) = ty
        .strip_prefix("List<")
        .and_then(|remaining| remaining.strip_suffix('>'))
    {
        return serde_json::json!({
            "type": "array",
            "items": openapi_schema_for_ax_type(inner)
        });
    }

    if let Some(inner) = ty
        .strip_prefix("Optional<")
        .and_then(|remaining| remaining.strip_suffix('>'))
    {
        let mut schema = openapi_schema_for_ax_type(inner);
        if let serde_json::Value::Object(object) = &mut schema {
            object.insert("nullable".to_string(), serde_json::Value::Bool(true));
        }
        return schema;
    }

    match ty.to_ascii_lowercase().as_str() {
        "string" | "str" => serde_json::json!({ "type": "string" }),
        "bool" | "boolean" => serde_json::json!({ "type": "boolean" }),
        "i64" | "u64" | "int" | "integer" => serde_json::json!({ "type": "integer" }),
        "f64" | "float" | "number" => serde_json::json!({ "type": "number" }),
        "json" => serde_json::json!({}),
        "null" => serde_json::json!({ "type": "null" }),
        _ => serde_json::json!({ "$ref": format!("#/components/schemas/{ty}") }),
    }
}

fn openapi_path(route: &str) -> String {
    route
        .split('/')
        .map(|segment| {
            segment
                .strip_prefix(':')
                .map(|param| format!("{{{param}}}"))
                .unwrap_or_else(|| segment.to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn api_route_type_name(route: &ApiRouteReport) -> String {
    format!(
        "{}{}",
        sanitize_type_name(&route.method.to_ascii_lowercase()),
        sanitize_type_name(&route.route)
    )
}

fn print_actions_text(report: &ActionReport) {
    if report.routes.is_empty() {
        println!("No route-local actions found in app/**/actions.ax.");
        return;
    }

    println!("Actions:");
    for route in &report.routes {
        println!("  {:<28} file={}", route.route, route.file);
        for action in &route.actions {
            if let Some(returns) = &action.returns {
                println!("    {} -> {}", action.name, returns);
            } else {
                println!("    {}", action.name);
            }
            if action.inputs.is_empty() {
                println!("      inputs: none");
                continue;
            }

            for input in &action.inputs {
                let required = if input.optional {
                    "optional"
                } else {
                    "required"
                };
                let default = input
                    .default
                    .as_ref()
                    .map(|value| format!(" default={value}"))
                    .unwrap_or_default();

                println!(
                    "      {:<18} type={} {}{}",
                    input.name, input.ty, required, default
                );
            }
        }
    }
}

fn print_actions_schema_text(report: &ActionReport) {
    if report.routes.is_empty() {
        println!("No route-local actions found in app/**/actions.ax.");
        return;
    }

    for route in &report.routes {
        println!("// {}", route.route);
        for action in &route.actions {
            if let Some(returns) = &action.returns {
                println!("// response: {}", ax_return_schema_type(returns));
            }
            println!("type {}Input {{", action.name);
            if action.inputs.is_empty() {
                println!("  // no inputs");
            } else {
                for input in &action.inputs {
                    let marker = if input.optional { "?" } else { "" };
                    let default = input
                        .default
                        .as_ref()
                        .map(|value| format!(" = {value}"))
                        .unwrap_or_default();
                    println!(
                        "  {}{}: {}{}",
                        input.name,
                        marker,
                        ax_schema_type(&input.ty),
                        default
                    );
                }
            }
            println!("}}\n");
        }
    }
}

fn ax_schema_type(input_ty: &str) -> &'static str {
    match input_ty.to_ascii_lowercase().as_str() {
        "string" | "str" => "String",
        "bool" | "boolean" => "Bool",
        "i64" | "u64" | "int" | "integer" | "number" => "Number",
        _ => "String",
    }
}

fn ax_return_schema_type(return_ty: &str) -> String {
    let return_ty = return_ty.trim();

    if let Some(inner) = return_ty.strip_suffix("[]") {
        return format!("List<{}>", ax_return_schema_type(inner));
    }

    match return_ty.to_ascii_lowercase().as_str() {
        "string" | "str" => "String".to_string(),
        "bool" | "boolean" => "Bool".to_string(),
        "i64" | "u64" | "int" | "integer" | "number" | "f64" | "float" => "Number".to_string(),
        _ => return_ty.to_string(),
    }
}

fn format_ax_expr(expr: &AxExpr) -> String {
    match expr {
        AxExpr::String(value) => format!("{value:?}"),
        AxExpr::Number(value) => value.to_string(),
        AxExpr::Bool(value) => value.to_string(),
        AxExpr::Identifier(value) => value.clone(),
        AxExpr::Member { object, property } => format!("{}.{}", format_ax_expr(object), property),
        AxExpr::OptionalMember { object, property } => {
            format!("{}?.{}", format_ax_expr(object), property)
        }
        AxExpr::Call { path, args } => {
            let args = args
                .iter()
                .map(format_ax_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({args})", path.join("."))
        }
    }
}

fn collect_state_report(root: &Path) -> Result<StateReport> {
    let mut paths = Vec::new();
    collect_ax_files(&root.join("app"), &mut paths)?;
    paths.sort();

    let mut files = Vec::new();
    for path in paths {
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read '{}'", path.display()))?;
        if !source_has_state_declaration(&source) {
            continue;
        }

        let file = parse_ax_v2(&source).with_context(|| {
            format!(
                "failed to parse state declarations from '{}'",
                path.display()
            )
        })?;
        let scope = state_scope_for_path(root, &path);
        let manifest =
            build_state_manifest_with_scope_mapper(&file, &scope, |state, default_scope| {
                scoped_state_decl_scope(root, &path, state.scope.as_deref())
                    .unwrap_or_else(|| default_scope.to_string())
            })
            .with_context(|| format!("failed to build state manifest for '{}'", path.display()))?;
        if manifest.is_empty() {
            continue;
        }

        let signals = manifest
            .signals
            .into_iter()
            .zip(file.states.iter())
            .map(|(signal, state)| StateReportSignal {
                name: signal.name,
                key: signal.key,
                scope: signal.scope,
                owner: scoped_state_decl_owner(root, &path, state.scope.as_deref())
                    .unwrap_or_else(|| state_owner_for_path(root, &path)),
                ty: signal.ty,
                initial: signal.initial,
            })
            .collect();

        files.push(StateReportFile {
            file: display_relative_path(root, &path),
            signals,
        });
    }

    Ok(StateReport { files })
}

fn source_has_state_declaration(source: &str) -> bool {
    source.lines().map(str::trim_start).any(|line| {
        line.starts_with("state ")
            || line.starts_with("app state ")
            || line.starts_with("layout state ")
            || line.starts_with("page state ")
    })
}

fn state_scope_for_path(root: &Path, path: &Path) -> String {
    let app_root = root.join("app");
    let relative = path.strip_prefix(&app_root).unwrap_or(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));

    if file_name == "layout.ax" && parent.components().next().is_none() {
        return "app".to_string();
    }

    let route = route_pattern_for_app_relative_dir(parent);
    let route_scope = scope_route_fragment(&route);

    match file_name {
        "layout.ax" => format!("layout:{route_scope}"),
        "page.ax" => format!("page:{route_scope}"),
        other => format!("file:{}", other.trim_end_matches(".ax")),
    }
}

fn state_owner_for_path(root: &Path, path: &Path) -> String {
    let app_root = root.join("app");
    let relative = path.strip_prefix(&app_root).unwrap_or(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));

    if file_name == "layout.ax" && parent.components().next().is_none() {
        return "app".to_string();
    }

    let route = route_pattern_for_app_relative_dir(parent);

    match file_name {
        "layout.ax" => format!("layout:{route}"),
        "page.ax" => format!("page:{route}"),
        other => format!("file:{other}"),
    }
}

fn scoped_state_decl_scope(root: &Path, path: &Path, scope: Option<&str>) -> Option<String> {
    let scope = scope?;
    let app_root = root.join("app");
    let relative = path.strip_prefix(&app_root).unwrap_or(path);
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let route = route_pattern_for_app_relative_dir(parent);
    let route_scope = scope_route_fragment(&route);

    match scope {
        "app" => Some("app".to_string()),
        "layout" => Some(format!("layout:{route_scope}")),
        "page" => Some(format!("page:{route_scope}")),
        _ => None,
    }
}

fn scoped_state_decl_owner(root: &Path, path: &Path, scope: Option<&str>) -> Option<String> {
    let scope = scope?;
    let app_root = root.join("app");
    let relative = path.strip_prefix(&app_root).unwrap_or(path);
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let route = route_pattern_for_app_relative_dir(parent);

    match scope {
        "app" => Some("app".to_string()),
        "layout" => Some(format!("layout:{route}")),
        "page" => Some(format!("page:{route}")),
        _ => None,
    }
}

fn app_root_for_app_path(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    loop {
        if current.file_name().and_then(|name| name.to_str()) == Some("app") {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn route_pattern_for_app_relative_dir(relative: &Path) -> String {
    let segments = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    route_pattern_from_segments(&segments)
}

fn scope_route_fragment(route: &str) -> String {
    if route == "/" {
        return "root".to_string();
    }
    route
        .trim_matches('/')
        .replace('/', ".")
        .replace(':', "$")
        .replace(['[', ']'], "")
}

fn print_state_text(report: &StateReport) {
    if report.files.is_empty() {
        println!("No state declarations found in app/**/*.ax.");
        return;
    }

    println!("State manifest:");
    for file in &report.files {
        println!("  {}", file.file);
        for signal in &file.signals {
            println!(
                "    {:<18} key={} owner={} type={} initial={}",
                signal.name,
                signal.key,
                signal.owner,
                signal.ty,
                format_state_value(&signal.initial)
            );
        }
    }
}

fn format_state_value(value: &AxStateValue) -> String {
    match value {
        AxStateValue::Null => "null".to_string(),
        AxStateValue::String(value) => format!("{value:?}"),
        AxStateValue::Bool(value) => value.to_string(),
        AxStateValue::Number(value) => {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        }
    }
}

fn state_signal_labels_for_route(report: &StateReport, route: &str) -> Vec<String> {
    let mut labels = Vec::new();
    for file in &report.files {
        for signal in &file.signals {
            if state_signal_is_visible_to_route(signal, route) {
                labels.push(format!("{}:{}", signal.owner, signal.name));
            }
        }
    }
    labels.sort();
    labels.dedup();
    labels
}

fn state_signal_is_visible_to_route(signal: &StateReportSignal, route: &str) -> bool {
    if signal.owner == "app" {
        return true;
    }

    if signal.owner == format!("page:{route}") || signal.owner == format!("layout:{route}") {
        return true;
    }

    let Some(layout_route) = signal.owner.strip_prefix("layout:") else {
        return false;
    };

    layout_route != "/" && route.starts_with(&format!("{layout_route}/"))
}

fn format_max_body_bytes_for_root(root: &Path) -> String {
    configured_max_request_body_bytes(root)
        .map(format_bytes)
        .unwrap_or_else(|error| format!("invalid ({error})"))
}

fn collect_backend_sources(root: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
    let routes_root = root.join("routes");
    let jobs_root = root.join("jobs");
    let app_root = root.join("app");

    collect_backend_sources_in_dir(&routes_root, &routes_root, out, true)?;
    collect_backend_sources_in_dir(&jobs_root, &jobs_root, out, true)?;
    collect_named_backend_files(&app_root, &app_root, out, &["loader.ax", "actions.ax"])?;
    Ok(())
}

fn collect_backend_sources_in_dir(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
    recurse: bool,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .with_context(|| format!("failed to read directory '{}'", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() && recurse {
            collect_backend_sources_in_dir(root, &path, out, true)?;
            continue;
        }

        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("ax") {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("failed to read backend source '{}'", path.display()))?;
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((name, source));
        }
    }

    Ok(())
}

fn collect_named_backend_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
    names: &[&str],
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .with_context(|| format!("failed to read directory '{}'", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_named_backend_files(root, &path, out, names)?;
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !names.contains(&file_name) {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read backend source '{}'", path.display()))?;
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((name, source));
    }

    Ok(())
}

fn scaffold_docs_module(root: &Path) -> Result<()> {
    write_if_missing(root, "app/docs/layout.ax", DOCS_LAYOUT_AX)?;
    write_if_missing(root, "app/docs/page.ax", DOCS_HOME_AX)?;
    write_if_missing(
        root,
        "app/docs/getting-started/page.ax",
        DOCS_GETTING_STARTED_AX,
    )?;
    write_if_missing(root, "app/docs/reference/page.ax", DOCS_REFERENCE_AX)?;
    write_if_missing(root, "app/docs/examples/page.ax", DOCS_EXAMPLES_AX)?;
    Ok(())
}

fn add_ui_module(root: &Path, axonyx_toml: &Path) -> Result<()> {
    ensure_ui_cargo_dependency(root)?;
    let _ = ensure_ui_layout_setup(root)?;
    enable_module(&axonyx_toml.to_path_buf(), "ui")?;

    println!("Ensured Cargo dependency: axonyx-ui = \"{AXONYX_UI_VERSION}\".");
    println!("Updated app/layout.ax with silver theme and Axonyx UI package use when possible.");
    println!("You can now import components such as:");
    println!("  import {{ SectionCard }} from \"@axonyx/ui/foundry/SectionCard.ax\"");
    Ok(())
}

fn ensure_ui_cargo_dependency(root: &Path) -> Result<()> {
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Ok(());
    }

    ensure_cargo_dependency_version(&cargo_toml, "axonyx-ui", AXONYX_UI_VERSION)
}

fn ensure_cargo_dependency_version(
    cargo_toml: &Path,
    dependency_name: &str,
    dependency_version: &str,
) -> Result<()> {
    let source = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read '{}'", cargo_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", cargo_toml.display()))?;
    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml root must be a TOML table"))?;
    let dependencies = root_table
        .entry("dependencies")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let dependencies_table = dependencies
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("[dependencies] must be a TOML table"))?;

    if dependencies_table.contains_key(dependency_name) {
        return Ok(());
    }

    dependencies_table.insert(
        dependency_name.to_string(),
        toml::Value::String(dependency_version.to_string()),
    );

    let rendered = toml::to_string_pretty(&value).context("failed to render Cargo.toml")?;
    fs::write(cargo_toml, rendered)
        .with_context(|| format!("failed to write '{}'", cargo_toml.display()))?;
    Ok(())
}

fn upgrade_cargo_dependency_version(
    cargo_toml: &Path,
    dependency_name: &str,
    dependency_version: &str,
) -> Result<bool> {
    let source = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read '{}'", cargo_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", cargo_toml.display()))?;
    let Some(dependencies_table) = value
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
    else {
        return Ok(false);
    };
    let Some(dependency) = dependencies_table.get_mut(dependency_name) else {
        return Ok(false);
    };

    let changed = match dependency {
        toml::Value::String(version) => {
            if version == dependency_version {
                false
            } else {
                *version = dependency_version.to_string();
                true
            }
        }
        toml::Value::Table(table) if table.contains_key("path") || table.contains_key("git") => {
            false
        }
        toml::Value::Table(table) => match table.get_mut("version") {
            Some(toml::Value::String(version)) if version != dependency_version => {
                *version = dependency_version.to_string();
                true
            }
            Some(toml::Value::String(_)) => false,
            _ => {
                table.insert(
                    "version".to_string(),
                    toml::Value::String(dependency_version.to_string()),
                );
                true
            }
        },
        _ => false,
    };

    if changed {
        let rendered = toml::to_string_pretty(&value).context("failed to render Cargo.toml")?;
        fs::write(cargo_toml, rendered)
            .with_context(|| format!("failed to write '{}'", cargo_toml.display()))?;
    }

    Ok(changed)
}

fn copy_dir_all_filtered(
    source: &Path,
    destination: &Path,
    skip: impl Fn(&Path) -> bool + Copy,
) -> Result<()> {
    if skip(source) {
        return Ok(());
    }

    if source.is_dir() {
        fs::create_dir_all(destination)
            .with_context(|| format!("failed to create '{}'", destination.display()))?;

        for entry in fs::read_dir(source)
            .with_context(|| format!("failed to read '{}'", source.display()))?
        {
            let entry =
                entry.with_context(|| format!("failed to read entry in '{}'", source.display()))?;
            let path = entry.path();
            let target = destination.join(entry.file_name());
            copy_dir_all_filtered(&path, &target, skip)?;
        }
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy '{}' to '{}'",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn ensure_ui_layout_setup(root: &Path) -> Result<bool> {
    let layout_path = root.join("app").join("layout.ax");
    if !layout_path.exists() {
        return Ok(false);
    }

    let source = fs::read_to_string(&layout_path)
        .with_context(|| format!("failed to read '{}'", layout_path.display()))?;
    let updated = if source.contains("<Head>") {
        ensure_ui_layout_setup_jsx(&source)
    } else {
        ensure_ui_layout_setup_v1(&source)
    };

    if updated != source {
        fs::write(&layout_path, updated)
            .with_context(|| format!("failed to write '{}'", layout_path.display()))?;
        return Ok(true);
    }

    Ok(false)
}

fn ensure_ui_layout_setup_jsx(source: &str) -> String {
    const THEME_TAG: &str = "<Theme>silver</Theme>";

    let mut updated = ensure_package_use_directive(source, AXONYX_UI_USE_DIRECTIVE);

    if updated.contains("<Head>") {
        if !updated.contains(THEME_TAG) {
            updated = updated.replacen("<Head>", &format!("<Head>\n  {THEME_TAG}"), 1);
        }

        return updated;
    }

    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let page_index = lines
        .iter()
        .position(|line| line.trim_start().starts_with("page "))
        .unwrap_or(0);

    let mut head_block = vec![
        String::new(),
        "<Head>".to_string(),
        format!("  {THEME_TAG}"),
        "</Head>".to_string(),
    ];

    lines.splice(page_index + 1..page_index + 1, head_block.drain(..));
    lines.join("\n")
}

fn ensure_package_use_directive(source: &str, directive: &str) -> String {
    if source.lines().any(|line| line.trim() == directive) {
        return source.to_string();
    }

    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let insert_at = lines
        .iter()
        .position(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("use ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("page ")
        })
        .unwrap_or(0);

    lines.insert(insert_at, directive.to_string());
    lines.join("\n")
}

fn ensure_ui_layout_setup_v1(source: &str) -> String {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let Some(page_index) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("page "))
    else {
        return source.to_string();
    };

    let has_theme = lines
        .iter()
        .any(|line| line.trim() == "theme \"silver\"" || line.trim_start().starts_with("theme "));
    let has_stylesheet = lines.iter().any(|line| {
        line.contains(AXONYX_UI_STYLESHEET_HREF) || line.contains("/css/axonyx-ui/index.css")
    });
    let has_script = lines
        .iter()
        .any(|line| line.contains(AXONYX_UI_SCRIPT_HREF));

    if has_theme && has_stylesheet && has_script {
        return source.to_string();
    }

    let mut insert_at = page_index + 1;
    while insert_at < lines.len() {
        let trimmed = lines[insert_at].trim_start();
        if lines[insert_at].starts_with("  ")
            && matches!(
                trimmed.split_whitespace().next(),
                Some("title" | "meta" | "link" | "script" | "theme")
            )
        {
            insert_at += 1;
            continue;
        }
        break;
    }

    let mut to_insert = Vec::new();
    if !has_theme {
        to_insert.push("  theme \"silver\"".to_string());
    }
    if !has_stylesheet {
        to_insert
            .push("  link rel: \"stylesheet\", href: \"/_ax/pkg/axonyx-ui/index.css\"".to_string());
    }
    if !has_script {
        to_insert.push("  script src: \"/_ax/pkg/axonyx-ui/js/index.js\", defer: true".to_string());
    }

    lines.splice(insert_at..insert_at, to_insert);
    lines.join("\n")
}

fn write_if_missing(root: &Path, relative: &str, contents: &str) -> Result<()> {
    let path = root.join(relative);
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory '{}'", parent.display()))?;
    }

    fs::write(&path, contents)
        .with_context(|| format!("failed to write module file '{}'", path.display()))?;
    Ok(())
}

fn enable_module(axonyx_toml: &PathBuf, module_name: &str) -> Result<()> {
    update_axonyx_toml(axonyx_toml, |root_table| {
        let modules = root_table
            .entry("modules")
            .or_insert_with(|| toml::Value::Table(Default::default()));

        let modules_table = modules
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("[modules] must be a TOML table"))?;

        let enabled = modules_table
            .entry("enabled")
            .or_insert_with(|| toml::Value::Array(Vec::new()));

        let enabled_array = enabled
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("[modules].enabled must be an array"))?;

        if !enabled_array
            .iter()
            .any(|item| item.as_str() == Some(module_name))
        {
            enabled_array.push(toml::Value::String(module_name.to_string()));
        }

        Ok(())
    })
}

fn update_axonyx_toml(
    axonyx_toml: &PathBuf,
    update: impl FnOnce(&mut toml::map::Map<String, toml::Value>) -> Result<()>,
) -> Result<()> {
    let source = fs::read_to_string(axonyx_toml)
        .with_context(|| format!("failed to read '{}'", axonyx_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", axonyx_toml.display()))?;

    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Axonyx.toml root must be a TOML table"))?;
    update(root_table)?;

    let rendered = toml::to_string_pretty(&value).context("failed to render Axonyx.toml")?;
    fs::write(axonyx_toml, rendered)
        .with_context(|| format!("failed to write '{}'", axonyx_toml.display()))?;
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    state: &DevServerState,
    mode: AxServerMode,
) -> Result<()> {
    stream
        .set_read_timeout(Some(state.runtime_config.request_timeout))
        .context("failed to set read timeout")?;

    let Some(request) = read_http_request(&mut stream, state.runtime_config.max_body_bytes)? else {
        return Ok(());
    };

    let suppress_body = suppress_response_body_for_method(&request.method);
    let request = normalize_request_for_routing(request);
    let started = Instant::now();
    let response = handle_http_request(state, mode, request.clone())?;
    let response = apply_server_response_policy(state, &request, response, suppress_body)?;
    log_request_if_enabled(state, &request, &response, started.elapsed());
    write_ax_response(&mut stream, &response, suppress_body)?;
    Ok(())
}

fn handle_http_request(
    state: &DevServerState,
    mode: AxServerMode,
    request: AxHttpRequest,
) -> Result<AxHttpResponse> {
    let max_body_bytes = state.runtime_config.max_body_bytes;
    if request_body_exceeds_limit(&request, max_body_bytes) {
        return Ok(AxHttpResponse::text(
            413,
            format!(
                "Payload Too Large: Axonyx currently accepts request bodies up to {}.",
                format_bytes(max_body_bytes)
            ),
        )
        .with_no_store());
    }

    if request.method == "GET" && is_health_target(&request.target) {
        return Ok(health_response(mode)?);
    }

    if mode == AxServerMode::Dev && request.method == "GET" && request.target == "/__axonyx/stream"
    {
        return Ok(stream_probe_response());
    }

    if mode == AxServerMode::Dev
        && request.method == "GET"
        && request.target == "/__axonyx/stream/html"
    {
        return Ok(stream_html_probe_response());
    }

    if mode == AxServerMode::Dev && request.method == "GET" && request.target == "/__axonyx/events"
    {
        return Ok(sse_probe_response());
    }

    if request.method == "GET" {
        if let Some(asset) = load_package_asset(&state.root, &request.target)? {
            return Ok(cacheable_asset_response(asset));
        }

        if let Some(asset) = load_public_asset(&state.root, &request.target)? {
            return Ok(cacheable_asset_response(asset));
        }
    }

    if request.method == "POST" && request.target.starts_with("/__axonyx/action") {
        return handle_action_request(state, &request);
    }

    if request.method == "GET" && request.target == "/favicon.ico" {
        return Ok(AxHttpResponse::text(204, "").with_no_store());
    }

    if mode == AxServerMode::Dev
        && request.method == "GET"
        && request.target.starts_with("/__axonyx/version")
    {
        let request_path = extract_version_path(&request.target).unwrap_or_else(|| "/".to_string());
        let Some(route) = resolve_route(&state.root, &request_path)? else {
            return Ok(AxHttpResponse::text(404, "route not found").with_no_store());
        };

        let version = route_version(&state.root, &route)?;
        return Ok(AxHttpResponse::text(200, version).with_no_store());
    }

    if let Some(response) = execute_backend_route_request(state, &request)? {
        return Ok(preview_response_to_http(response));
    }

    if request.method != "GET" {
        return Ok(method_not_allowed_response("GET, HEAD"));
    }

    let Some(route) = resolve_route(&state.root, &request.target)? else {
        if looks_like_asset_request(&request.target) {
            return Ok(AxHttpResponse::text(404, "asset not found").with_no_store());
        }

        return render_not_found_response(
            state,
            &request.target,
            mode.inject_dev_client(),
            should_stream_page_route(&state.root, &request.target),
        );
    };

    let response = match render_route_response(
        state,
        &route,
        mode.inject_dev_client(),
        should_stream_page_route(&state.root, &request.target),
    ) {
        Ok(response) => response,
        Err(error) => render_error_response(
            state,
            &request.target,
            &error,
            mode.inject_dev_client(),
            should_stream_page_route(&state.root, &request.target),
        )?,
    };
    Ok(response)
}

fn is_health_target(target: &str) -> bool {
    target.split_once('?').map_or(target, |(path, _)| path) == "/__axonyx/health"
}

fn health_response(mode: AxServerMode) -> Result<AxHttpResponse> {
    Ok(AxHttpResponse::json(
        200,
        &serde_json::json!({
            "ok": true,
            "service": "axonyx",
            "mode": mode.label(),
            "version": env!("CARGO_PKG_VERSION"),
        }),
    )?
    .with_no_store())
}

async fn serve_axum_tokio(
    config: AxServerConfig,
    state: Arc<DevServerState>,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_axum_tokio_until(config, state, tokio_shutdown_signal()).await
}

async fn serve_axum_tokio_until<S>(
    config: AxServerConfig,
    state: Arc<DevServerState>,
    shutdown: S,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: Future<Output = ()> + Send + 'static,
{
    let bind = config.bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind Axonyx Axum/Tokio server at {bind}"))?;
    let runtime_config = state.runtime_config;
    let tracker = TokioConnectionTracker::new(
        runtime_config.shutdown_grace,
        runtime_config.max_connections,
    );
    let mode = config.mode;
    let router_state = AxumServerState {
        dev: state,
        mode,
        tracker: tracker.clone(),
    };
    let router = axum::Router::new()
        .fallback(axum::routing::any(axum_tokio_handler))
        .with_state(router_state);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await?;

    println!(
        "Axonyx {} Axum/Tokio server stopped accepting requests.",
        mode.label()
    );
    wait_for_tokio_connections(&tracker).await;
    Ok(())
}

async fn axum_tokio_handler(
    axum::extract::State(state): axum::extract::State<AxumServerState>,
    request: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let Some(_request_guard) = state.tracker.try_track() else {
        return axonyx_response_to_axum(
            AxHttpResponse::text(
                503,
                format!(
                    "Service Unavailable: Axonyx is already handling {} active request{}.",
                    state.tracker.max_connections,
                    if state.tracker.max_connections == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            )
            .with_header("Retry-After", "1")
            .with_no_store(),
        );
    };

    let runtime_config = state.dev.runtime_config;
    match axum_request_to_dev_request(
        request,
        runtime_config.max_body_bytes,
        runtime_config.request_timeout,
    )
    .await
    {
        Ok(request) => {
            let suppress_body = suppress_response_body_for_method(&request.method);
            let request = normalize_request_for_routing(request);
            let started = Instant::now();
            let response = match handle_http_request(&state.dev, state.mode, request.clone()) {
                Ok(response) => response,
                Err(error) => AxHttpResponse::text(500, format!("Axonyx server error: {error:#}"))
                    .with_no_store(),
            };
            let response =
                match apply_server_response_policy(&state.dev, &request, response, suppress_body) {
                    Ok(response) => response,
                    Err(error) => {
                        AxHttpResponse::text(500, format!("Axonyx server policy error: {error:#}"))
                            .with_no_store()
                    }
                };
            log_request_if_enabled(&state.dev, &request, &response, started.elapsed());
            let mut response = axonyx_response_to_axum(response);
            if suppress_body {
                *response.body_mut() = axum::body::Body::empty();
            }
            response
        }
        Err(error) => axonyx_response_to_axum(error),
    }
}

async fn axum_request_to_dev_request(
    request: axum::http::Request<axum::body::Body>,
    max_body_bytes: usize,
    request_timeout: Duration,
) -> std::result::Result<AxHttpRequest, AxHttpResponse> {
    let (parts, body) = request.into_parts();
    let method = parts.method.as_str().to_string();
    let target = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let mut headers = std::collections::BTreeMap::new();
    for (name, value) in parts.headers.iter() {
        if let Ok(value) = value.to_str() {
            headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
        }
    }
    if headers
        .get("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .is_some_and(|length| length > max_body_bytes)
    {
        return Err(
            AxHttpResponse::text(413, "Payload Too Large").with_header("Connection", "close")
        );
    }

    let body =
        match tokio::time::timeout(request_timeout, axum::body::to_bytes(body, max_body_bytes))
            .await
        {
            Ok(Ok(body)) => body.to_vec(),
            Ok(Err(_)) => {
                return Err(AxHttpResponse::text(413, "Payload Too Large")
                    .with_header("Connection", "close"));
            }
            Err(_) => {
                return Err(
                    AxHttpResponse::text(408, "Request Timeout").with_header("Connection", "close")
                );
            }
        };

    Ok(AxHttpRequest {
        method,
        target,
        headers,
        body,
    })
}

#[cfg(test)]
async fn serve_tokio_until<S>(
    config: AxServerConfig,
    state: Arc<DevServerState>,
    shutdown_grace: Duration,
    max_connections: usize,
    shutdown: S,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: Future<Output = ()>,
{
    let bind = config.bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind Axonyx Tokio server at {bind}"))?;
    let tracker = TokioConnectionTracker::new(shutdown_grace, max_connections);
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                println!("Axonyx {} Tokio server shutdown signal received.", config.mode.label());
                break;
            }

            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let state = Arc::clone(&state);
                let mode = config.mode;
                let Some(connection_guard) = tracker.try_track() else {
                    reject_tokio_connection(stream, max_connections).await;
                    continue;
                };

                tokio::spawn(async move {
                    let _connection_guard = connection_guard;
                    if let Err(error) = handle_tokio_connection(stream, state, mode).await {
                        eprintln!("Axonyx {} Tokio server error: {error:#}", mode.label());
                    }
                });
            }
        }
    }

    wait_for_tokio_connections(&tracker).await;
    Ok(())
}

#[cfg(test)]
async fn reject_tokio_connection(mut stream: tokio::net::TcpStream, max_connections: usize) {
    let response = AxHttpResponse::text(
        503,
        format!(
            "Service Unavailable: Axonyx is already handling {max_connections} active connection{}.",
            if max_connections == 1 { "" } else { "s" }
        ),
    )
    .with_header("Retry-After", "1")
    .with_no_store();

    if let Err(error) = write_ax_response_async(&mut stream, &response, false).await {
        eprintln!("Axonyx Tokio connection rejection failed: {error:#}");
    }
}

async fn wait_for_tokio_connections(tracker: &TokioConnectionTracker) {
    let active = tracker.active_count();
    if active == 0 {
        println!("No active Tokio connections to drain.");
        return;
    }

    println!(
        "Waiting up to {} seconds for {} active Tokio connection{} to finish.",
        tracker.grace_period.as_secs(),
        active,
        if active == 1 { "" } else { "s" }
    );

    let drained = tokio::time::timeout(tracker.grace_period, async {
        loop {
            if tracker.active_count() == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();

    if drained {
        println!("All active Tokio connections drained.");
    } else {
        println!(
            "Shutdown grace period elapsed with {} active Tokio connection{}.",
            tracker.active_count(),
            if tracker.active_count() == 1 { "" } else { "s" }
        );
    }
}

async fn tokio_shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("Axonyx shutdown signal listener failed: {error}");
    }
}

#[cfg(test)]
async fn handle_tokio_connection(
    mut stream: tokio::net::TcpStream,
    state: Arc<DevServerState>,
    mode: AxServerMode,
) -> Result<()> {
    let max_body_bytes =
        configured_max_request_body_bytes(&state.root).map_err(anyhow::Error::msg)?;
    let request_timeout =
        configured_request_timeout_duration(&state.root).map_err(anyhow::Error::msg)?;
    let Some(request) =
        read_http_request_async(&mut stream, max_body_bytes, request_timeout).await?
    else {
        return Ok(());
    };
    let suppress_body = suppress_response_body_for_method(&request.method);
    let request = normalize_request_for_routing(request);
    let response = handle_http_request(&state, mode, request)?;
    write_ax_response_async(&mut stream, &response, suppress_body).await
}

fn suppress_response_body_for_method(method: &str) -> bool {
    method.eq_ignore_ascii_case("HEAD")
}

fn normalize_request_for_routing(mut request: AxHttpRequest) -> AxHttpRequest {
    if suppress_response_body_for_method(&request.method) {
        request.method = "GET".to_string();
    }

    request
}

fn execute_backend_route_request(
    state: &DevServerState,
    request: &AxHttpRequest,
) -> Result<Option<AxPreviewHttpResponse>> {
    let mut sources = Vec::new();
    let routes_root = state.root.join("routes");
    collect_backend_sources_in_dir(&routes_root, &routes_root, &mut sources, true)?;

    if sources.is_empty() {
        return Ok(None);
    }

    let source_refs = sources
        .iter()
        .map(|(_, source)| source.as_str())
        .collect::<Vec<_>>();
    let mut store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;

    execute_preview_route_request_sources(&source_refs, request, &mut store).with_context(|| {
        format!(
            "failed to execute backend route {} {}",
            request.method, request.target
        )
    })
}

fn preview_response_to_http(response: AxPreviewHttpResponse) -> AxHttpResponse {
    let mut http = AxHttpResponse::bytes(response.status, response.content_type, response.body)
        .with_no_store();
    for (name, value) in response.headers {
        http = http.with_header(name, value);
    }
    http.set_cookies.extend(response.set_cookies);
    http
}

fn apply_server_response_policy(
    state: &DevServerState,
    request: &AxHttpRequest,
    response: AxHttpResponse,
    suppress_body: bool,
) -> Result<AxHttpResponse> {
    let mut response = if state.runtime_config.security_headers {
        apply_security_headers(response)
    } else {
        response
    };

    if state.runtime_config.compression
        && !suppress_body
        && request_accepts_gzip(request)
        && should_gzip_response(&response)
    {
        response = gzip_response(response)?;
    }

    Ok(response)
}

fn apply_security_headers(mut response: AxHttpResponse) -> AxHttpResponse {
    response = ensure_response_header(response, "X-Content-Type-Options", "nosniff");
    response = ensure_response_header(response, "X-Frame-Options", "DENY");
    response = ensure_response_header(
        response,
        "Referrer-Policy",
        "strict-origin-when-cross-origin",
    );
    response = ensure_response_header(
        response,
        "Permissions-Policy",
        "geolocation=(), microphone=(), camera=()",
    );
    response
}

fn ensure_response_header(
    response: AxHttpResponse,
    name: &'static str,
    value: &'static str,
) -> AxHttpResponse {
    if response.header_value(name).is_some() {
        response
    } else {
        response.with_header(name, value)
    }
}

fn request_accepts_gzip(request: &AxHttpRequest) -> bool {
    request.headers.get("accept-encoding").is_some_and(|value| {
        value
            .split(',')
            .any(|encoding| encoding.trim().eq_ignore_ascii_case("gzip"))
    })
}

fn should_gzip_response(response: &AxHttpResponse) -> bool {
    if response.body.is_streaming()
        || response.body_len() < 1024
        || response.header_value("Content-Encoding").is_some()
    {
        return false;
    }

    let content_type = response.content_type.to_ascii_lowercase();
    content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("javascript")
        || content_type.contains("xml")
        || content_type.contains("wasm")
}

fn gzip_response(response: AxHttpResponse) -> Result<AxHttpResponse> {
    let AxHttpResponse {
        status,
        content_type,
        headers,
        set_cookies,
        body,
    } = response;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&body.into_bytes())
        .context("failed to gzip response body")?;
    let compressed = encoder.finish().context("failed to finish gzip body")?;
    let mut response = AxHttpResponse::bytes(status, content_type, compressed);
    response.headers = headers;
    response.set_cookies = set_cookies;
    response = response.with_header("Content-Encoding", "gzip");
    response = response.with_header("Vary", "Accept-Encoding");
    Ok(response)
}

fn log_request_if_enabled(
    state: &DevServerState,
    request: &AxHttpRequest,
    response: &AxHttpResponse,
    duration: Duration,
) {
    if !state.runtime_config.request_logging {
        return;
    }

    println!(
        "{}",
        render_request_log_line(state.runtime_config.log_format, request, response, duration)
    );
}

fn render_request_log_line(
    format: AxServerLogFormat,
    request: &AxHttpRequest,
    response: &AxHttpResponse,
    duration: Duration,
) -> String {
    match format {
        AxServerLogFormat::Text => render_text_request_log_line(request, response, duration),
        AxServerLogFormat::Json => render_json_request_log_line(request, response, duration),
    }
}

fn render_text_request_log_line(
    request: &AxHttpRequest,
    response: &AxHttpResponse,
    duration: Duration,
) -> String {
    format!(
        "[axonyx] {} {} {} {} {} {}",
        request.method,
        request.target,
        response.status,
        format_duration(duration),
        response.content_type,
        format_bytes(response.body_len())
    )
}

fn render_json_request_log_line(
    request: &AxHttpRequest,
    response: &AxHttpResponse,
    duration: Duration,
) -> String {
    serde_json::json!({
        "source": "axonyx",
        "method": request.method,
        "path": request.target,
        "status": response.status,
        "duration_ms": duration.as_millis(),
        "content_type": response.content_type,
        "bytes": response.body_len(),
    })
    .to_string()
}

fn format_duration(duration: Duration) -> String {
    if duration.as_millis() == 0 {
        format!("{}us", duration.as_micros())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn method_not_allowed_response(allow: &str) -> AxHttpResponse {
    AxHttpResponse::text(405, "Method Not Allowed")
        .with_header("Allow", allow)
        .with_no_store()
}

fn stream_probe_response() -> AxHttpResponse {
    AxHttpResponse::stream_chunks(
        200,
        "text/plain; charset=utf-8",
        vec![
            b"axonyx-stream:start\n".to_vec(),
            b"axonyx-stream:chunk\n".to_vec(),
            b"axonyx-stream:end\n".to_vec(),
        ],
    )
    .with_no_store()
}

fn stream_html_probe_response() -> AxHttpResponse {
    AxHttpResponse::stream_chunks(
        200,
        "text/html; charset=utf-8",
        vec![
            b"<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Axonyx Stream</title><style>body{margin:0;background:#0f1115;color:#f4efe6;font-family:Georgia,serif}.shell{min-height:100vh;display:grid;place-items:center;padding:48px}.card{max-width:720px;border:1px solid rgba(232,183,103,.35);background:linear-gradient(135deg,rgba(255,255,255,.08),rgba(255,255,255,.02));border-radius:28px;padding:32px;box-shadow:0 24px 80px rgba(0,0,0,.42)}.eyebrow{color:#e8b767;text-transform:uppercase;letter-spacing:.16em;font-size:12px}.chunk{margin-top:18px;color:#b9c0cc}</style></head><body><main class=\"shell\"><section class=\"card\"><p class=\"eyebrow\">Axonyx UI Streaming Probe</p><h1>Shell arrived first.</h1>".to_vec(),
            b"<p class=\"chunk\">Then the streamed content chunk arrived through <code>Transfer-Encoding: chunked</code>.</p>".to_vec(),
            b"<p class=\"chunk\">This is still a dev probe, but it proves the server can send HTML in pieces.</p></section></main></body></html>".to_vec(),
        ],
    )
    .with_no_store()
}

fn sse_probe_response() -> AxHttpResponse {
    AxHttpResponse::sse_events([
        AxSseEvent::named("axonyx", r#"{"phase":"start"}"#).with_id("1"),
        AxSseEvent::named("patch", r#"{"scope":"page","patches":[]}"#).with_id("2"),
        AxSseEvent::named("axonyx", r#"{"phase":"end"}"#).with_id("3"),
    ])
}

fn read_http_request(
    stream: &mut TcpStream,
    max_body_bytes: usize,
) -> Result<Option<AxHttpRequest>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;

    loop {
        let read = stream
            .read(&mut chunk)
            .context("failed to read request from dev client")?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            break;
        }

        buffer.extend_from_slice(&chunk[..read]);

        if header_end.is_none() {
            header_end = find_header_end(&buffer);
        }

        if let Some(end) = header_end {
            let header_text = String::from_utf8_lossy(&buffer[..end]);
            let content_length = parse_content_length(&header_text);
            if content_length > max_body_bytes {
                break;
            }
            let total = end + 4 + content_length;
            if buffer.len() >= total {
                break;
            }
        }
    }

    let Some(header_end) = find_header_end(&buffer) else {
        return Ok(None);
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let Some(request_line) = lines.next() else {
        return Ok(None);
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let body_start = header_end + 4;
    let body = buffer[body_start..].to_vec();

    Ok(Some(AxHttpRequest {
        method,
        target,
        headers,
        body,
    }))
}

#[cfg(test)]
async fn read_http_request_async(
    stream: &mut tokio::net::TcpStream,
    max_body_bytes: usize,
    request_timeout: Duration,
) -> Result<Option<AxHttpRequest>> {
    let read = tokio::time::timeout(request_timeout, async {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let mut header_end = None;

        loop {
            let read = stream
                .read(&mut chunk)
                .await
                .context("failed to read request from async dev client")?;
            if read == 0 {
                if buffer.is_empty() {
                    return Ok(None);
                }
                break;
            }

            buffer.extend_from_slice(&chunk[..read]);

            if header_end.is_none() {
                header_end = find_header_end(&buffer);
            }

            if let Some(end) = header_end {
                let header_text = String::from_utf8_lossy(&buffer[..end]);
                let content_length = parse_content_length(&header_text);
                if content_length > max_body_bytes {
                    break;
                }
                let total = end + 4 + content_length;
                if buffer.len() >= total {
                    break;
                }
            }
        }

        parse_http_request_buffer(&buffer)
    })
    .await
    .context("timed out reading request from async dev client")?;

    read
}

#[cfg(test)]
fn parse_http_request_buffer(buffer: &[u8]) -> Result<Option<AxHttpRequest>> {
    let Some(header_end) = find_header_end(buffer) else {
        return Ok(None);
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let Some(request_line) = lines.next() else {
        return Ok(None);
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let body_start = header_end + 4;
    let body = buffer[body_start..].to_vec();

    Ok(Some(AxHttpRequest {
        method,
        target,
        headers,
        body,
    }))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().ok();
            }
            None
        })
        .unwrap_or(0)
}

fn request_content_length(request: &AxHttpRequest) -> Option<usize> {
    request
        .headers
        .get("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok())
}

fn request_body_exceeds_limit(request: &AxHttpRequest, max_body_bytes: usize) -> bool {
    request.body.len() > max_body_bytes
        || request_content_length(request).is_some_and(|length| length > max_body_bytes)
}

fn configured_max_request_body_bytes(root: &Path) -> std::result::Result<usize, String> {
    match axonyx_config_value(root, "server", "max_body_bytes") {
        Some(value) => parse_max_body_bytes_value(&value),
        None => Ok(MAX_REQUEST_BODY_BYTES),
    }
}

fn configured_request_timeout_duration(root: &Path) -> std::result::Result<Duration, String> {
    match axonyx_config_value(root, "server", "request_timeout_seconds") {
        Some(value) => parse_request_timeout_seconds_value(&value),
        None => Ok(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS)),
    }
}

fn configured_shutdown_grace_duration(root: &Path) -> std::result::Result<Duration, String> {
    match axonyx_config_value(root, "server", "shutdown_grace_seconds") {
        Some(value) => parse_shutdown_grace_seconds_value(&value),
        None => Ok(Duration::from_secs(DEFAULT_SHUTDOWN_GRACE_SECONDS)),
    }
}

fn configured_max_connections(root: &Path) -> std::result::Result<usize, String> {
    match axonyx_config_value(root, "server", "max_connections") {
        Some(value) => parse_max_connections_value(&value),
        None => Ok(DEFAULT_MAX_CONNECTIONS),
    }
}

fn configured_server_bool(
    root: &Path,
    key: &str,
    default: bool,
) -> std::result::Result<bool, String> {
    match axonyx_config_value(root, "server", key) {
        Some(value) => parse_bool_config_value(&value)
            .map_err(|_| format!("[server].{key} must be a boolean.")),
        None => Ok(default),
    }
}

fn configured_server_log_format(root: &Path) -> std::result::Result<AxServerLogFormat, String> {
    match axonyx_config_value(root, "server", "log_format") {
        Some(value) => parse_server_log_format_value(&value),
        None => parse_server_log_format_str(DEFAULT_LOG_FORMAT),
    }
}

fn parse_bool_config_value(value: &toml::Value) -> std::result::Result<bool, String> {
    match value {
        toml::Value::Boolean(value) => Ok(*value),
        toml::Value::String(value) => {
            parse_boolish_strict(value).ok_or_else(|| "expected a boolean-like string".to_string())
        }
        _ => Err("expected a boolean".to_string()),
    }
}

fn parse_server_log_format_value(
    value: &toml::Value,
) -> std::result::Result<AxServerLogFormat, String> {
    match value {
        toml::Value::String(value) => parse_server_log_format_str(value),
        _ => Err("[server].log_format must be a string.".to_string()),
    }
}

fn parse_server_log_format_str(value: &str) -> std::result::Result<AxServerLogFormat, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "text" => Ok(AxServerLogFormat::Text),
        "json" => Ok(AxServerLogFormat::Json),
        _ => Err("[server].log_format must be \"text\" or \"json\".".to_string()),
    }
}

fn parse_request_timeout_seconds_value(
    value: &toml::Value,
) -> std::result::Result<Duration, String> {
    match value {
        toml::Value::Integer(number) if *number > 0 => Ok(Duration::from_secs(*number as u64)),
        toml::Value::Integer(_) => {
            Err("[server].request_timeout_seconds must be positive.".to_string())
        }
        _ => Err("[server].request_timeout_seconds must be an integer.".to_string()),
    }
}

fn parse_shutdown_grace_seconds_value(
    value: &toml::Value,
) -> std::result::Result<Duration, String> {
    match value {
        toml::Value::Integer(number) if *number > 0 => Ok(Duration::from_secs(*number as u64)),
        toml::Value::Integer(_) => {
            Err("[server].shutdown_grace_seconds must be positive.".to_string())
        }
        _ => Err("[server].shutdown_grace_seconds must be an integer.".to_string()),
    }
}

fn parse_max_connections_value(value: &toml::Value) -> std::result::Result<usize, String> {
    match value {
        toml::Value::Integer(number) if *number > 0 => Ok(*number as usize),
        toml::Value::Integer(_) => Err("[server].max_connections must be positive.".to_string()),
        _ => Err("[server].max_connections must be an integer.".to_string()),
    }
}

fn parse_max_body_bytes_value(value: &toml::Value) -> std::result::Result<usize, String> {
    match value {
        toml::Value::Integer(number) if *number > 0 => Ok(*number as usize),
        toml::Value::Integer(_) => Err("[server].max_body_bytes must be positive.".to_string()),
        toml::Value::String(value) => parse_byte_size(value),
        _ => Err("[server].max_body_bytes must be an integer or size string.".to_string()),
    }
}

fn parse_byte_size(value: &str) -> std::result::Result<usize, String> {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err("empty byte size".to_string());
    }

    let digit_len = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digit_len == 0 {
        return Err(format!("invalid byte size '{value}'"));
    }

    let number = trimmed[..digit_len]
        .parse::<usize>()
        .map_err(|_| format!("invalid byte size '{value}'"))?;
    if number == 0 {
        return Err("[server].max_body_bytes must be positive.".to_string());
    }

    let unit = trimmed[digit_len..].trim();
    let multiplier = match unit {
        "" | "b" => 1,
        "kb" | "kib" => 1024,
        "mb" | "mib" => 1024 * 1024,
        "gb" | "gib" => 1024 * 1024 * 1024,
        _ => return Err(format!("unsupported byte size unit '{unit}'")),
    };

    number
        .checked_mul(multiplier)
        .ok_or_else(|| format!("byte size '{value}' is too large"))
}

fn format_bytes(bytes: usize) -> String {
    const GIB: usize = 1024 * 1024 * 1024;
    const MIB: usize = 1024 * 1024;
    const KIB: usize = 1024;

    if bytes % GIB == 0 {
        format!("{} GiB", bytes / GIB)
    } else if bytes % MIB == 0 {
        format!("{} MiB", bytes / MIB)
    } else if bytes % KIB == 0 {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

fn handle_action_request(
    state: &DevServerState,
    request: &AxHttpRequest,
) -> Result<AxHttpResponse> {
    let content_type = request
        .headers
        .get("content-type")
        .map(String::as_str)
        .unwrap_or("");
    if !content_type.starts_with("application/x-www-form-urlencoded") {
        return Ok(
            AxHttpResponse::text(415, "expected application/x-www-form-urlencoded").with_no_store(),
        );
    }

    let request_path =
        extract_action_query_param(&request.target, "path").unwrap_or_else(|| "/".to_string());
    let action_name = extract_action_query_param(&request.target, "name").unwrap_or_default();
    if action_name.is_empty() {
        return Ok(AxHttpResponse::text(400, "missing action name").with_no_store());
    }

    let Some(route) = resolve_route(&state.root, &request_path)? else {
        return Ok(AxHttpResponse::text(404, "route not found").with_no_store());
    };

    let Some(actions_path) = &route.actions_path else {
        return Ok(AxHttpResponse::text(404, "actions.ax not found for route").with_no_store());
    };

    let action_source = fs::read_to_string(actions_path)
        .with_context(|| format!("failed to read '{}'", actions_path.display()))?;
    let input_fields = parse_form_body(&request.body);
    let mut store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;
    let result = execute_preview_action_sources(
        &[action_source.as_str()],
        &action_name,
        &input_fields,
        &mut store,
    )
    .with_context(|| {
        format!(
            "failed to execute action '{}' from '{}'",
            action_name,
            actions_path.display()
        )
    })?;

    if wants_action_patch_response(request, &input_fields) {
        return action_patch_response(&route, &result);
    }

    let redirect_to = result.redirect_to.unwrap_or(route.request_path);
    Ok(redirect_response(303, &redirect_to))
}

fn wants_action_patch_response(
    request: &AxHttpRequest,
    input_fields: &std::collections::BTreeMap<String, String>,
) -> bool {
    input_fields
        .get("__ax_patch")
        .is_some_and(|value| parse_boolish(value))
        || request
            .headers
            .get("accept")
            .is_some_and(|value| value.contains("application/ax-patch+json"))
}

fn action_patch_response(
    route: &ResolvedRoute,
    result: &AxPreviewActionResult,
) -> Result<AxHttpResponse> {
    let patches = normalize_action_patches(route, &result.patches)?;
    validate_action_patches(route, &patches)?;

    let redirect_to = result
        .redirect_to
        .clone()
        .unwrap_or_else(|| route.request_path.clone());
    let body = serde_json::to_vec(&serde_json::json!({
        "ok": true,
        "redirect": redirect_to,
        "value": ax_value_to_json(&result.value),
        "patches": patches.iter().map(state_patch_to_json).collect::<Vec<_>>(),
    }))
    .context("failed to serialize action patch response")?;

    Ok(
        AxHttpResponse::bytes(200, "application/ax-patch+json; charset=utf-8", body)
            .with_no_store(),
    )
}

fn normalize_action_patches(
    route: &ResolvedRoute,
    patches: &[AxPreviewStatePatch],
) -> Result<Vec<AxPreviewStatePatch>> {
    if patches.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = collect_route_state_manifest(route)?;
    Ok(patches
        .iter()
        .map(|patch| {
            let mut patch = patch.clone();
            if let Some(signal) = manifest.resolve_signal_key(&patch.signal) {
                patch.signal = signal;
            }
            patch
        })
        .collect())
}

fn validate_action_patches(route: &ResolvedRoute, patches: &[AxPreviewStatePatch]) -> Result<()> {
    if patches.is_empty() {
        return Ok(());
    }

    let manifest = collect_route_state_manifest(route)?;
    if manifest.is_empty() {
        return Ok(());
    }

    for patch in patches {
        let Some(expected_ty) = manifest.signal_types.get(&patch.signal) else {
            continue;
        };
        if state_patch_value_matches_type(&patch.value, expected_ty) {
            continue;
        }

        bail!(
            "state patch for '{}' expected {} but got {}",
            patch.signal,
            expected_ty,
            ax_value_type_name(&patch.value)
        );
    }

    Ok(())
}

fn collect_route_state_manifest(route: &ResolvedRoute) -> Result<RouteStateManifest> {
    let mut signals = RouteStateManifest::default();

    for path in route
        .layout_paths
        .iter()
        .chain(std::iter::once(&route.page_path))
    {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read '{}'", path.display()))?;
        if !source_has_state_declaration(&source) {
            continue;
        }

        let file = parse_ax_v2(&source).with_context(|| {
            format!(
                "failed to parse state declarations from '{}'",
                path.display()
            )
        })?;
        let app_root = app_root_for_app_path(&route.page_path).unwrap_or_else(|| {
            route
                .page_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf()
        });
        let root = app_root.parent().unwrap_or_else(|| Path::new(""));
        let scope = state_scope_for_path(root, path);
        let manifest =
            build_state_manifest_with_scope_mapper(&file, &scope, |state, default_scope| {
                scoped_state_decl_scope(root, path, state.scope.as_deref())
                    .unwrap_or_else(|| default_scope.to_string())
            })
            .with_context(|| format!("failed to build state manifest for '{}'", path.display()))?;

        for signal in manifest.signals {
            let legacy_key = format!("root:{}:{}", signal.name, signal.id.index);
            signals.insert(signal.name, signal.key, legacy_key, signal.ty);
        }
    }

    Ok(signals)
}

#[derive(Debug, Default)]
struct RouteStateManifest {
    signal_types: std::collections::BTreeMap<String, String>,
    aliases: std::collections::BTreeMap<String, String>,
}

impl RouteStateManifest {
    fn insert(&mut self, name: String, key: String, legacy_key: String, ty: String) {
        self.signal_types.insert(key.clone(), ty.clone());
        self.signal_types.insert(legacy_key.clone(), ty);
        self.aliases.entry(name).or_insert_with(|| key.clone());
        self.aliases.insert(legacy_key, key.clone());
        self.aliases.insert(key.clone(), key);
    }

    fn is_empty(&self) -> bool {
        self.signal_types.is_empty()
    }

    fn resolve_signal_key(&self, signal: &str) -> Option<String> {
        self.aliases.get(signal).cloned()
    }
}

fn state_patch_value_matches_type(value: &AxValue, expected_ty: &str) -> bool {
    match expected_ty {
        "String" => matches!(value, AxValue::String(_)),
        "Number" => matches!(value, AxValue::Number(_)),
        "Bool" => matches!(value, AxValue::Bool(_)),
        "Unknown" => true,
        _ => true,
    }
}

fn ax_value_type_name(value: &AxValue) -> &'static str {
    match value {
        AxValue::Null => "Null",
        AxValue::String(_) => "String",
        AxValue::Number(_) => "Number",
        AxValue::Bool(_) => "Bool",
        AxValue::Record(_) => "Record",
        AxValue::List(_) => "List",
    }
}

fn state_patch_to_json(patch: &AxPreviewStatePatch) -> serde_json::Value {
    serde_json::json!({
        "op": patch.op,
        "signal": patch.signal,
        "value": ax_value_to_json(&patch.value),
        "source": patch.source,
    })
}

fn ax_value_to_json(value: &AxValue) -> serde_json::Value {
    match value {
        AxValue::Null => serde_json::Value::Null,
        AxValue::String(value) => serde_json::Value::String(value.clone()),
        AxValue::Number(value) => serde_json::Value::Number((*value).into()),
        AxValue::Bool(value) => serde_json::Value::Bool(*value),
        AxValue::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), ax_value_to_json(value)))
                .collect(),
        ),
        AxValue::List(items) => {
            serde_json::Value::Array(items.iter().map(ax_value_to_json).collect())
        }
    }
}

fn parse_form_body(body: &[u8]) -> std::collections::BTreeMap<String, String> {
    let text = String::from_utf8_lossy(body);
    let mut fields = std::collections::BTreeMap::new();

    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        fields.insert(url_decode(key), url_decode(value));
    }

    fields
}

fn redirect_response(status: u16, location: &str) -> AxHttpResponse {
    AxHttpResponse::redirect_with_status(status, location).with_no_store()
}

fn cacheable_asset_response(asset: StaticAsset) -> AxHttpResponse {
    AxHttpResponse::bytes(200, asset.content_type, asset.body)
        .with_header("Cache-Control", IMMUTABLE_ASSET_CACHE_CONTROL)
}

fn load_public_asset(root: &Path, request_path: &str) -> Result<Option<StaticAsset>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    if segments.is_empty() {
        return Ok(None);
    }

    let asset_path = segments
        .iter()
        .fold(root.join("public"), |current, segment| {
            current.join(segment)
        });

    if !asset_path.exists() || !asset_path.is_file() {
        return Ok(None);
    }

    let body = fs::read(&asset_path)
        .with_context(|| format!("failed to read asset '{}'", asset_path.display()))?;

    Ok(Some(StaticAsset {
        content_type: content_type_for(&asset_path),
        body,
    }))
}

fn load_package_asset(root: &Path, request_path: &str) -> Result<Option<StaticAsset>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    if segments.len() < 4 || segments[0] != "_ax" || segments[1] != "pkg" {
        return Ok(None);
    }

    let package_name = &segments[2];
    let relative = segments[3..].join("/");
    let Some(package_root) = resolve_package_asset_root(root, package_name) else {
        return Ok(None);
    };
    let Some(asset_path) = package_asset_path(&package_root, &relative) else {
        return Ok(None);
    };

    if !asset_path.exists() || !asset_path.is_file() {
        return Ok(None);
    }

    let body = fs::read(&asset_path)
        .with_context(|| format!("failed to read package asset '{}'", asset_path.display()))?;

    Ok(Some(StaticAsset {
        content_type: content_type_for(&asset_path),
        body,
    }))
}

fn resolve_package_asset_root(root: &Path, package_name: &str) -> Option<PathBuf> {
    if package_name == AXONYX_UI_PACKAGE_NAME {
        let app_vendor = root.join("vendor").join("axonyx-ui");
        if app_vendor.exists() {
            return Some(app_vendor);
        }

        return cargo_package_root(root, package_name)
            .or_else(|| axonyx_ui_workspace_package_root(root));
    }

    cargo_package_root(root, package_name)
}

fn axonyx_ui_workspace_package_root(root: &Path) -> Option<PathBuf> {
    let workspace_root = root.parent()?;
    [
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui"),
        workspace_root.join("axonyx-ui"),
    ]
    .into_iter()
    .find(|package_root| package_root.exists())
}

fn axonyx_ui_workspace_import_bases(root: &Path) -> Vec<PathBuf> {
    let Some(workspace_root) = root.parent() else {
        return Vec::new();
    };

    vec![
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui")
            .join("src")
            .join("ax"),
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui")
            .join("src"),
        workspace_root.join("axonyx-ui").join("src").join("ax"),
        workspace_root.join("axonyx-ui").join("src"),
    ]
}

fn axonyx_ui_app_import_bases(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("vendor").join("axonyx-ui").join("src").join("ax"),
        root.join("vendor").join("axonyx-ui").join("src"),
    ]
}

fn package_asset_path(package_root: &Path, relative: &str) -> Option<PathBuf> {
    let relative_path = safe_relative_path(relative)?;
    let css_root = package_css_root(package_root);
    let css_entry = package_css_entry(package_root);
    let js_root = package_js_root(package_root);
    let js_entry = package_js_entry(package_root);

    if relative_path.components().count() == 1
        && css_entry.file_name().is_some_and(|file_name| {
            file_name == relative_path.as_os_str()
                || is_hashed_entry_file_name(&relative_path, file_name)
        })
    {
        return Some(css_entry);
    }

    if relative_path.components().count() == 2
        && relative_path.starts_with("js")
        && js_entry.file_name().is_some_and(|file_name| {
            relative_path.file_name().is_some_and(|relative_file| {
                file_name == relative_file
                    || is_hashed_entry_file_name(Path::new(relative_file), file_name)
            })
        })
    {
        return Some(js_entry);
    }

    if let Ok(js_relative) = relative_path.strip_prefix("js") {
        return Some(js_root.join(js_relative));
    }

    Some(css_root.join(relative_path))
}

fn is_hashed_entry_file_name(relative_path: &Path, entry_file_name: &std::ffi::OsStr) -> bool {
    let Some(relative_name) = relative_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(entry_name) = entry_file_name.to_str() else {
        return false;
    };
    let Some((stem, extension)) = entry_name.rsplit_once('.') else {
        return false;
    };

    let prefix = format!("{stem}.");
    let suffix = format!(".{extension}");
    if !relative_name.starts_with(&prefix) || !relative_name.ends_with(&suffix) {
        return false;
    }

    let hash = &relative_name[prefix.len()..relative_name.len() - suffix.len()];
    hash.len() == 12 && hash.chars().all(|character| character.is_ascii_hexdigit())
}

fn hashed_asset_file_name(entry: &Path) -> Result<Option<OsString>> {
    if !entry.exists() || !entry.is_file() {
        return Ok(None);
    }

    let Some(stem) = entry.file_stem().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    let Some(extension) = entry.extension().and_then(|value| value.to_str()) else {
        return Ok(None);
    };

    let body = fs::read(entry).with_context(|| {
        format!(
            "failed to read package asset '{}' for hashing",
            entry.display()
        )
    })?;
    let hash = short_content_hash(&body);
    Ok(Some(OsString::from(format!("{stem}.{hash}.{extension}"))))
}

fn short_content_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}").chars().take(12).collect()
}

fn package_css_root(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "css_root")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("css"))
}

fn package_css_entry(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "css_entry")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("css").join("index.css"))
}

fn package_js_root(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "js_root")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("js"))
}

fn package_js_entry(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "js_entry")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("js").join("index.js"))
}

fn package_metadata_export(package_root: &Path, key: &str) -> Option<String> {
    let source = fs::read_to_string(package_root.join("Axonyx.package.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    value
        .get("exports")
        .and_then(|exports| exports.get(key))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn safe_relative_path(relative: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

fn resolve_route(root: &Path, request_path: &str) -> Result<Option<ResolvedRoute>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    let app_root = root.join("app");
    let Some((page_dir, matched_dirs, params)) = resolve_app_route_dir(&app_root, &segments)?
    else {
        return Ok(None);
    };
    let page_path = page_dir.join("page.ax");

    let mut layout_paths = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layout_paths.push(root_layout);
    }

    let mut current = app_root;
    for segment in &matched_dirs {
        current = current.join(segment);
        let layout_path = current.join("layout.ax");
        if layout_path.exists() {
            layout_paths.push(layout_path);
        }
    }

    let loader_path = page_path
        .parent()
        .map(|parent| parent.join("loader.ax"))
        .filter(|path| path.exists());
    let actions_path = page_path
        .parent()
        .map(|parent| parent.join("actions.ax"))
        .filter(|path| path.exists());

    Ok(Some(ResolvedRoute {
        request_path: normalized,
        request_target: request_path.to_string(),
        page_path,
        layout_paths,
        loader_path,
        actions_path,
        params,
    }))
}

fn resolve_boundary_route(
    root: &Path,
    file_name: &str,
    request_path: &str,
) -> Option<ResolvedRoute> {
    let app_root = root.join("app");
    let page_path = app_root.join(file_name);
    if !page_path.exists() {
        return None;
    }

    let mut layout_paths = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layout_paths.push(root_layout);
    }

    Some(ResolvedRoute {
        request_path: normalize_request_path(request_path).unwrap_or_else(|_| "/".to_string()),
        request_target: request_path.to_string(),
        page_path,
        layout_paths,
        loader_path: None,
        actions_path: None,
        params: std::collections::BTreeMap::new(),
    })
}

fn resolve_app_route_dir(
    app_root: &Path,
    segments: &[String],
) -> Result<
    Option<(
        PathBuf,
        Vec<String>,
        std::collections::BTreeMap<String, String>,
    )>,
> {
    resolve_app_route_dir_from(
        app_root,
        segments,
        Vec::new(),
        std::collections::BTreeMap::new(),
    )
}

fn resolve_app_route_dir_from(
    current: &Path,
    segments: &[String],
    matched_dirs: Vec<String>,
    params: std::collections::BTreeMap<String, String>,
) -> Result<
    Option<(
        PathBuf,
        Vec<String>,
        std::collections::BTreeMap<String, String>,
    )>,
> {
    if segments.is_empty() {
        if current.join("page.ax").exists() {
            return Ok(Some((current.to_path_buf(), matched_dirs, params)));
        }
        return Ok(None);
    }

    let segment = &segments[0];
    let exact_dir = current.join(segment);
    if exact_dir.is_dir() {
        let mut exact_dirs = matched_dirs.clone();
        exact_dirs.push(segment.clone());
        if let Some(found) =
            resolve_app_route_dir_from(&exact_dir, &segments[1..], exact_dirs, params.clone())?
        {
            return Ok(Some(found));
        }
    }

    let mut dynamic_dirs = fs::read_dir(current)
        .with_context(|| format!("failed to read directory '{}'", current.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let param_name = parse_dynamic_app_segment(&name)?.to_string();
            Some((name, param_name, path))
        })
        .collect::<Vec<_>>();
    dynamic_dirs.sort_by(|left, right| left.0.cmp(&right.0));

    for (dir_name, param_name, dir_path) in dynamic_dirs {
        let mut next_dirs = matched_dirs.clone();
        next_dirs.push(dir_name);
        let mut next_params = params.clone();
        next_params.insert(param_name, segment.clone());
        if let Some(found) =
            resolve_app_route_dir_from(&dir_path, &segments[1..], next_dirs, next_params)?
        {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

fn parse_dynamic_app_segment(segment: &str) -> Option<&str> {
    segment
        .strip_prefix('[')?
        .strip_suffix(']')
        .filter(|name| !name.is_empty())
}

fn render_route_html(state: &DevServerState, route: &ResolvedRoute) -> Result<String> {
    let page_source = fs::read_to_string(&route.page_path)
        .with_context(|| format!("failed to read '{}'", route.page_path.display()))?;
    let layout_sources = route
        .layout_paths
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let layout_refs = layout_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let loader_sources = route
        .loader_path
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let loader_refs = loader_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let action_sources = route
        .actions_path
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let action_refs = action_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;
    let import_resolver = |source: &str| load_preview_import_source(&state.root, source);

    let html = preview_ax_route_with_request_context_and_imports(
        &layout_refs,
        &loader_refs,
        &action_refs,
        &page_source,
        &route.request_target,
        &route.params,
        &store,
        &import_resolver,
    )
    .with_context(|| {
        format!(
            "failed to render route '{}' from '{}'",
            route.request_path,
            route.page_path.display()
        )
    })?;

    let html = apply_package_use_assets(&state.root, html, &layout_refs, &page_source);
    Ok(apply_theme_config(&state.root, html))
}

fn apply_package_use_assets(
    root: &Path,
    html: String,
    layout_sources: &[&str],
    page_source: &str,
) -> String {
    let uses_axonyx_ui = layout_sources
        .iter()
        .any(|source| source_uses_package(source, "@axonyx/ui"))
        || source_uses_package(page_source, "@axonyx/ui");

    if !uses_axonyx_ui {
        return html;
    }

    let package_available = resolve_package_asset_root(root, AXONYX_UI_PACKAGE_NAME).is_some()
        || load_package_asset(root, AXONYX_UI_STYLESHEET_HREF)
            .ok()
            .flatten()
            .is_some();
    if !package_available {
        return html;
    }

    let (stylesheet_href, script_href) = axonyx_ui_asset_hrefs(root);
    let html = ensure_head_stylesheet(&html, &stylesheet_href);
    ensure_head_script(&html, &script_href)
}

fn axonyx_ui_asset_hrefs(root: &Path) -> (String, String) {
    let Some(package_root) = resolve_package_asset_root(root, AXONYX_UI_PACKAGE_NAME) else {
        return (
            AXONYX_UI_STYLESHEET_HREF.to_string(),
            AXONYX_UI_SCRIPT_HREF.to_string(),
        );
    };

    let stylesheet = hashed_package_asset_href(
        AXONYX_UI_PACKAGE_NAME,
        "",
        &package_css_entry(&package_root),
        AXONYX_UI_STYLESHEET_HREF,
    );
    let script = hashed_package_asset_href(
        AXONYX_UI_PACKAGE_NAME,
        "js",
        &package_js_entry(&package_root),
        AXONYX_UI_SCRIPT_HREF,
    );

    (stylesheet, script)
}

fn hashed_package_asset_href(
    package_name: &str,
    prefix: &str,
    entry: &Path,
    fallback: &str,
) -> String {
    let Some(file_name) = hashed_asset_file_name(entry).ok().flatten() else {
        return fallback.to_string();
    };

    if prefix.is_empty() {
        format!("/_ax/pkg/{package_name}/{}", file_name.to_string_lossy())
    } else {
        format!(
            "/_ax/pkg/{package_name}/{prefix}/{}",
            file_name.to_string_lossy()
        )
    }
}

fn source_uses_package(source: &str, package: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == format!("use \"{package}\"") || trimmed == format!("use '{package}'")
    })
}

fn render_route_response(
    state: &DevServerState,
    route: &ResolvedRoute,
    inject_dev_client_script: bool,
    stream_response: bool,
) -> Result<AxHttpResponse> {
    render_route_response_with_status(state, route, 200, inject_dev_client_script, stream_response)
}

fn render_route_response_with_status(
    state: &DevServerState,
    route: &ResolvedRoute,
    status: u16,
    inject_dev_client_script: bool,
    stream_response: bool,
) -> Result<AxHttpResponse> {
    let mut html = render_route_html(state, route)?;
    if inject_dev_client_script {
        html = inject_dev_client(&html, &route.request_path);
    }
    if stream_response {
        return Ok(AxHttpResponse::stream_chunks(
            status,
            "text/html; charset=utf-8",
            html_stream_chunks(&html),
        )
        .with_no_store());
    }
    Ok(AxHttpResponse::html(status, html).with_no_store())
}

fn render_not_found_response(
    state: &DevServerState,
    request_target: &str,
    inject_dev_client_script: bool,
    stream_response: bool,
) -> Result<AxHttpResponse> {
    if let Some(route) = resolve_boundary_route(&state.root, "not-found.ax", request_target) {
        return render_route_response_with_status(
            state,
            &route,
            404,
            inject_dev_client_script,
            stream_response,
        );
    }

    let html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Axonyx 404</title></head><body><h1>Route not found</h1><p>No <code>page.ax</code> matched <code>{}</code>.</p></body></html>",
        html_escape(request_target)
    );
    Ok(AxHttpResponse::html(404, html).with_no_store())
}

fn render_error_response(
    state: &DevServerState,
    request_target: &str,
    error: &anyhow::Error,
    inject_dev_client_script: bool,
    stream_response: bool,
) -> Result<AxHttpResponse> {
    if let Some(route) = resolve_boundary_route(&state.root, "error.ax", request_target) {
        match render_route_response_with_status(
            state,
            &route,
            500,
            inject_dev_client_script,
            stream_response,
        ) {
            Ok(response) => return Ok(response),
            Err(boundary_error) => {
                eprintln!("Axonyx error boundary failed: {boundary_error:#}");
            }
        }
    }

    let html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Axonyx 500</title></head><body><h1>Application error</h1><p>Axonyx could not render <code>{}</code>.</p><pre>{}</pre></body></html>",
        html_escape(request_target),
        html_escape(&error.to_string())
    );
    Ok(AxHttpResponse::html(500, html).with_no_store())
}

fn should_stream_page_route(root: &Path, target: &str) -> bool {
    query_param_value(target, "__ax_stream")
        .map(parse_boolish)
        .unwrap_or_else(|| axonyx_config_bool(root, "server", "stream_pages").unwrap_or(false))
}

fn html_stream_chunks(html: &str) -> Vec<Vec<u8>> {
    if let Some(body_start) = html.find("<body") {
        if let Some(open_end) = html[body_start..].find('>') {
            let split = body_start + open_end + 1;
            if let Some(body_end) = html[split..].rfind("</body>") {
                let body_end = split + body_end;
                return vec![
                    html[..split].as_bytes().to_vec(),
                    html[split..body_end].as_bytes().to_vec(),
                    html[body_end..].as_bytes().to_vec(),
                ];
            }
        }
    }

    vec![html.as_bytes().to_vec()]
}

fn route_version(root: &Path, route: &ResolvedRoute) -> Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut visited = std::collections::BTreeSet::new();
    route.request_path.hash(&mut hasher);
    let config_path = root.join("Axonyx.toml");
    if config_path.exists() {
        hash_file(&config_path, &mut hasher)?;
    }

    hash_ax_file_with_imports(root, &route.page_path, &mut hasher, &mut visited)?;
    for path in &route.layout_paths {
        hash_ax_file_with_imports(root, path, &mut hasher, &mut visited)?;
    }
    if let Some(path) = &route.loader_path {
        hash_file(path, &mut hasher)?;
    }
    if let Some(path) = &route.actions_path {
        hash_file(path, &mut hasher)?;
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn apply_theme_config(root: &Path, html: String) -> String {
    let Some(theme_table) = axonyx_config_table(root, "theme") else {
        return html;
    };

    let mut html = html;

    if let Some(active) = theme_table
        .get("active")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        html = ensure_html_theme_attr(&html, active.trim());
    }

    if let Some(stylesheet) = theme_table
        .get("stylesheet")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        html = ensure_head_stylesheet(&html, stylesheet.trim());
    }

    html
}

fn ensure_html_theme_attr(html: &str, theme: &str) -> String {
    if html.contains("data-theme=") {
        return html.to_string();
    }

    html.replacen(
        "<html",
        &format!("<html data-theme=\"{}\"", html_escape(theme)),
        1,
    )
}

fn ensure_head_stylesheet(html: &str, stylesheet: &str) -> String {
    if html.contains(stylesheet) {
        return html.to_string();
    }

    let tag = format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        html_escape(stylesheet)
    );

    if html.contains("</head>") {
        return html.replacen("</head>", &format!("{tag}</head>"), 1);
    }

    html.to_string()
}

fn ensure_head_script(html: &str, script: &str) -> String {
    if html.contains(script) {
        return html.to_string();
    }

    let tag = format!(
        "<script src=\"{}\" defer=\"true\"></script>",
        html_escape(script)
    );

    if html.contains("</head>") {
        return html.replacen("</head>", &format!("{tag}</head>"), 1);
    }

    html.to_string()
}

fn hash_file(path: &Path, hasher: &mut impl Hasher) -> Result<()> {
    path.to_string_lossy().hash(hasher);
    let contents = fs::read(path)
        .with_context(|| format!("failed to read '{}' for hashing", path.display()))?;
    contents.hash(hasher);
    Ok(())
}

fn hash_ax_file_with_imports(
    root: &Path,
    path: &Path,
    hasher: &mut impl Hasher,
    visited: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }

    hash_file(path, hasher)?;

    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))?;
    let Ok(document) = parse_ax_auto(&source) else {
        return Ok(());
    };

    for import_decl in document.imports {
        if let Some(import_path) = resolve_preview_import_path(root, &import_decl.source) {
            if import_path.exists() {
                hash_ax_file_with_imports(root, &import_path, hasher, visited)?;
            }
        }
    }

    Ok(())
}

fn load_preview_import_source(root: &Path, source: &str) -> Option<String> {
    let path = resolve_preview_import_path(root, source)?;
    fs::read_to_string(path).ok()
}

fn resolve_preview_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    resolve_component_override_import_path(root, source)
        .or_else(|| resolve_package_override_import_path(root, source))
        .or_else(|| resolve_app_import_path(root, source))
        .or_else(|| resolve_axonyx_ui_app_import_path(root, source))
        .or_else(|| resolve_cargo_package_import_path(root, source))
        .or_else(|| resolve_axonyx_ui_workspace_import_path(root, source))
}

fn resolve_app_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@/")?;
    let mut path = root.join("app");

    for segment in relative.split('/') {
        if !segment.is_empty() {
            path.push(segment);
        }
    }

    if path.extension().is_none() {
        path.set_extension("ax");
    }

    Some(path)
}

fn resolve_component_override_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let target = axonyx_config_string(root, "component_overrides", source)?;
    resolve_config_path(root, &target)
}

fn resolve_package_override_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let overrides = axonyx_config_table(root, "package_overrides")?;
    let mut matches = overrides
        .iter()
        .filter_map(|(package, value)| {
            let target = value.as_str()?;
            let relative = source
                .strip_prefix(package)
                .and_then(|rest| rest.strip_prefix('/'))?;
            Some((package.len(), target, relative))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| right.0.cmp(&left.0));
    let (_, target, relative) = matches.into_iter().next()?;

    let mut base = resolve_config_base_path(root, target)?;
    let ax_root = base.join("src").join("ax");
    let src_root = base.join("src");
    if ax_root.exists() {
        base = ax_root;
    } else if src_root.exists() {
        base = src_root;
    }

    Some(join_import_relative(base, relative))
}

fn resolve_cargo_package_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let (namespace, relative) = split_package_import(source)?;
    let package_name = cargo_package_name_for_namespace(root, namespace)?;
    let package_root = cargo_package_root(root, &package_name)?;
    let ax_root = cargo_package_ax_root(&package_root, namespace)
        .unwrap_or_else(|| default_package_ax_root(&package_root));

    Some(join_import_relative(ax_root, relative))
}

fn split_package_import(source: &str) -> Option<(&str, &str)> {
    let mut parts = source.splitn(3, '/');
    let scope = parts.next()?;
    if !scope.starts_with('@') {
        return None;
    }

    let package = parts.next()?;
    let relative = parts.next()?;
    let namespace_len = scope.len() + 1 + package.len();

    Some((&source[..namespace_len], relative))
}

fn cargo_package_name_for_namespace(root: &Path, namespace: &str) -> Option<String> {
    if let Some(value) = axonyx_config_value(root, "packages", namespace) {
        if let Some(package_name) = value.as_str() {
            return Some(package_name.to_string());
        }

        if let Some(package_name) = value
            .as_table()
            .and_then(|table| table.get("crate"))
            .and_then(|value| value.as_str())
        {
            return Some(package_name.to_string());
        }
    }

    match namespace {
        "@axonyx/ui" => Some("axonyx-ui".to_string()),
        _ => None,
    }
}

fn cargo_package_root(root: &Path, package_name: &str) -> Option<PathBuf> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.exists() {
        return None;
    }

    let cache_key = format!("{}::{package_name}", manifest_path.to_string_lossy());
    let cache =
        CARGO_PACKAGE_ROOT_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(package_root) = cache.get(&cache_key) {
            return Some(package_root.clone());
        }
    }

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
    let packages = metadata.get("packages")?.as_array()?;
    let manifest = packages
        .iter()
        .find(|package| package.get("name").and_then(|name| name.as_str()) == Some(package_name))?
        .get("manifest_path")?
        .as_str()?;

    let package_root = PathBuf::from(manifest).parent().map(Path::to_path_buf)?;
    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key, package_root.clone());
    }

    Some(package_root)
}

fn cargo_package_ax_root(package_root: &Path, namespace: &str) -> Option<PathBuf> {
    let source = fs::read_to_string(package_root.join("Axonyx.package.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    let metadata_namespace = value
        .get("package")
        .and_then(|package| package.get("namespace"))
        .and_then(|namespace| namespace.as_str())?;

    if metadata_namespace != namespace {
        return None;
    }

    let ax_root = value
        .get("exports")
        .and_then(|exports| exports.get("ax_root"))
        .and_then(|ax_root| ax_root.as_str())?;

    Some(package_root.join(ax_root))
}

fn default_package_ax_root(package_root: &Path) -> PathBuf {
    let ax_root = package_root.join("src").join("ax");
    if ax_root.exists() {
        return ax_root;
    }

    package_root.join("src")
}

fn resolve_axonyx_ui_workspace_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@axonyx/ui/")?;
    let mut fallback = None;

    for base in axonyx_ui_workspace_import_bases(root) {
        let path = join_import_relative(base, relative);
        fallback.get_or_insert_with(|| path.clone());

        if path.exists() {
            return Some(path);
        }
    }

    fallback
}

fn resolve_axonyx_ui_app_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@axonyx/ui/")?;

    axonyx_ui_app_import_bases(root)
        .into_iter()
        .map(|base| join_import_relative(base, relative))
        .find(|path| path.exists())
}

fn join_import_relative(mut base: PathBuf, relative: &str) -> PathBuf {
    for segment in relative.split('/') {
        if !segment.is_empty() {
            base.push(segment);
        }
    }

    if base.extension().is_none() {
        base.set_extension("ax");
    }

    base
}

fn resolve_config_path(root: &Path, target: &str) -> Option<PathBuf> {
    if target.starts_with("@/") {
        return resolve_app_import_path(root, target);
    }

    if target.starts_with("@axonyx/ui/") {
        return resolve_axonyx_ui_app_import_path(root, target)
            .or_else(|| resolve_cargo_package_import_path(root, target))
            .or_else(|| resolve_axonyx_ui_workspace_import_path(root, target));
    }

    let path = PathBuf::from(target);
    let mut path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };

    if path.extension().is_none() {
        path.set_extension("ax");
    }

    Some(path)
}

fn resolve_config_base_path(root: &Path, target: &str) -> Option<PathBuf> {
    let path = PathBuf::from(target);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn axonyx_config_string(root: &Path, table: &str, key: &str) -> Option<String> {
    axonyx_config_table(root, table)?
        .get(key)?
        .as_str()
        .map(ToOwned::to_owned)
}

fn axonyx_config_bool(root: &Path, table: &str, key: &str) -> Option<bool> {
    match axonyx_config_value(root, table, key)? {
        toml::Value::Boolean(value) => Some(value),
        toml::Value::String(value) => parse_boolish_strict(&value),
        _ => None,
    }
}

fn axonyx_config_value(root: &Path, table: &str, key: &str) -> Option<toml::Value> {
    axonyx_config_table(root, table)?.get(key).cloned()
}

fn axonyx_config_table(root: &Path, table: &str) -> Option<toml::map::Map<String, toml::Value>> {
    let source = fs::read_to_string(root.join("Axonyx.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    value.get(table)?.as_table().cloned()
}

fn inject_dev_client(html: &str, request_path: &str) -> String {
    let route_literal = format!("{request_path:?}");
    let script = format!(
        r#"<script>
(() => {{
  const route = {route_literal};
  let version = null;
  const poll = async () => {{
    try {{
      const response = await fetch(`/__axonyx/version?path=${{encodeURIComponent(route)}}`, {{ cache: "no-store" }});
      if (!response.ok) return;
      const next = (await response.text()).trim();
      if (version === null) {{
        version = next;
        return;
      }}
      if (next && next !== version) {{
        window.location.reload();
      }}
    }} catch (_error) {{
      // Dev-only polling can stay quiet while the server restarts.
    }}
  }};
  setInterval(poll, 900);
}})();
</script>"#
    );

    if let Some(index) = html.rfind("</body>") {
        let mut with_client = String::with_capacity(html.len() + script.len());
        with_client.push_str(&html[..index]);
        with_client.push_str(&script);
        with_client.push_str(&html[index..]);
        with_client
    } else {
        let mut with_client = html.to_string();
        with_client.push_str(&script);
        with_client
    }
}

fn write_ax_response(
    stream: &mut TcpStream,
    response: &AxHttpResponse,
    suppress_body: bool,
) -> Result<()> {
    let header = if suppress_body {
        render_response_header_with_body_policy(response, true)
    } else {
        render_response_header(response)
    };
    stream
        .write_all(header.as_bytes())
        .context("failed to write response headers")?;

    if suppress_body {
        stream.flush().context("failed to flush response")?;
        return Ok(());
    }

    if response.body.is_streaming() {
        write_chunked_body(stream, response)?;
    } else {
        for chunk in response.body.chunks_iter() {
            stream
                .write_all(chunk)
                .context("failed to write response body")?;
        }
    }
    stream.flush().context("failed to flush response")?;
    Ok(())
}

#[cfg(test)]
async fn write_ax_response_async(
    stream: &mut tokio::net::TcpStream,
    response: &AxHttpResponse,
    suppress_body: bool,
) -> Result<()> {
    let header = if suppress_body {
        render_response_header_with_body_policy(response, true)
    } else {
        render_response_header(response)
    };

    stream
        .write_all(header.as_bytes())
        .await
        .context("failed to write async response headers")?;

    if suppress_body {
        stream
            .flush()
            .await
            .context("failed to flush async response")?;
        return Ok(());
    }

    if response.body.is_streaming() {
        write_chunked_body_async(stream, response).await?;
    } else {
        for chunk in response.body.chunks_iter() {
            stream
                .write_all(chunk)
                .await
                .context("failed to write async response body")?;
        }
    }
    stream
        .flush()
        .await
        .context("failed to flush async response")?;
    Ok(())
}

fn render_response_header(response: &AxHttpResponse) -> String {
    render_response_header_with_body_policy(response, false)
}

fn render_response_header_with_body_policy(
    response: &AxHttpResponse,
    suppress_body: bool,
) -> String {
    let status = response.status_line();
    let mut header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nConnection: close\r\n",
        response.content_type
    );
    if response.body.is_streaming() && !suppress_body {
        header.push_str("Transfer-Encoding: chunked\r\n");
    } else {
        header.push_str(&format!("Content-Length: {}\r\n", response.body_len()));
    }
    if response.header_value("Cache-Control").is_none() {
        header.push_str("Cache-Control: no-store\r\n");
    }
    for (name, value) in &response.headers {
        header.push_str(name);
        header.push_str(": ");
        header.push_str(value);
        header.push_str("\r\n");
    }
    for cookie in &response.set_cookies {
        header.push_str("Set-Cookie: ");
        header.push_str(cookie);
        header.push_str("\r\n");
    }
    header.push_str("\r\n");
    header
}

fn write_chunked_body(stream: &mut TcpStream, response: &AxHttpResponse) -> Result<()> {
    for chunk in response.body.chunks_iter() {
        write!(stream, "{:X}\r\n", chunk.len()).context("failed to write chunk header")?;
        stream
            .write_all(chunk)
            .context("failed to write response chunk")?;
        stream
            .write_all(b"\r\n")
            .context("failed to finish response chunk")?;
    }
    stream
        .write_all(b"0\r\n\r\n")
        .context("failed to finish chunked response")?;
    Ok(())
}

#[cfg(test)]
async fn write_chunked_body_async(
    stream: &mut tokio::net::TcpStream,
    response: &AxHttpResponse,
) -> Result<()> {
    for chunk in response.body.chunks_iter() {
        stream
            .write_all(format!("{:X}\r\n", chunk.len()).as_bytes())
            .await
            .context("failed to write async chunk header")?;
        stream
            .write_all(chunk)
            .await
            .context("failed to write async response chunk")?;
        stream
            .write_all(b"\r\n")
            .await
            .context("failed to finish async response chunk")?;
    }
    stream
        .write_all(b"0\r\n\r\n")
        .await
        .context("failed to finish async chunked response")?;
    Ok(())
}

fn extract_version_path(target: &str) -> Option<String> {
    query_param_value(target, "path").map(url_decode)
}

fn extract_action_query_param(target: &str, needle: &str) -> Option<String> {
    query_param_value(target, needle).map(url_decode)
}

fn query_param_value<'a>(target: &'a str, needle: &str) -> Option<&'a str> {
    let query = target.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == needle {
            return Some(value);
        }
    }
    None
}

fn parse_boolish(value: &str) -> bool {
    parse_boolish_strict(value).unwrap_or(false)
}

fn parse_boolish_strict(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn normalize_request_path(request_path: &str) -> Result<String> {
    let raw_path = request_path.split(['?', '#']).next().unwrap_or("/").trim();
    let raw_path = if raw_path.is_empty() { "/" } else { raw_path };
    let mut segments = Vec::new();

    for segment in raw_path.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\\') {
            bail!("invalid route path '{request_path}'");
        }
        segments.push(segment.to_string());
    }

    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

fn path_segments(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                    out.push(decoded as char);
                    index += 3;
                } else {
                    out.push('%');
                    index += 1;
                }
            }
            byte => {
                out.push(byte as char);
                index += 1;
            }
        }
    }

    out
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn looks_like_asset_request(request_path: &str) -> bool {
    request_path
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .is_some_and(|segment| segment.contains('.'))
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
        TEST_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test env lock should not be poisoned")
    }

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!(
            "axonyx-cargo-test-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("temp dir should create");
        path
    }

    fn test_dev_state(root: &Path) -> DevServerState {
        DevServerState {
            root: root.to_path_buf(),
            preview_store: Mutex::new(AxPreviewStore::default()),
            runtime_config: AxServerRuntimeConfig::default(),
        }
    }

    fn write_test_axonyx_ui_package(root: &Path, card_title: &str, css: &str) {
        fs::create_dir_all(root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::create_dir_all(root.join("src/css")).expect("ui css dir should exist");
        fs::create_dir_all(root.join("src/js")).expect("ui js dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
css_root = "src/css"
css_entry = "src/css/index.css"
js_root = "src/js"
js_entry = "src/js/index.js"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            root.join("src/foundry/SectionCard.ax"),
            format!(
                r#"
page SectionCard
<Card title="{card_title}" />
"#
            ),
        )
        .expect("ui component should write");
        fs::write(root.join("src/css/index.css"), css).expect("ui css should write");
        fs::write(
            root.join("src/js/index.js"),
            "window.__axonyxUiRuntime = true;",
        )
        .expect("ui js should write");
    }

    #[test]
    fn resolve_route_collects_nested_layouts() {
        let root = make_temp_dir("route");
        fs::create_dir_all(root.join("app/docs")).expect("app/docs should exist");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n  Slot\n")
            .expect("root layout should write");
        fs::write(root.join("app/docs/layout.ax"), "page DocsLayout\n  Slot\n")
            .expect("nested layout should write");
        fs::write(
            root.join("app/docs/loader.ax"),
            "loader DocsList\n  data docs = Db.Stream(\"docs\")\n  return docs\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/docs/actions.ax"),
            "action CreateDoc\n  input:\n    title: string\n\n  return ok\n",
        )
        .expect("actions should write");
        fs::write(
            root.join("app/docs/page.ax"),
            "page DocsHome\n  Copy -> \"Docs\"\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/docs")
            .expect("route resolution should work")
            .expect("route should exist");

        assert_eq!(route.request_path, "/docs");
        assert_eq!(route.request_target, "/docs");
        assert_eq!(route.layout_paths.len(), 2);
        assert!(route
            .loader_path
            .as_ref()
            .is_some_and(|path| path.ends_with(Path::new("app").join("docs").join("loader.ax"))));
        assert!(route
            .actions_path
            .as_ref()
            .is_some_and(|path| path.ends_with(Path::new("app").join("docs").join("actions.ax"))));
        assert!(route
            .page_path
            .ends_with(Path::new("app").join("docs").join("page.ax")));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn resolve_route_supports_dynamic_app_segments() {
        let root = make_temp_dir("dynamic-route");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n  Copy -> params.slug\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/posts/hello-axonyx")
            .expect("route resolution should work")
            .expect("route should exist");

        assert_eq!(route.request_path, "/posts/hello-axonyx");
        assert_eq!(
            route.params.get("slug").map(String::as_str),
            Some("hello-axonyx")
        );
        assert!(route.page_path.ends_with(
            Path::new("app")
                .join("posts")
                .join("[slug]")
                .join("page.ax")
        ));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn resolve_route_prefers_static_app_segment_over_dynamic_one() {
        let root = make_temp_dir("dynamic-static-route");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::create_dir_all(root.join("app/posts/featured")).expect("static app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page DynamicPost\n  Copy -> params.slug\n",
        )
        .expect("dynamic page should write");
        fs::write(
            root.join("app/posts/featured/page.ax"),
            "page FeaturedPost\n  Copy -> \"Featured\"\n",
        )
        .expect("static page should write");

        let route = resolve_route(&root, "/posts/featured")
            .expect("route resolution should work")
            .expect("route should exist");

        assert!(route.page_path.ends_with(
            Path::new("app")
                .join("posts")
                .join("featured")
                .join("page.ax")
        ));
        assert!(route.params.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn collects_app_route_manifest_for_cli_output() {
        let root = make_temp_dir("route-manifest");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n<Slot />\n")
            .expect("root layout should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("home page should write");
        fs::write(
            root.join("app/posts/[slug]/layout.ax"),
            "page PostLayout\n<Slot />\n",
        )
        .expect("post layout should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n<Copy>{params.slug}</Copy>\n",
        )
        .expect("post page should write");
        fs::write(
            root.join("app/posts/[slug]/loader.ax"),
            "loader Post\n  return ok\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/posts/[slug]/actions.ax"),
            "action Save\n  return ok\n",
        )
        .expect("actions should write");

        let routes = collect_app_route_manifest(&root).expect("manifest should collect");

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].kind, "page");
        assert_eq!(routes[0].route, "/");
        assert_eq!(routes[0].method, None);
        assert_eq!(routes[0].file, "app/page.ax");
        assert_eq!(routes[0].layouts, vec!["app/layout.ax"]);

        assert_eq!(routes[1].kind, "page");
        assert_eq!(routes[1].route, "/posts/:slug");
        assert_eq!(routes[1].params, vec!["slug"]);
        assert_eq!(routes[1].layouts.len(), 2);
        assert_eq!(
            routes[1].loader.as_deref(),
            Some("app/posts/[slug]/loader.ax")
        );
        assert_eq!(
            routes[1].actions.as_deref(),
            Some("app/posts/[slug]/actions.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn route_manifest_includes_backend_api_routes() {
        let root = make_temp_dir("backend-route-manifest");
        fs::create_dir_all(root.join("routes/api")).expect("api routes dir should exist");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts"
  before Security.headers
  after Cache.noStore
  return ok

route POST "/api/posts/:slug"
  input:
    title: string
    featured?: bool = false

  return ok
"#,
        )
        .expect("route source should write");

        let routes = collect_app_route_manifest(&root).expect("manifest should collect");

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].kind, "api");
        assert_eq!(routes[0].method.as_deref(), Some("GET"));
        assert_eq!(routes[0].route, "/api/posts");
        assert_eq!(routes[0].file, "routes/api/posts.ax");
        assert_eq!(
            routes[0].hooks,
            vec![
                RouteHookReport {
                    phase: "before",
                    value: "Security.headers".to_string(),
                },
                RouteHookReport {
                    phase: "after",
                    value: "Cache.noStore".to_string(),
                },
            ]
        );

        assert_eq!(routes[1].kind, "api");
        assert_eq!(routes[1].method.as_deref(), Some("POST"));
        assert_eq!(routes[1].route, "/api/posts/:slug");
        assert_eq!(routes[1].params, vec!["slug"]);
        assert_eq!(
            routes[1].inputs,
            vec![
                ActionInputReport {
                    name: "title".to_string(),
                    ty: "string".to_string(),
                    optional: false,
                    default: None,
                },
                ActionInputReport {
                    name: "featured".to_string(),
                    ty: "bool".to_string(),
                    optional: true,
                    default: Some("false".to_string()),
                },
            ]
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn api_report_collects_typed_route_contracts() {
        let root = make_temp_dir("api-report-contracts");
        fs::create_dir_all(root.join("routes/api")).expect("api routes dir should exist");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts" -> Post[]
  return ok

route POST "/api/posts" -> Post
  input:
    title: string
    featured?: bool = false

  return json(input.title)
"#,
        )
        .expect("route source should write");

        let report = collect_api_report(&root).expect("api report should collect");

        assert_eq!(report.routes.len(), 2);
        assert!(report.schemas.is_empty());
        assert_eq!(report.routes[0].method, "GET");
        assert_eq!(report.routes[0].route, "/api/posts");
        assert_eq!(report.routes[0].returns.as_deref(), Some("Post[]"));
        assert!(report.routes[0].inputs.is_empty());
        assert_eq!(report.routes[1].method, "POST");
        assert_eq!(report.routes[1].returns.as_deref(), Some("Post"));
        assert_eq!(
            report.routes[1].inputs,
            vec![
                ActionInputReport {
                    name: "title".to_string(),
                    ty: "string".to_string(),
                    optional: false,
                    default: None,
                },
                ActionInputReport {
                    name: "featured".to_string(),
                    ty: "bool".to_string(),
                    optional: true,
                    default: Some("false".to_string()),
                },
            ]
        );
        assert_eq!(api_route_type_name(&report.routes[1]), "PostApiPosts");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn api_report_can_render_openapi_document() {
        let report = ApiReport {
            routes: vec![ApiRouteReport {
                method: "POST".to_string(),
                route: "/api/posts/:slug".to_string(),
                returns: Some("Post[]".to_string()),
                file: "routes/api/posts.ax".to_string(),
                params: vec!["slug".to_string()],
                inputs: vec![
                    ActionInputReport {
                        name: "title".to_string(),
                        ty: "string".to_string(),
                        optional: false,
                        default: None,
                    },
                    ActionInputReport {
                        name: "featured".to_string(),
                        ty: "bool".to_string(),
                        optional: true,
                        default: Some("false".to_string()),
                    },
                ],
                hooks: Vec::new(),
            }],
            schemas: vec![ApiSchemaReport {
                name: "Post".to_string(),
                fields: vec![
                    ApiSchemaFieldReport {
                        name: "title".to_string(),
                        ty: "String".to_string(),
                        optional: false,
                    },
                    ApiSchemaFieldReport {
                        name: "summary".to_string(),
                        ty: "String".to_string(),
                        optional: true,
                    },
                ],
            }],
        };

        let value = api_report_openapi_value(&report);

        assert_eq!(value["openapi"], "3.1.0");
        let operation = &value["paths"]["/api/posts/{slug}"]["post"];
        assert_eq!(operation["operationId"], "PostApiPostsSlug");
        assert_eq!(operation["parameters"][0]["name"], "slug");
        assert_eq!(
            operation["requestBody"]["content"]["application/json"]["schema"]["required"][0],
            "title"
        );
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["type"],
            "array"
        );
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["items"]["$ref"],
            "#/components/schemas/Post"
        );
        assert_eq!(
            value["components"]["schemas"]["Post"]["properties"]["title"]["type"],
            "string"
        );
        assert_eq!(
            value["components"]["schemas"]["Post"]["required"][0],
            "title"
        );
        assert!(value["components"]["schemas"]["Post"]["required"]
            .as_array()
            .expect("required should be array")
            .iter()
            .all(|field| field != "summary"));
    }

    #[test]
    fn api_report_collects_type_schemas_for_openapi_components() {
        let root = make_temp_dir("api-report-type-schemas");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("routes/api")).expect("api routes dir should exist");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

type Post {
  title: String
  summary: Optional<String>
}

<Copy>Home</Copy>
"#,
        )
        .expect("page should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts" -> Post[]
  return json(posts)
"#,
        )
        .expect("route source should write");

        let report = collect_api_report(&root).expect("api report should collect");
        let value = api_report_openapi_value(&report);

        assert_eq!(report.schemas.len(), 1);
        assert_eq!(report.schemas[0].name, "Post");
        assert_eq!(
            value["components"]["schemas"]["Post"]["properties"]["summary"]["type"],
            "string"
        );
        assert!(value["components"]["schemas"]["Post"]["required"]
            .as_array()
            .expect("required should be array")
            .iter()
            .all(|field| field != "summary"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn api_output_writer_creates_parent_directories() {
        let root = make_temp_dir("api-output-writer");
        let out = root.join("public/contracts/openapi.json");

        write_or_print_api_output(Some(&out), r#"{"openapi":"3.1.0"}"#)
            .expect("api output should write");

        let written = fs::read_to_string(&out).expect("openapi output should exist");
        assert_eq!(written, "{\"openapi\":\"3.1.0\"}\n");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn routes_report_includes_server_streaming_mode() {
        let root = make_temp_dir("route-report-stream-mode");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = true\n",
        )
        .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("home page should write");

        let report = routes_report(&root).expect("routes report should collect");

        assert!(report.stream_pages);
        assert_eq!(report.routes.len(), 1);
        assert_eq!(report.routes[0].route, "/");

        let json = serde_json::to_string(&report).expect("routes report should serialize");
        assert!(json.contains("\"stream_pages\":true"));
        assert!(json.contains("\"routes\""));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn melt_report_collects_framework_layer_graph() {
        let root = make_temp_dir("melt-report");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = true\n",
        )
        .expect("config should write");
        fs::create_dir_all(root.join("app/settings")).expect("settings dir should exist");
        fs::create_dir_all(root.join("routes/api")).expect("api dir should exist");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

page state theme: String = "silver"

<Copy>Home</Copy>
"#,
        )
        .expect("page should write");
        fs::write(
            root.join("app/settings/page.ax"),
            "page Settings\n<Copy>Settings</Copy>\n",
        )
        .expect("settings page should write");
        fs::write(
            root.join("app/settings/actions.ax"),
            "action Save\n  return ok\n",
        )
        .expect("actions should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            "route GET \"/api/posts\"\n  return json(\"ok\")\n",
        )
        .expect("api route should write");

        let report = collect_melt_report(&root).expect("melt report should collect");

        assert_eq!(report.app.name, "demo");
        assert_eq!(report.summary.page_routes, 2);
        assert_eq!(report.summary.api_routes, 1);
        assert_eq!(report.summary.action_routes, 1);
        assert_eq!(report.summary.actions, 1);
        assert_eq!(report.summary.state_signals, 1);
        assert_eq!(report.summary.diagnostics, 0);
        assert!(report
            .layers
            .iter()
            .any(|layer| layer.name == "Axonyx Pages" && layer.status == "ready"));
        assert!(report
            .layers
            .iter()
            .any(|layer| layer.name == "Axonyx Melt" && layer.status == "ready"));

        let json = serde_json::to_string(&report).expect("melt report should serialize");
        assert!(json.contains("\"Axonyx Pages\""));
        assert!(json.contains("\"summary\""));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn state_graph_maps_app_layout_and_page_signals_to_routes() {
        let report = StateReport {
            files: vec![StateReportFile {
                file: "app/page.ax".to_string(),
                signals: vec![
                    StateReportSignal {
                        name: "language".to_string(),
                        key: "app:language:1".to_string(),
                        scope: "app".to_string(),
                        owner: "app".to_string(),
                        ty: "String".to_string(),
                        initial: AxStateValue::String("sr".to_string()),
                    },
                    StateReportSignal {
                        name: "sidebarOpen".to_string(),
                        key: "layout:docs:sidebarOpen:1".to_string(),
                        scope: "layout:docs".to_string(),
                        owner: "layout:/docs".to_string(),
                        ty: "Bool".to_string(),
                        initial: AxStateValue::Bool(false),
                    },
                    StateReportSignal {
                        name: "filter".to_string(),
                        key: "page:docs.getting-started:filter:1".to_string(),
                        scope: "page:docs.getting-started".to_string(),
                        owner: "page:/docs/getting-started".to_string(),
                        ty: "String".to_string(),
                        initial: AxStateValue::String(String::new()),
                    },
                ],
            }],
        };

        let labels = state_signal_labels_for_route(&report, "/docs/getting-started");

        assert!(labels.contains(&"app:language".to_string()));
        assert!(labels.contains(&"layout:/docs:sidebarOpen".to_string()));
        assert!(labels.contains(&"page:/docs/getting-started:filter".to_string()));
    }

    #[test]
    fn action_report_collects_route_local_inputs() {
        let root = make_temp_dir("action-report");
        fs::create_dir_all(root.join("app/settings")).expect("settings dir should exist");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("home page should write");
        fs::write(
            root.join("app/settings/page.ax"),
            "page Settings\n<Copy>Settings</Copy>\n",
        )
        .expect("settings page should write");
        fs::write(
            root.join("app/settings/actions.ax"),
            r#"
action SetTheme -> ThemePatch
  input:
    theme: string = "silver"
    newsletter?: bool = false
    count: i64 = 0

  patch theme = input.theme

action ClearTheme
  return ok
"#,
        )
        .expect("actions should write");

        let report = collect_action_report(&root).expect("action report should collect");

        assert_eq!(report.routes.len(), 1);
        assert_eq!(report.routes[0].route, "/settings");
        assert_eq!(report.routes[0].file, "app/settings/actions.ax");
        assert_eq!(report.routes[0].actions.len(), 2);
        assert_eq!(report.routes[0].actions[0].name, "SetTheme");
        assert_eq!(
            report.routes[0].actions[0].returns.as_deref(),
            Some("ThemePatch")
        );
        assert_eq!(
            report.routes[0].actions[0].inputs,
            vec![
                ActionInputReport {
                    name: "theme".to_string(),
                    ty: "string".to_string(),
                    optional: false,
                    default: Some("\"silver\"".to_string()),
                },
                ActionInputReport {
                    name: "newsletter".to_string(),
                    ty: "bool".to_string(),
                    optional: true,
                    default: Some("false".to_string()),
                },
                ActionInputReport {
                    name: "count".to_string(),
                    ty: "i64".to_string(),
                    optional: false,
                    default: Some("0".to_string()),
                },
            ]
        );
        assert!(report.routes[0].actions[1].inputs.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn action_report_filters_by_route_and_name() {
        let report = ActionReport {
            routes: vec![
                ActionRouteReport {
                    route: "/settings".to_string(),
                    file: "app/settings/actions.ax".to_string(),
                    actions: vec![
                        ActionItemReport {
                            name: "SetTheme".to_string(),
                            returns: None,
                            inputs: Vec::new(),
                        },
                        ActionItemReport {
                            name: "ClearTheme".to_string(),
                            returns: None,
                            inputs: Vec::new(),
                        },
                    ],
                },
                ActionRouteReport {
                    route: "/feedback".to_string(),
                    file: "app/feedback/actions.ax".to_string(),
                    actions: vec![ActionItemReport {
                        name: "SendFeedback".to_string(),
                        returns: None,
                        inputs: Vec::new(),
                    }],
                },
            ],
        };
        let args = ActionsArgs {
            format: CheckFormat::Text,
            route: Some("/settings".to_string()),
            name: Some("ClearTheme".to_string()),
            schema: false,
        };

        let filtered = filter_action_report(report, &args);

        assert_eq!(filtered.routes.len(), 1);
        assert_eq!(filtered.routes[0].route, "/settings");
        assert_eq!(filtered.routes[0].actions.len(), 1);
        assert_eq!(filtered.routes[0].actions[0].name, "ClearTheme");
    }

    #[test]
    fn action_schema_maps_inputs_to_ax_types() {
        assert_eq!(ax_schema_type("string"), "String");
        assert_eq!(ax_schema_type("bool"), "Bool");
        assert_eq!(ax_schema_type("i64"), "Number");
        assert_eq!(ax_schema_type("unknown"), "String");
    }

    #[test]
    fn response_schema_normalizes_return_contract_types() {
        assert_eq!(ax_return_schema_type("Post"), "Post");
        assert_eq!(ax_return_schema_type("Post[]"), "List<Post>");
        assert_eq!(ax_return_schema_type("string"), "String");
        assert_eq!(ax_return_schema_type("f64"), "Number");
        assert_eq!(ax_return_schema_type("Optional<Post>"), "Optional<Post>");
    }

    #[test]
    fn state_report_collects_app_state_declarations() {
        let root = make_temp_dir("state-report");
        fs::create_dir_all(root.join("app/settings")).expect("settings dir should exist");
        fs::write(
            root.join("app/layout.ax"),
            r#"
page RootLayout

app state language: String = "sr"

<Slot />
"#,
        )
        .expect("root layout should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

page state theme: String = "silver"
page state count: Number = 0

<input bind:value={theme} />
"#,
        )
        .expect("home page should write");
        fs::write(
            root.join("app/settings/layout.ax"),
            r#"
page SettingsLayout

layout state sidebarOpen: Bool = false

<Slot />
"#,
        )
        .expect("settings layout should write");
        fs::write(
            root.join("app/settings/page.ax"),
            r#"
page Settings

page state enabled = signal(true)

<input bind:checked={enabled} />
"#,
        )
        .expect("settings page should write");
        fs::write(
            root.join("app/settings/actions.ax"),
            "action Save\n  return ok\n",
        )
        .expect("actions should write");

        let report = collect_state_report(&root).expect("state report should collect");

        assert_eq!(report.files.len(), 4);
        assert_eq!(report.files[0].file, "app/layout.ax");
        assert_eq!(report.files[0].signals[0].name, "language");
        assert_eq!(report.files[0].signals[0].key, "app:language:1");
        assert_eq!(report.files[0].signals[0].scope, "app");
        assert_eq!(report.files[0].signals[0].owner, "app");

        assert_eq!(report.files[1].file, "app/page.ax");
        assert_eq!(report.files[1].signals.len(), 2);
        assert_eq!(report.files[1].signals[0].name, "theme");
        assert_eq!(report.files[1].signals[0].key, "page:root:theme:1");
        assert_eq!(report.files[1].signals[0].scope, "page:root");
        assert_eq!(report.files[1].signals[0].owner, "page:/");
        assert_eq!(report.files[1].signals[0].ty, "String");
        assert_eq!(
            report.files[1].signals[0].initial,
            AxStateValue::String("silver".to_string())
        );

        assert_eq!(report.files[2].file, "app/settings/layout.ax");
        assert_eq!(report.files[2].signals[0].name, "sidebarOpen");
        assert_eq!(
            report.files[2].signals[0].key,
            "layout:settings:sidebarOpen:1"
        );
        assert_eq!(report.files[2].signals[0].scope, "layout:settings");
        assert_eq!(report.files[2].signals[0].owner, "layout:/settings");

        assert_eq!(report.files[3].file, "app/settings/page.ax");
        assert_eq!(report.files[3].signals[0].name, "enabled");
        assert_eq!(report.files[3].signals[0].key, "page:settings:enabled:1");
        assert_eq!(report.files[3].signals[0].scope, "page:settings");
        assert_eq!(report.files[3].signals[0].owner, "page:/settings");
        assert_eq!(report.files[3].signals[0].initial, AxStateValue::Bool(true));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_duplicate_page_route_patterns() {
        let root = make_temp_dir("duplicate-page-routes");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("slug route should exist");
        fs::create_dir_all(root.join("app/posts/[id]")).expect("id route should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page PostBySlug\n<Copy>{params.slug}</Copy>\n",
        )
        .expect("slug page should write");
        fs::write(
            root.join("app/posts/[id]/page.ax"),
            "page PostById\n<Copy>{params.id}</Copy>\n",
        )
        .expect("id page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-route-duplicate");
        assert!(diagnostics[0].message.contains("/posts/:"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_stream_pages_config() {
        let root = make_temp_dir("invalid-stream-pages-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = \"maybe\"\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "axonyx-config-stream-pages"
                && diagnostic.file.ends_with("Axonyx.toml")
                && diagnostic.line == 5
                && diagnostic.message.contains("stream_pages")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_max_body_bytes_config() {
        let root = make_temp_dir("invalid-max-body-bytes-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nmax_body_bytes = \"huge\"\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "axonyx-config-max-body-bytes"
                && diagnostic.file.ends_with("Axonyx.toml")
                && diagnostic.line == 5
                && diagnostic.message.contains("max_body_bytes")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_request_timeout_config() {
        let root = make_temp_dir("invalid-request-timeout-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nrequest_timeout_seconds = 0\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "axonyx-config-request-timeout"
                && diagnostic.file.ends_with("Axonyx.toml")
                && diagnostic.line == 5
                && diagnostic.message.contains("request_timeout_seconds")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_shutdown_grace_config() {
        let root = make_temp_dir("invalid-shutdown-grace-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nshutdown_grace_seconds = -1\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "axonyx-config-shutdown-grace"
                && diagnostic.file.ends_with("Axonyx.toml")
                && diagnostic.line == 5
                && diagnostic.message.contains("shutdown_grace_seconds")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_max_connections_config() {
        let root = make_temp_dir("invalid-max-connections-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nmax_connections = 0\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "axonyx-config-max-connections"
                && diagnostic.file.ends_with("Axonyx.toml")
                && diagnostic.line == 5
                && diagnostic.message.contains("max_connections")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_invalid_server_hardening_config() {
        let root = make_temp_dir("invalid-server-hardening-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\ncompression = 12\nsecurity_headers = \"sometimes\"\nrequest_logging = []\nlog_format = \"xml\"\n",
        )
        .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "axonyx-config-compression"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "axonyx-config-security-headers"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "axonyx-config-request-logging"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "axonyx-config-log-format"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_duplicate_backend_api_routes() {
        let root = make_temp_dir("duplicate-api-routes");
        fs::create_dir_all(root.join("routes/api")).expect("api dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts"
  return ok
"#,
        )
        .expect("first route should write");
        fs::write(
            root.join("routes/api/posts-copy.ax"),
            r#"
route GET "/api/posts"
  return ok
"#,
        )
        .expect("second route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-route-duplicate");
        assert!(diagnostics[0].message.contains("GET /api/posts"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn normalize_request_path_rejects_parent_segments() {
        assert!(normalize_request_path("/../secret").is_err());
    }

    #[test]
    fn inject_dev_client_adds_reload_script() {
        let html = inject_dev_client("<html><body><main>Hello</main></body></html>", "/docs");

        assert!(html.contains("/__axonyx/version"));
        assert!(html.contains("window.location.reload"));
    }

    #[test]
    fn loads_public_asset_from_public_directory() {
        let root = make_temp_dir("public");
        fs::create_dir_all(root.join("public")).expect("public dir should exist");
        fs::write(root.join("public/logo.svg"), "<svg></svg>").expect("asset should write");

        let asset = load_public_asset(&root, "/logo.svg")
            .expect("asset lookup should work")
            .expect("asset should exist");

        assert_eq!(asset.content_type, "image/svg+xml");
        assert_eq!(asset.body, b"<svg></svg>");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn public_asset_response_uses_immutable_cache_header() {
        let root = make_temp_dir("public-cache");
        fs::create_dir_all(root.join("public")).expect("public dir should exist");
        fs::write(root.join("public/logo.svg"), "<svg></svg>").expect("asset should write");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
            runtime_config: AxServerRuntimeConfig::from_root(&root)
                .expect("runtime config should load"),
        };
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/logo.svg".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };

        let response =
            handle_http_request(&state, AxServerMode::Start, request).expect("request should run");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.header_value("Cache-Control"),
            Some(IMMUTABLE_ASSET_CACHE_CONTROL)
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn missing_public_asset_returns_none() {
        let root = make_temp_dir("missing-public");

        let asset = load_public_asset(&root, "/missing.svg").expect("asset lookup should work");

        assert!(asset.is_none());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn loads_package_asset_from_cargo_dependency() {
        let workspace = make_temp_dir("package-asset-cargo");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        write_test_axonyx_ui_package(&ui_root, "Cargo UI", "body { color: silver; }");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");

        let asset = load_package_asset(&root, "/_ax/pkg/axonyx-ui/index.css")
            .expect("package asset lookup should work")
            .expect("package asset should exist");

        assert_eq!(asset.content_type, "text/css; charset=utf-8");
        assert_eq!(asset.body, b"body { color: silver; }");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn loads_hashed_package_asset_from_cargo_dependency() {
        let workspace = make_temp_dir("hashed-package-asset-cargo");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        write_test_axonyx_ui_package(&ui_root, "Cargo UI", "body { color: silver; }");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");
        let file_name = hashed_asset_file_name(&ui_root.join("src/css/index.css"))
            .expect("asset hash should compute")
            .expect("hashed file name should exist");
        let request_path = format!("/_ax/pkg/axonyx-ui/{}", file_name.to_string_lossy());

        let asset = load_package_asset(&root, &request_path)
            .expect("package asset lookup should work")
            .expect("package asset should exist");

        assert_eq!(asset.content_type, "text/css; charset=utf-8");
        assert_eq!(asset.body, b"body { color: silver; }");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn cargo_dependency_asset_wins_over_framework_workspace_vendor() {
        let workspace = make_temp_dir("package-asset-cargo-before-framework-vendor");
        let root = workspace.join("axonyx-site");
        let cargo_ui_root = workspace.join("axonyx-ui");
        let framework_vendor_ui_root = workspace
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui");
        let ui_path = cargo_ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        write_test_axonyx_ui_package(&cargo_ui_root, "Cargo UI", "body { color: cargo-silver; }");
        write_test_axonyx_ui_package(
            &framework_vendor_ui_root,
            "Vendored UI",
            "body { color: stale-vendor; }",
        );
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");

        let asset = load_package_asset(&root, "/_ax/pkg/axonyx-ui/index.css")
            .expect("package asset lookup should work")
            .expect("package asset should exist");

        assert_eq!(asset.body, b"body { color: cargo-silver; }");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn loads_package_js_asset_from_cargo_dependency() {
        let workspace = make_temp_dir("package-js-asset-cargo");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        write_test_axonyx_ui_package(&ui_root, "Cargo UI", "body { color: silver; }");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");

        let asset = load_package_asset(&root, "/_ax/pkg/axonyx-ui/js/index.js")
            .expect("package asset lookup should work")
            .expect("package asset should exist");

        assert_eq!(asset.content_type, "application/javascript; charset=utf-8");
        assert_eq!(asset.body, b"window.__axonyxUiRuntime = true;");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn package_asset_rejects_parent_segments() {
        let root = make_temp_dir("package-asset-parent");

        assert!(load_package_asset(&root, "/_ax/pkg/axonyx-ui/../secret.css").is_err());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_command_generates_backend_module_from_app_sources() {
        let root = make_temp_dir("build");
        fs::create_dir_all(root.join("routes").join("api")).expect("routes dir should exist");
        fs::create_dir_all(root.join("src").join("generated")).expect("generated dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("routes").join("api").join("posts.ax"),
            "route GET \"/api/posts\"\n  data posts = Db.Stream(\"posts\")\n  return posts\n",
        )
        .expect("route should write");

        let status = compile_backend_from_app_root(&root).expect("build should succeed");

        match status {
            BackendBuildStatus::Generated {
                source_count,
                output_path,
            } => {
                assert_eq!(source_count, 1);
                let module =
                    fs::read_to_string(output_path).expect("generated backend should be readable");
                assert!(module.contains("pub fn route_get_api_posts"));
            }
            BackendBuildStatus::NoSources { .. } => panic!("backend sources should be found"),
        }

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_preflight_reports_file_level_diagnostics() {
        let root = make_temp_dir("build-preflight-diagnostics");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { MissingCard } from "@/components/MissingCard.ax"

page Home
<MissingCard />
"#,
        )
        .expect("page should write");

        let error =
            ensure_no_check_diagnostics_for(&root, "build").expect_err("diagnostics should fail");
        let message = error.to_string();

        assert!(message.contains("Axonyx diagnostics failed before build"));
        assert!(message.contains("app/page.ax"));
        assert!(message.contains("axonyx-import"));
        assert!(message.contains("@/components/MissingCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_melt_preflight_reports_file_level_diagnostics() {
        let root = make_temp_dir("build-melt-preflight-diagnostics");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { MissingCard } from "@/components/MissingCard.ax"

page Home
<MissingCard />
"#,
        )
        .expect("page should write");

        let error =
            ensure_no_melt_diagnostics_for(&root, "build").expect_err("diagnostics should fail");
        let message = error.to_string();

        assert!(message.contains("Axonyx diagnostics failed before build"));
        assert!(message.contains("app/page.ax"));
        assert!(message.contains("axonyx-import"));
        assert!(message.contains("@/components/MissingCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn start_preflight_reports_diagnostics_before_production_start() {
        let root = make_temp_dir("start-preflight-diagnostics");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { MissingCard } from "@/components/MissingCard.ax"

page Home
<MissingCard />
"#,
        )
        .expect("page should write");

        let error = ensure_no_check_diagnostics_for(&root, "production start")
            .expect_err("diagnostics should fail");
        let message = error.to_string();

        assert!(message.contains("Axonyx diagnostics failed before production start"));
        assert!(message.contains("app/page.ax"));
        assert!(message.contains("axonyx-import"));
        assert!(message.contains("@/components/MissingCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn cli_error_hint_detects_prerender_config_errors() {
        let error = anyhow::anyhow!("prerender route '/blog/:slug' is missing params");

        assert_eq!(
            hint_for_error(&error),
            Some("Check the [prerender] routes in Axonyx.toml. Dynamic params must be strings.")
        );
    }

    #[test]
    fn build_static_site_generates_html_and_copies_public_assets() {
        let root = make_temp_dir("static-build");
        fs::create_dir_all(root.join("app/docs")).expect("docs dir should exist");
        fs::create_dir_all(root.join("public/css")).expect("public css dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Title>Demo</Title>
</Head>
<Slot />
"#,
        )
        .expect("layout should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Home page</Copy>
"#,
        )
        .expect("home page should write");
        fs::write(
            root.join("app/docs/page.ax"),
            r#"
page Docs
<Copy>Docs page</Copy>
"#,
        )
        .expect("docs page should write");
        fs::write(root.join("public/css/site.css"), "body { color: red; }")
            .expect("asset should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                melt_graph_written,
                output_dir,
            } => {
                assert_eq!(route_count, 2);
                assert_eq!(prerendered_count, 0);
                assert_eq!(skipped_dynamic_count, 0);
                assert_eq!(content_collection_count, 0);
                assert!(melt_graph_written);
                assert_eq!(output_dir, root.join("dist"));
            }
            StaticBuildStatus::NoPages { .. } => panic!("static pages should be found"),
        }

        let home =
            fs::read_to_string(root.join("dist/index.html")).expect("home html should exist");
        let docs =
            fs::read_to_string(root.join("dist/docs/index.html")).expect("docs html should exist");
        assert!(home.contains("Home page"));
        assert!(docs.contains("Docs page"));
        assert!(root.join("dist/css/site.css").exists());
        let melt_graph = fs::read_to_string(root.join("dist/_ax/melt/graph.json"))
            .expect("Melt graph should exist");
        assert!(melt_graph.contains("\"summary\""));
        assert!(melt_graph.contains("\"Axonyx Pages\""));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_copies_package_assets_and_use_injects_them() {
        let root = make_temp_dir("static-build-package-assets");
        let ui_root = root.join("vendor/axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Vendored UI", "body { color: gold; }");
        fs::write(
            root.join("app/page.ax"),
            r#"
use "@axonyx/ui"

page Home
<Container>
  <Copy>Static package CSS</Copy>
</Container>
"#,
        )
        .expect("page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static site should build");

        assert!(matches!(status, StaticBuildStatus::Generated { .. }));
        assert_eq!(
            fs::read_to_string(root.join("dist/_ax/pkg/axonyx-ui/index.css"))
                .expect("package css should copy"),
            "body { color: gold; }"
        );
        assert_eq!(
            fs::read_to_string(root.join("dist/_ax/pkg/axonyx-ui/js/index.js"))
                .expect("package js should copy"),
            "window.__axonyxUiRuntime = true;"
        );
        let css_file_name = hashed_asset_file_name(&ui_root.join("src/css/index.css"))
            .expect("css hash should compute")
            .expect("css hashed file name should exist");
        let js_file_name = hashed_asset_file_name(&ui_root.join("src/js/index.js"))
            .expect("js hash should compute")
            .expect("js hashed file name should exist");
        assert_eq!(
            fs::read_to_string(root.join("dist/_ax/pkg/axonyx-ui").join(&css_file_name))
                .expect("hashed package css should copy"),
            "body { color: gold; }"
        );
        assert_eq!(
            fs::read_to_string(root.join("dist/_ax/pkg/axonyx-ui/js").join(&js_file_name))
                .expect("hashed package js should copy"),
            "window.__axonyxUiRuntime = true;"
        );
        let html = fs::read_to_string(root.join("dist/index.html"))
            .expect("static home page should write");
        assert!(html.contains(&format!(
            r#"<link rel="stylesheet" href="/_ax/pkg/axonyx-ui/{}">"#,
            css_file_name.to_string_lossy()
        )));
        assert!(html.contains(&format!(
            r#"<script src="/_ax/pkg/axonyx-ui/js/{}" defer="true"></script>"#,
            js_file_name.to_string_lossy()
        )));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_skips_dynamic_routes_until_prerender_config_exists() {
        let root = make_temp_dir("static-build-dynamic");
        fs::create_dir_all(root.join("app/blog/[slug]")).expect("blog dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/blog/[slug]/page.ax"),
            r#"
page BlogPost
<Copy>Dynamic post</Copy>
"#,
        )
        .expect("dynamic page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::NoPages {
                skipped_dynamic_count,
                content_collection_count,
                melt_graph_written,
                output_dir,
            } => {
                assert_eq!(output_dir, root.join("dist"));
                assert_eq!(skipped_dynamic_count, 1);
                assert_eq!(content_collection_count, 0);
                assert!(melt_graph_written);
            }
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                melt_graph_written,
                ..
            } => {
                assert_eq!(route_count, 0);
                assert_eq!(prerendered_count, 0);
                assert_eq!(skipped_dynamic_count, 1);
                assert_eq!(content_collection_count, 0);
                assert!(melt_graph_written);
            }
        }
        assert!(!root.join("dist/blog/[slug]/index.html").exists());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_prerenders_dynamic_routes_from_config() {
        let root = make_temp_dir("static-build-prerender");
        fs::create_dir_all(root.join("app/blog/[slug]")).expect("blog dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[prerender]
routes = [
  { route = "/blog/:slug", params = [{ slug = "hello-axonyx" }, { slug = "foundry-ui" }] },
]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/blog/[slug]/page.ax"),
            r#"
page BlogPost
<Copy>Post slug: {params.slug}</Copy>
"#,
        )
        .expect("dynamic page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                melt_graph_written,
                output_dir,
            } => {
                assert_eq!(route_count, 0);
                assert_eq!(prerendered_count, 2);
                assert_eq!(skipped_dynamic_count, 0);
                assert_eq!(content_collection_count, 0);
                assert!(melt_graph_written);
                assert_eq!(output_dir, root.join("dist"));
            }
            StaticBuildStatus::NoPages { .. } => panic!("prerender pages should be generated"),
        }

        let hello = fs::read_to_string(root.join("dist/blog/hello-axonyx/index.html"))
            .expect("hello page should exist");
        let foundry = fs::read_to_string(root.join("dist/blog/foundry-ui/index.html"))
            .expect("foundry page should exist");
        assert!(hello.contains("hello-axonyx"));
        assert!(foundry.contains("foundry-ui"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_prerenders_dynamic_routes_from_content_collection() {
        let root = make_temp_dir("static-build-content-prerender");
        fs::create_dir_all(root.join("app/docs/content/[slug]")).expect("docs dir should exist");
        fs::create_dir_all(root.join("content/docs")).expect("content dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "content/docs"

[prerender.collections.docs]
route = "/docs/content/:slug"
param = "slug"
field = "slug"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/docs/content/[slug]/loader.ax"),
            r#"
loader DocDetail
  data docs = Content.Collection("docs")
    where slug = params.slug
    limit 1
  return docs
"#,
        )
        .expect("loader should write");
        fs::write(
            root.join("app/docs/content/[slug]/page.ax"),
            r#"
page DocDetail
<Each items={load DocDetail} as="doc">
  <h1>{doc.title}</h1>
  <Html content={doc.html} />
</Each>
"#,
        )
        .expect("page should write");
        fs::write(
            root.join("content/docs/hello.md"),
            "---\ntitle: Hello Content\n---\n# Hello Content\n\nRendered from markdown.\n",
        )
        .expect("content should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                melt_graph_written,
                ..
            } => {
                assert_eq!(prerendered_count, 1);
                assert_eq!(skipped_dynamic_count, 0);
                assert_eq!(content_collection_count, 1);
                assert!(melt_graph_written);
            }
            StaticBuildStatus::NoPages { .. } => panic!("content prerender page should build"),
        }

        let html = fs::read_to_string(root.join("dist/docs/content/hello/index.html"))
            .expect("content page should exist");
        assert!(html.contains("Hello Content"));
        assert!(html.contains("<p>Rendered from markdown.</p>"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_clean_removes_previous_output() {
        let root = make_temp_dir("static-build-clean");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("dist/stale")).expect("stale dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(root.join("dist/stale/file.txt"), "old").expect("stale file should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Fresh build</Copy>
"#,
        )
        .expect("page should write");

        build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static build should clean");

        assert!(!root.join("dist/stale/file.txt").exists());
        assert!(root.join("dist/index.html").exists());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_clean_refuses_to_remove_app_root() {
        let root = make_temp_dir("static-build-clean-root");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");

        let error = build_static_site_from_app_root(&root, Path::new("."), true)
            .expect_err("clean should refuse app root");

        assert!(error.to_string().contains("--clean refuses to remove"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn strips_cargo_subcommand_prefix_for_ax() {
        let args = vec![
            OsString::from("cargo-ax.exe"),
            OsString::from("ax"),
            OsString::from("run"),
            OsString::from("dev"),
        ];

        let normalized = {
            let mut args = args;
            if args
                .get(1)
                .and_then(|value| value.to_str())
                .is_some_and(|value| matches!(value, "ax" | "axonyx"))
            {
                args.remove(1);
            }
            args
        };

        let cli = Cli::try_parse_from(normalized).expect("cargo ax args should parse");

        assert!(matches!(cli.command, Commands::Run(_)));
    }

    #[test]
    fn parses_stream_probe_command() {
        let cli =
            Cli::try_parse_from(["cargo-ax", "stream", "--host", "0.0.0.0", "--port", "4100"])
                .expect("stream command should parse");

        let Commands::Stream(args) = cli.command else {
            panic!("expected stream command");
        };
        assert_eq!(args.host, "0.0.0.0");
        assert_eq!(args.port, Some(4100));
        assert_eq!(args.transport, ServerTransport::Tokio);
    }

    #[test]
    fn parses_melt_check_command() {
        let cli = Cli::try_parse_from(["cargo-ax", "melt", "--check"])
            .expect("melt check command should parse");

        let Commands::Melt(args) = cli.command else {
            panic!("expected melt command");
        };
        assert!(args.check);
        assert_eq!(args.format, CheckFormat::Text);
    }

    #[test]
    fn parses_graph_json_command() {
        let cli = Cli::try_parse_from(["cargo-ax", "graph", "--format", "json"])
            .expect("graph command should parse");

        let Commands::Graph(args) = cli.command else {
            panic!("expected graph command");
        };
        assert_eq!(args.format, CheckFormat::Json);
    }

    #[test]
    fn parses_doctor_render_deploy_command() {
        let cli = Cli::try_parse_from(["cargo-ax", "doctor", "--deploy", "render"])
            .expect("doctor render deploy command should parse");

        let Commands::Doctor(args) = cli.command else {
            panic!("expected doctor command");
        };
        assert_eq!(args.deploy, Some(DeployTarget::Render));
    }

    #[test]
    fn parses_api_openapi_out_command() {
        let cli = Cli::try_parse_from([
            "cargo-ax",
            "api",
            "--openapi",
            "--out",
            "public/openapi.json",
        ])
        .expect("api openapi out command should parse");

        let Commands::Api(args) = cli.command else {
            panic!("expected api command");
        };
        assert!(args.openapi);
        assert_eq!(args.out.as_deref(), Some(Path::new("public/openapi.json")));
    }

    #[test]
    fn parses_tokio_transport_for_run_dev() {
        let cli = Cli::try_parse_from([
            "cargo-ax",
            "run",
            "dev",
            "--host",
            "127.0.0.1",
            "--port",
            "4101",
            "--transport",
            "tokio",
        ])
        .expect("tokio transport should parse");

        let Commands::Run(args) = cli.command else {
            panic!("expected run command");
        };
        let RunCommands::Dev(args) = args.command else {
            panic!("expected run dev command");
        };
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, Some(4101));
        assert_eq!(args.transport, ServerTransport::Tokio);
    }

    #[test]
    fn production_server_flag_selects_tokio_transport() {
        let cli = Cli::try_parse_from([
            "cargo-ax",
            "run",
            "start",
            "--host",
            "0.0.0.0",
            "--port",
            "4102",
            "--production-server",
        ])
        .expect("production server flag should parse");

        let Commands::Run(args) = cli.command else {
            panic!("expected run command");
        };
        let RunCommands::Start(args) = args.command else {
            panic!("expected run start command");
        };

        assert!(args.production_server);
        assert_eq!(args.transport, ServerTransport::Tokio);
        assert_eq!(args.effective_transport(), ServerTransport::Tokio);
    }

    #[test]
    fn parses_std_transport_fallback_for_run_dev() {
        let cli = Cli::try_parse_from([
            "cargo-ax",
            "run",
            "dev",
            "--host",
            "127.0.0.1",
            "--port",
            "4103",
            "--transport",
            "std",
        ])
        .expect("std fallback transport should parse");

        let Commands::Run(args) = cli.command else {
            panic!("expected run command");
        };
        let RunCommands::Dev(args) = args.command else {
            panic!("expected run dev command");
        };
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, Some(4103));
        assert_eq!(args.transport, ServerTransport::Std);
        assert_eq!(args.effective_transport(), ServerTransport::Std);
    }

    #[test]
    fn resolves_dev_port_without_env_fallback() {
        assert_eq!(
            resolve_server_port(AxServerMode::Dev, None, Some("4200"))
                .expect("dev port should resolve"),
            3000
        );
        assert_eq!(
            resolve_server_port(AxServerMode::Dev, Some(4100), Some("4200"))
                .expect("cli port should win"),
            4100
        );
    }

    #[test]
    fn resolves_start_port_from_env_when_cli_port_is_missing() {
        assert_eq!(
            resolve_server_port(AxServerMode::Start, None, Some("4300"))
                .expect("start port should resolve from env"),
            4300
        );
        assert_eq!(
            resolve_server_port(AxServerMode::Start, Some(4100), Some("4300"))
                .expect("cli port should win"),
            4100
        );
    }

    #[test]
    fn reports_invalid_start_port_env() {
        let error = resolve_server_port(AxServerMode::Start, None, Some("not-a-port"))
            .expect_err("invalid PORT should fail");

        assert!(error.to_string().contains("invalid PORT"));
    }

    #[test]
    fn parses_reserved_test_command() {
        let cli = Cli::try_parse_from(["cargo-ax", "test"]).expect("test command should parse");

        let Commands::Test(args) = cli.command else {
            panic!("expected test command");
        };
        assert!(args.command.is_none());
        assert_eq!(args.config, PathBuf::from("aegis.toml"));
        assert_eq!(args.format, CheckFormat::Text);
        assert!(args.fail_fast);
    }

    #[test]
    fn parses_aegis_test_options() {
        let cli = Cli::try_parse_from([
            "cargo-ax",
            "test",
            "--config",
            "qa/aegis.toml",
            "--format",
            "json",
            "--fail-fast",
            "false",
        ])
        .expect("test options should parse");

        let Commands::Test(args) = cli.command else {
            panic!("expected test command");
        };
        assert!(args.command.is_none());
        assert_eq!(args.config, PathBuf::from("qa/aegis.toml"));
        assert_eq!(args.format, CheckFormat::Json);
        assert!(!args.fail_fast);
    }

    #[test]
    fn parses_reserved_test_browser_command() {
        let cli = Cli::try_parse_from(["cargo-ax", "test", "browser"])
            .expect("test browser command should parse");

        let Commands::Test(args) = cli.command else {
            panic!("expected test command");
        };
        assert!(matches!(args.command, Some(TestCommands::Browser)));
    }

    #[test]
    fn parses_reserved_cms_add_modules() {
        let cms = Cli::try_parse_from(["cargo-ax", "add", "cms"])
            .expect("cms module command should parse");
        let Commands::Add(args) = cms.command else {
            panic!("expected add command");
        };
        assert_eq!(args.module, ModuleKind::Cms);

        let blockbit = Cli::try_parse_from(["cargo-ax", "add", "blockbit"])
            .expect("blockbit module command should parse");
        let Commands::Add(args) = blockbit.command else {
            panic!("expected add command");
        };
        assert_eq!(args.module, ModuleKind::Blockbit);

        let error = add_reserved_cms_module().expect_err("cms module should not install yet");
        assert!(error.to_string().contains("future Axonyx module"));
        assert!(error.to_string().contains("not part of framework core"));
    }

    #[test]
    fn add_ui_module_adds_registry_dependency_and_updates_layout() {
        let workspace = make_temp_dir("add-ui");
        let app_root = workspace.join("demo-app");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = []\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            "page RootLayout\n  title \"Demo\"\n  Slot\n",
        )
        .expect("layout should write");

        add_ui_module(&app_root, &app_root.join("Axonyx.toml")).expect("ui module should add");

        assert!(!app_root.join("vendor/axonyx-ui").exists());
        assert!(!app_root.join("public/css/axonyx-ui").exists());

        let axonyx_toml =
            fs::read_to_string(app_root.join("Axonyx.toml")).expect("config should read back");
        assert!(axonyx_toml.contains("\"ui\""));
        assert!(!axonyx_toml.contains("[package_overrides]"));

        let layout =
            fs::read_to_string(app_root.join("app/layout.ax")).expect("layout should read back");
        assert!(layout.contains("theme \"silver\""));
        assert!(layout.contains("/_ax/pkg/axonyx-ui/index.css"));
        assert!(layout.contains("/_ax/pkg/axonyx-ui/js/index.js"));

        let cargo_toml =
            fs::read_to_string(app_root.join("Cargo.toml")).expect("cargo manifest should read");
        assert!(cargo_toml.contains("axonyx-ui = \"0.0.40\""));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_healthy_ui_package_setup() {
        let workspace = make_temp_dir("doctor-healthy-ui");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"

[dependencies.axonyx-ui]
path = "vendor/axonyx-ui"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
  <Script src="/_ax/pkg/axonyx-ui/js/index.js" defer="true" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");
        fs::write(
            app_root.join("aegis.toml"),
            "base_url = \"http://127.0.0.1:3000\"\n",
        )
        .expect("aegis config should write");

        let checks = doctor_checks(&app_root, None);

        assert!(checks
            .iter()
            .all(|check| check.severity == DoctorSeverity::Ok));
        assert!(checks.iter().any(|check| check.code == "ui-package-css"));
        assert!(checks.iter().any(|check| check.code == "ui-package-js"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_warns_when_interactive_foundry_runtime_is_missing() {
        let workspace = make_temp_dir("doctor-interactive-ui-runtime");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"

[dependencies.axonyx-ui]
path = "vendor/axonyx-ui"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");
        fs::write(
            app_root.join("app/page.ax"),
            r#"
import { Accordion } from "@axonyx/ui/foundry/Accordion.ax"

page Home

<Accordion single="true" />
"#,
        )
        .expect("page should write");

        let checks = doctor_checks(&app_root, None);
        let ui_script = checks
            .iter()
            .find(|check| check.code == "ui-script")
            .expect("ui script check should exist");

        assert_eq!(ui_script.severity, DoctorSeverity::Warn);
        assert!(ui_script.message.contains("Interactive Foundry components"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_aegis_config_status() {
        let root = make_temp_dir("doctor-aegis-config");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");

        let checks = doctor_checks(&root, None);
        let missing = checks
            .iter()
            .find(|check| check.code == "aegis-config")
            .expect("aegis config check should exist");
        assert_eq!(missing.severity, DoctorSeverity::Warn);
        assert!(missing.message.contains("missing"));

        fs::write(
            root.join("aegis.toml"),
            "base_url = \"http://127.0.0.1:3000\"\n",
        )
        .expect("aegis config should write");

        let checks = doctor_checks(&root, None);
        let configured = checks
            .iter()
            .find(|check| check.code == "aegis-config")
            .expect("aegis config check should exist");
        assert_eq!(configured.severity, DoctorSeverity::Ok);
        assert!(configured.message.contains("cargo ax test"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_page_streaming_config() {
        let root = make_temp_dir("doctor-stream-pages");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = true\n",
        )
        .expect("config should write");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"
"#,
        )
        .expect("cargo manifest should write");

        let checks = doctor_checks(&root, None);
        let streaming = checks
            .iter()
            .find(|check| check.code == "server-stream-pages")
            .expect("server streaming check should exist");

        assert_eq!(streaming.severity, DoctorSeverity::Ok);
        assert!(streaming.message.contains("enabled"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_request_timeout_config() {
        let root = make_temp_dir("doctor-request-timeout");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nrequest_timeout_seconds = 5\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let timeout = checks
            .iter()
            .find(|check| check.code == "server-request-timeout")
            .expect("server timeout check should exist");

        assert_eq!(timeout.severity, DoctorSeverity::Ok);
        assert!(timeout.message.contains("5 second"));

        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nrequest_timeout_seconds = false\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let timeout = checks
            .iter()
            .find(|check| check.code == "server-request-timeout")
            .expect("server timeout check should exist");

        assert_eq!(timeout.severity, DoctorSeverity::Error);
        assert!(timeout.message.contains("request_timeout_seconds"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_shutdown_grace_config() {
        let root = make_temp_dir("doctor-shutdown-grace");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nshutdown_grace_seconds = 9\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let grace = checks
            .iter()
            .find(|check| check.code == "server-shutdown-grace")
            .expect("server shutdown grace check should exist");

        assert_eq!(grace.severity, DoctorSeverity::Ok);
        assert!(grace.message.contains("9 second"));

        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nshutdown_grace_seconds = \"later\"\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let grace = checks
            .iter()
            .find(|check| check.code == "server-shutdown-grace")
            .expect("server shutdown grace check should exist");

        assert_eq!(grace.severity, DoctorSeverity::Error);
        assert!(grace.message.contains("shutdown_grace_seconds"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_max_connections_config() {
        let root = make_temp_dir("doctor-max-connections");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nmax_connections = 7\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let limit = checks
            .iter()
            .find(|check| check.code == "server-max-connections")
            .expect("server max connections check should exist");

        assert_eq!(limit.severity, DoctorSeverity::Ok);
        assert!(limit.message.contains("7"));

        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nmax_connections = \"many\"\n",
        )
        .expect("config should write");

        let checks = doctor_checks(&root, None);
        let limit = checks
            .iter()
            .find(|check| check.code == "server-max-connections")
            .expect("server max connections check should exist");

        assert_eq!(limit.severity, DoctorSeverity::Error);
        assert!(limit.message.contains("max_connections"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_state_manifest_status() {
        let root = make_temp_dir("doctor-state-manifest");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

state theme = "silver"

<input bind:value={theme} />
"#,
        )
        .expect("page should write");

        let checks = doctor_checks(&root, None);
        let state = checks
            .iter()
            .find(|check| check.code == "state-manifest")
            .expect("state manifest check should exist");

        assert_eq!(state.severity, DoctorSeverity::Ok);
        assert!(state.message.contains("1 signal"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_state_manifest_errors() {
        let root = make_temp_dir("doctor-state-manifest-error");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

state theme = Runtime.Env.public.THEME

<Copy>{theme}</Copy>
"#,
        )
        .expect("page should write");

        let checks = doctor_checks(&root, None);
        let state = checks
            .iter()
            .find(|check| check.code == "state-manifest")
            .expect("state manifest check should exist");

        assert_eq!(state.severity, DoctorSeverity::Error);
        assert!(state.message.contains("State manifest failed"));
        assert_eq!(
            state.hint,
            Some("Run `cargo ax state` to inspect state declarations and manifest output.")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_warns_when_ui_dependency_is_missing() {
        let workspace = make_temp_dir("doctor-missing-ui-dependency");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");

        let checks = doctor_checks(&app_root, None);
        let ui_dependency = checks
            .iter()
            .find(|check| check.code == "ui-cargo-dependency")
            .expect("ui dependency check should exist");

        assert_eq!(ui_dependency.severity, DoctorSeverity::Warn);
        assert!(ui_dependency.message.contains("axonyx-ui"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_warns_for_outdated_registry_dependencies() {
        let workspace = make_temp_dir("doctor-outdated-dependencies");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.5"
axonyx-ui = "0.0.32"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");

        let checks = doctor_checks(&app_root, None);
        let runtime_version = checks
            .iter()
            .find(|check| check.code == "runtime-version")
            .expect("runtime version check should exist");
        let ui_version = checks
            .iter()
            .find(|check| check.code == "ui-version")
            .expect("ui version check should exist");

        assert_eq!(runtime_version.severity, DoctorSeverity::Warn);
        assert!(runtime_version.message.contains("0.1.5"));
        assert_eq!(runtime_version.hint, Some("cargo update -p axonyx-runtime"));
        assert_eq!(ui_version.severity, DoctorSeverity::Warn);
        assert!(ui_version.message.contains("0.0.32"));
        assert_eq!(ui_version.hint, Some("cargo update -p axonyx-ui"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn upgrade_updates_registry_dependencies_only() {
        let workspace = make_temp_dir("upgrade-registry-dependencies");
        let app_root = workspace.join("demo-app");
        fs::create_dir_all(&app_root).expect("app dir should exist");
        let cargo_toml = app_root.join("Cargo.toml");

        fs::write(
            &cargo_toml,
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.5"
axonyx-ui = { version = "0.0.32" }
serde_json = "1"
"#,
        )
        .expect("cargo manifest should write");

        assert!(upgrade_cargo_dependency_version(
            &cargo_toml,
            "axonyx-runtime",
            AXONYX_RUNTIME_VERSION
        )
        .expect("runtime should upgrade"));
        assert!(
            upgrade_cargo_dependency_version(&cargo_toml, "axonyx-ui", AXONYX_UI_VERSION)
                .expect("ui should upgrade")
        );

        let updated = fs::read_to_string(&cargo_toml).expect("cargo manifest should read");
        assert!(updated.contains("axonyx-runtime = \"0.1.14\""));
        assert!(updated.contains("version = \"0.0.40\""));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn upgrade_can_repair_ui_layout_runtime_setup() {
        let workspace = make_temp_dir("upgrade-ui-layout");
        let app_root = workspace.join("demo-app");
        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");

        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Title>Demo</Title>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");

        assert!(ensure_ui_layout_setup(&app_root).expect("layout should upgrade"));

        let updated =
            fs::read_to_string(app_root.join("app/layout.ax")).expect("layout should read");
        assert!(updated.contains(r#"<Title>Demo</Title>"#));
        assert!(
            updated.contains(r#"<Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />"#)
        );
        assert!(updated.contains(AXONYX_UI_USE_DIRECTIVE));
        assert!(
            !updated.contains(r#"<Script src="/_ax/pkg/axonyx-ui/js/index.js" defer="true" />"#)
        );

        assert!(!ensure_ui_layout_setup(&app_root).expect("layout should already be current"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn upgrade_keeps_path_and_git_dependencies() {
        let workspace = make_temp_dir("upgrade-path-dependencies");
        let app_root = workspace.join("demo-app");
        fs::create_dir_all(&app_root).expect("app dir should exist");
        let cargo_toml = app_root.join("Cargo.toml");

        fs::write(
            &cargo_toml,
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = { git = "https://github.com/vladanPro/axonyx-runtime" }
axonyx-ui = { path = "vendor/axonyx-ui" }
"#,
        )
        .expect("cargo manifest should write");

        assert!(!upgrade_cargo_dependency_version(
            &cargo_toml,
            "axonyx-runtime",
            AXONYX_RUNTIME_VERSION
        )
        .expect("runtime should stay pinned to git"));
        assert!(
            !upgrade_cargo_dependency_version(&cargo_toml, "axonyx-ui", AXONYX_UI_VERSION)
                .expect("ui should stay pinned to path")
        );

        let updated = fs::read_to_string(&cargo_toml).expect("cargo manifest should read");
        assert!(updated.contains("git = \"https://github.com/vladanPro/axonyx-runtime\""));
        assert!(updated.contains("path = \"vendor/axonyx-ui\""));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_summary_counts_severities_and_deny_warnings_can_fail() {
        let checks = vec![
            DoctorCheck {
                code: "ok-check",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "warn-check",
                severity: DoctorSeverity::Warn,
                message: "warn".to_string(),
                hint: None,
            },
        ];

        let summary = doctor_summary(&checks);

        assert_eq!(summary.ok, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 0);
        assert!(!doctor_should_fail(&checks, false));
        assert!(doctor_should_fail(&checks, true));
    }

    #[test]
    fn doctor_framework_layer_status_lines_show_public_layers() {
        let checks = vec![
            DoctorCheck {
                code: "axonyx-config",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "cargo-manifest",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "runtime-dependency",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "ax-sources",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "server-body-limit",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "state-manifest",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "aegis-config",
                severity: DoctorSeverity::Warn,
                message: "optional".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "melt-graph",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
        ];

        let lines = doctor_framework_layer_status_lines(&checks).join("\n");

        assert!(lines.contains("Axonyx Pages: ready"));
        assert!(lines.contains("Axonyx Server: ready"));
        assert!(lines.contains("Axonyx State: ready"));
        assert!(lines.contains("Axonyx Foundry: optional"));
        assert!(lines.contains("Axonyx Melt: ready"));
        assert!(!lines.contains("Aegis:"));
    }

    #[test]
    fn doctor_reports_melt_graph_status() {
        let root = make_temp_dir("doctor-melt-graph");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"
"#,
        )
        .expect("cargo manifest should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let checks = doctor_checks(&root, None);
        let melt = checks
            .iter()
            .find(|check| check.code == "melt-graph")
            .expect("melt graph check should exist");

        assert_eq!(melt.severity, DoctorSeverity::Ok);
        assert!(melt.message.contains("1 page route"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_render_deploy_checks_report_production_start_contract() {
        let root = make_temp_dir("doctor-render-deploy");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.14"
"#,
        )
        .expect("cargo manifest should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");

        let checks = doctor_checks(&root, Some(DeployTarget::Render));

        assert!(checks.iter().any(|check| {
            check.code == "deploy-render-service"
                && check.severity == DoctorSeverity::Ok
                && check
                    .hint
                    .is_some_and(|hint| hint.contains("cargo ax run start --host"))
        }));
        assert!(checks.iter().any(|check| {
            check.code == "deploy-render-port" && check.severity == DoctorSeverity::Ok
        }));
        assert!(checks.iter().any(|check| {
            check.code == "deploy-render-production-server"
                && check.severity == DoctorSeverity::Ok
                && check.message.contains("Tokio")
        }));
        assert!(checks.iter().any(|check| {
            check.code == "deploy-render-health"
                && check.severity == DoctorSeverity::Ok
                && check.hint == Some("Health check path: /__axonyx/health")
        }));
        assert!(checks.iter().any(|check| {
            check.code == "deploy-render-melt" && check.message.contains("1 page route")
        }));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_ax_source_diagnostics() {
        let root = make_temp_dir("doctor-ax-diagnostics");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy></Card>\n")
            .expect("page should write");

        let checks = doctor_checks(&root, None);
        let ax_sources = checks
            .iter()
            .find(|check| check.code == "ax-sources")
            .expect("ax source check should exist");

        assert_eq!(ax_sources.severity, DoctorSeverity::Error);
        assert!(ax_sources.message.contains("diagnostic"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn jsx_layout_setup_adds_theme_and_stylesheet_inside_head() {
        let source = r#"page SiteLayout

<Head>
  <Title>Demo</Title>
</Head>

<Container max="xl">
  <Slot />
</Container>"#;

        let updated = ensure_ui_layout_setup_jsx(source);

        assert!(updated.contains("<Theme>silver</Theme>"));
        assert!(updated.contains(AXONYX_UI_USE_DIRECTIVE));
        assert!(!updated.contains(AXONYX_UI_STYLESHEET_HREF));
        assert!(!updated.contains(AXONYX_UI_SCRIPT_HREF));
        assert!(updated.contains("<Title>Demo</Title>"));
    }

    #[test]
    fn parse_form_body_decodes_urlencoded_pairs() {
        let fields = parse_form_body(b"title=Hello+Axonyx&excerpt=Fast%20forms");

        assert_eq!(
            fields.get("title").map(String::as_str),
            Some("Hello Axonyx")
        );
        assert_eq!(
            fields.get("excerpt").map(String::as_str),
            Some("Fast forms")
        );
    }

    #[test]
    fn detects_oversized_request_body_from_header_or_bytes() {
        let header_request = AxHttpRequest {
            method: "POST".to_string(),
            target: "/api/posts".to_string(),
            headers: [(
                "content-length".to_string(),
                (MAX_REQUEST_BODY_BYTES + 1).to_string(),
            )]
            .into_iter()
            .collect(),
            body: Vec::new(),
        };
        let body_request = AxHttpRequest {
            method: "POST".to_string(),
            target: "/api/posts".to_string(),
            headers: Default::default(),
            body: vec![0; MAX_REQUEST_BODY_BYTES + 1],
        };

        assert!(request_body_exceeds_limit(
            &header_request,
            MAX_REQUEST_BODY_BYTES
        ));
        assert!(request_body_exceeds_limit(
            &body_request,
            MAX_REQUEST_BODY_BYTES
        ));
    }

    #[test]
    fn parses_configured_request_body_limits() {
        assert_eq!(
            parse_max_body_bytes_value(&toml::Value::Integer(2048))
                .expect("integer limit should parse"),
            2048
        );
        assert_eq!(
            parse_max_body_bytes_value(&toml::Value::String("512kb".to_string()))
                .expect("kb limit should parse"),
            512 * 1024
        );
        assert_eq!(
            parse_max_body_bytes_value(&toml::Value::String("2mb".to_string()))
                .expect("mb limit should parse"),
            2 * 1024 * 1024
        );
        assert!(parse_max_body_bytes_value(&toml::Value::String("nope".to_string())).is_err());
    }

    #[test]
    fn parses_configured_request_timeouts() {
        assert_eq!(
            parse_request_timeout_seconds_value(&toml::Value::Integer(7))
                .expect("timeout should parse"),
            Duration::from_secs(7)
        );
        assert!(parse_request_timeout_seconds_value(&toml::Value::Integer(0)).is_err());
        assert!(
            parse_request_timeout_seconds_value(&toml::Value::String("7".to_string())).is_err()
        );
    }

    #[test]
    fn parses_configured_shutdown_grace_periods() {
        assert_eq!(
            parse_shutdown_grace_seconds_value(&toml::Value::Integer(11))
                .expect("shutdown grace should parse"),
            Duration::from_secs(11)
        );
        assert!(parse_shutdown_grace_seconds_value(&toml::Value::Integer(0)).is_err());
        assert!(
            parse_shutdown_grace_seconds_value(&toml::Value::String("11".to_string())).is_err()
        );
    }

    #[test]
    fn parses_configured_max_connections() {
        assert_eq!(
            parse_max_connections_value(&toml::Value::Integer(128))
                .expect("max connections should parse"),
            128
        );
        assert!(parse_max_connections_value(&toml::Value::Integer(0)).is_err());
        assert!(parse_max_connections_value(&toml::Value::String("128".to_string())).is_err());
    }

    #[test]
    fn tokio_server_can_shutdown_without_request() {
        let root = make_temp_dir("tokio-server-shutdown");
        let state = Arc::new(test_dev_state(&root));
        let config = AxServerConfig::new("127.0.0.1", 0, AxServerMode::Start);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime should build");

        runtime
            .block_on(serve_tokio_until(
                config,
                state,
                Duration::from_secs(DEFAULT_SHUTDOWN_GRACE_SECONDS),
                DEFAULT_MAX_CONNECTIONS,
                async {},
            ))
            .expect("tokio server should shut down cleanly");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn axum_tokio_server_can_shutdown_without_request() {
        let root = make_temp_dir("axum-tokio-server-shutdown");
        let state = Arc::new(test_dev_state(&root));
        let config = AxServerConfig::new("127.0.0.1", 0, AxServerMode::Start);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime should build");

        runtime
            .block_on(serve_axum_tokio_until(config, state, async {}))
            .expect("axum tokio server should shut down cleanly");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn tokio_shutdown_waits_for_active_connection_guard() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("tokio runtime should build");

        runtime.block_on(async {
            let tracker = TokioConnectionTracker::new(Duration::from_millis(200), 1);
            let guard = tracker.try_track().expect("connection should fit");

            tokio::spawn(async move {
                let _guard = guard;
                tokio::time::sleep(Duration::from_millis(25)).await;
            });

            wait_for_tokio_connections(&tracker).await;
            assert_eq!(tracker.active_count(), 0);
        });
    }

    #[test]
    fn tokio_connection_tracker_rejects_over_limit() {
        let tracker = TokioConnectionTracker::new(Duration::from_secs(1), 1);
        let guard = tracker.try_track().expect("first connection should fit");

        assert!(tracker.try_track().is_none());
        assert_eq!(tracker.active_count(), 1);

        drop(guard);
        assert_eq!(tracker.active_count(), 0);
        assert!(tracker.try_track().is_some());
    }

    #[test]
    fn write_ax_response_uses_chunked_transfer_for_streaming_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");
        let response = AxHttpResponse::stream_chunks(
            200,
            "text/plain; charset=utf-8",
            vec![b"Hello".to_vec(), b" Axonyx".to_vec()],
        );

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test client should connect");
            write_ax_response(&mut stream, &response, false).expect("response should write");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.contains("Transfer-Encoding: chunked\r\n"));
        assert!(!raw.contains("Content-Length:"));
        assert!(raw.ends_with("5\r\nHello\r\n7\r\n Axonyx\r\n0\r\n\r\n"));
    }

    #[test]
    fn write_ax_response_can_suppress_body_for_head_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");
        let response = AxHttpResponse::text(200, "Hello Axonyx");

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test client should connect");
            write_ax_response(&mut stream, &response, true).expect("response should write");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.starts_with("HTTP/1.1 200 OK"));
        assert!(raw.contains("Content-Length: 12\r\n"));
        assert!(!raw.ends_with("Hello Axonyx"));
    }

    #[test]
    fn head_requests_route_as_get_without_response_body() {
        let request = AxHttpRequest {
            method: "HEAD".to_string(),
            target: "/".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };

        assert!(suppress_response_body_for_method(&request.method));
        assert_eq!(normalize_request_for_routing(request).method, "GET");
    }

    #[test]
    fn page_method_not_allowed_reports_allow_header() {
        let root = make_temp_dir("method-not-allowed");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
            runtime_config: AxServerRuntimeConfig::from_root(&root)
                .expect("runtime config should load"),
        };
        let request = AxHttpRequest {
            method: "PUT".to_string(),
            target: "/".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };

        let response =
            handle_http_request(&state, AxServerMode::Start, request).expect("request should run");

        assert_eq!(response.status, 405);
        assert_eq!(response.header_value("Allow"), Some("GET, HEAD"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn health_endpoint_reports_ok_json() {
        let root = make_temp_dir("health-endpoint");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        let state = test_dev_state(&root);
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/__axonyx/health?probe=1".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };

        let response =
            handle_http_request(&state, AxServerMode::Start, request).expect("request should run");
        let status = response.status;
        let content_type = response.content_type.clone();
        assert_eq!(response.header_value("Cache-Control"), Some("no-store"));
        let body = serde_json::from_slice::<serde_json::Value>(&response.body.into_bytes())
            .expect("health response should be json");

        assert_eq!(status, 200);
        assert_eq!(content_type, "application/json; charset=utf-8");
        assert_eq!(body["ok"], true);
        assert_eq!(body["service"], "axonyx");
        assert_eq!(body["mode"], "start");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn server_policy_adds_baseline_security_headers() {
        let root = make_temp_dir("security-headers");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        let state = test_dev_state(&root);
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let response = AxHttpResponse::html(200, "<main>ok</main>");

        let response = apply_server_response_policy(&state, &request, response, false)
            .expect("policy should apply");

        assert_eq!(
            response.header_value("X-Content-Type-Options"),
            Some("nosniff")
        );
        assert_eq!(response.header_value("X-Frame-Options"), Some("DENY"));
        assert_eq!(
            response.header_value("Referrer-Policy"),
            Some("strict-origin-when-cross-origin")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn server_policy_gzips_large_text_responses_when_accepted() {
        let root = make_temp_dir("gzip-response");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        let state = test_dev_state(&root);
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: BTreeMap::from([("accept-encoding".to_string(), "br, gzip".to_string())]),
            body: Vec::new(),
        };
        let response = AxHttpResponse::html(200, "Axonyx ".repeat(512));

        let response = apply_server_response_policy(&state, &request, response, false)
            .expect("policy should apply");

        assert_eq!(response.header_value("Content-Encoding"), Some("gzip"));
        assert_eq!(response.header_value("Vary"), Some("Accept-Encoding"));
        assert!(response.body.into_bytes().starts_with(&[0x1f, 0x8b]));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn server_policy_skips_gzip_when_disabled() {
        let root = make_temp_dir("gzip-disabled");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\ncompression = false\n",
        )
        .expect("config should write");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
            runtime_config: AxServerRuntimeConfig::from_root(&root)
                .expect("runtime config should load"),
        };
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/".to_string(),
            headers: BTreeMap::from([("accept-encoding".to_string(), "gzip".to_string())]),
            body: Vec::new(),
        };
        let response = AxHttpResponse::html(200, "Axonyx ".repeat(512));

        let response = apply_server_response_policy(&state, &request, response, false)
            .expect("policy should apply");

        assert_eq!(response.header_value("Content-Encoding"), None);

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn request_log_line_renders_text_summary() {
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/docs".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let response = AxHttpResponse::html(200, "Axonyx docs");

        let line = render_request_log_line(
            AxServerLogFormat::Text,
            &request,
            &response,
            Duration::from_millis(14),
        );

        assert!(line.contains("[axonyx] GET /docs 200 14ms"));
        assert!(line.contains("text/html; charset=utf-8"));
        assert!(line.contains("11 bytes"));
    }

    #[test]
    fn request_log_line_renders_json_summary() {
        let request = AxHttpRequest {
            method: "POST".to_string(),
            target: "/api/posts".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let response = AxHttpResponse::json(201, &serde_json::json!({ "ok": true }))
            .expect("json response should render");

        let line = render_request_log_line(
            AxServerLogFormat::Json,
            &request,
            &response,
            Duration::from_millis(3),
        );
        let value =
            serde_json::from_str::<serde_json::Value>(&line).expect("request log should be json");

        assert_eq!(value["source"], "axonyx");
        assert_eq!(value["method"], "POST");
        assert_eq!(value["path"], "/api/posts");
        assert_eq!(value["status"], 201);
        assert_eq!(value["duration_ms"], 3);
    }

    #[test]
    fn render_response_header_writes_multiple_set_cookie_headers() {
        let response = AxHttpResponse::text(200, "ok")
            .with_cookie(axonyx_runtime::server::AxCookie::new("a", "1").with_path("/"))
            .with_cookie(axonyx_runtime::server::AxCookie::new("b", "2").with_path("/"));

        let header = render_response_header(&response);

        assert!(header.contains("Set-Cookie: a=1; Path=/\r\n"));
        assert!(header.contains("Set-Cookie: b=2; Path=/\r\n"));
    }

    #[test]
    fn dev_stream_probe_route_uses_chunked_transfer() {
        let root = make_temp_dir("stream-probe-route");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Dev).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET /__axonyx/stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.contains("Transfer-Encoding: chunked\r\n"));
        assert!(raw.contains("axonyx-stream:start\n"));
        assert!(raw.contains("axonyx-stream:chunk\n"));
        assert!(raw.contains("axonyx-stream:end\n"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn dev_stream_html_probe_route_uses_chunked_html() {
        let root = make_temp_dir("stream-html-probe-route");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Dev).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET /__axonyx/stream/html HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(raw.contains("Transfer-Encoding: chunked\r\n"));
        assert!(raw.contains("Shell arrived first."));
        assert!(raw.contains("Then the streamed content chunk arrived"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn missing_page_route_renders_not_found_boundary() {
        let root = make_temp_dir("not-found-boundary");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/layout.ax"),
            "page RootLayout\n<Container><Copy>Shell</Copy><Slot /></Container>\n",
        )
        .expect("layout should write");
        fs::write(
            root.join("app/not-found.ax"),
            "page NotFound\n<Copy>Custom Axonyx not found</Copy>\n",
        )
        .expect("not-found boundary should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Start).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.starts_with("HTTP/1.1 404 Not Found"));
        assert!(raw.contains("Shell"));
        assert!(raw.contains("Custom Axonyx not found"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_error_renders_error_boundary() {
        let root = make_temp_dir("error-boundary");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/layout.ax"),
            "page RootLayout\n<Container><Copy>Shell</Copy><Slot /></Container>\n",
        )
        .expect("layout should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy></Card>\n")
            .expect("broken page should write");
        fs::write(
            root.join("app/error.ax"),
            "page Error\n<Copy>Custom Axonyx error</Copy>\n",
        )
        .expect("error boundary should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Start).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.starts_with("HTTP/1.1 500 Internal Server Error"));
        assert!(raw.contains("Shell"));
        assert!(raw.contains("Custom Axonyx error"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn action_request_can_return_state_patch_response() {
        let root = make_temp_dir("action-patch-response");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\npage state theme: String = \"silver\"\n<Copy>Home</Copy>\n",
        )
        .expect("page should write");
        fs::write(
            root.join("app/actions.ax"),
            r#"
action SetTheme
  input:
    theme: string

  patch theme = input.theme
  return ok
"#,
        )
        .expect("actions should write");
        let route = resolve_route(&root, "/")
            .expect("route should resolve")
            .expect("route should exist");
        let manifest = collect_route_state_manifest(&route).expect("state manifest should collect");
        assert_eq!(
            manifest.resolve_signal_key("root:theme:1").as_deref(),
            Some("page:root:theme:1")
        );
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Dev).expect("request should handle");
        });

        let body = "__ax_patch=1&theme=gold";
        let request = format!(
            "POST /__axonyx/action?path=%2F&name=SetTheme HTTP/1.1\r\nHost: localhost\r\nAccept: application/ax-patch+json\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(request.as_bytes())
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.starts_with("HTTP/1.1 200 OK"));
        assert!(raw.contains("Content-Type: application/ax-patch+json; charset=utf-8"));
        assert!(raw.contains("\"redirect\":\"/\""));
        assert!(
            raw.contains("\"signal\":\"page:root:theme:1\""),
            "raw response was: {raw}"
        );
        assert!(raw.contains("\"value\":\"gold\""));
        assert!(raw.contains("\"source\":\"action\""));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn action_patch_response_rejects_known_state_type_mismatch() {
        let root = make_temp_dir("action-patch-type-mismatch");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

page state count: Number = 0

<input bind:value={count} />
"#,
        )
        .expect("page should write");
        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let result = AxPreviewActionResult {
            redirect_to: None,
            value: AxValue::Null,
            patches: vec![AxPreviewStatePatch::set(
                "root:count:1",
                AxValue::String("not-a-number".to_string()),
            )],
        };

        let error =
            action_patch_response(&route, &result).expect_err("mismatched state patch should fail");

        assert!(error
            .to_string()
            .contains("state patch for 'page:root:count:1' expected Number but got String"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn page_route_query_streams_through_http_handler() {
        let root = make_temp_dir("page-route-query-stream");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\n<Copy>Streamed from page route</Copy>\n",
        )
        .expect("page should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Dev).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET /?__ax_stream=1 HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(raw.contains("Transfer-Encoding: chunked\r\n"));
        assert!(raw.contains("Streamed from page route"));
        assert!(raw.contains("/__axonyx/version"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn page_route_config_streams_through_http_handler() {
        let root = make_temp_dir("page-route-config-stream");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = true\n",
        )
        .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\n<Copy>Config streamed page route</Copy>\n",
        )
        .expect("page should write");
        let state = test_dev_state(&root);
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener address should resolve");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("test client should connect");
            handle_connection(stream, &state, AxServerMode::Dev).expect("request should handle");
        });

        let mut client = TcpStream::connect(address).expect("client should connect");
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("client request should write");
        let mut raw = String::new();
        client
            .read_to_string(&mut raw)
            .expect("client should read response");
        server.join().expect("server thread should join");

        assert!(raw.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(raw.contains("Transfer-Encoding: chunked\r\n"));
        assert!(raw.contains("Config streamed page route"));
        assert!(raw.contains("/__axonyx/version"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn executes_backend_route_request_from_routes_directory() {
        let root = make_temp_dir("api-route");
        fs::create_dir_all(root.join("routes").join("api")).expect("routes dir should exist");
        fs::write(
            root.join("routes").join("api").join("posts.ax"),
            "route GET \"/api/posts\"\n  data posts = Db.Stream(\"posts\")\n    where status = \"published\"\n    limit 2\n  return posts\n",
        )
        .expect("route should write");

        let state = test_dev_state(&root);
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/api/posts".to_string(),
            headers: std::collections::BTreeMap::new(),
            body: Vec::new(),
        };

        let response = execute_backend_route_request(&state, &request)
            .expect("backend route request should succeed")
            .expect("backend route should match");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/json; charset=utf-8");
        let body = String::from_utf8(response.body).expect("json response should be utf-8");
        assert!(body.contains("Hello Axonyx"));
        assert!(body.contains("Docs Without Bloat"));
        assert!(!body.contains("Draft Preview"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn executes_dynamic_backend_route_request_with_query_context() {
        let root = make_temp_dir("api-route-dynamic");
        fs::create_dir_all(root.join("routes").join("api").join("posts"))
            .expect("routes dir should exist");
        fs::write(
            root.join("routes")
                .join("api")
                .join("posts")
                .join("show.ax"),
            "route GET \"/api/posts/:slug\"\n  data posts = Db.Stream(\"posts\")\n    where slug = params.slug\n    where status = query.status\n  return posts\n",
        )
        .expect("route should write");

        let state = test_dev_state(&root);
        let request = AxHttpRequest {
            method: "GET".to_string(),
            target: "/api/posts/draft-preview?status=draft".to_string(),
            headers: std::collections::BTreeMap::new(),
            body: Vec::new(),
        };

        let response = execute_backend_route_request(&state, &request)
            .expect("backend route request should succeed")
            .expect("backend route should match");

        assert_eq!(response.status, 200);
        let body = String::from_utf8(response.body).expect("json response should be utf-8");
        assert!(body.contains("Draft Preview"));
        assert!(!body.contains("Hello Axonyx"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn executes_backend_route_request_with_request_context() {
        let root = make_temp_dir("api-route-request-context");
        fs::create_dir_all(root.join("routes").join("api")).expect("routes dir should exist");
        fs::write(
            root.join("routes").join("api").join("session.ax"),
            "route POST \"/api/session\"\n  data theme = request.cookies.theme\n  data agent = request.headers.user_agent\n  data title = request.form.title\n  header \"X-Agent\" = agent\n  cookie \"theme\" = theme\n  return json(title)\n",
        )
        .expect("route should write");

        let state = test_dev_state(&root);
        let request = AxHttpRequest::new("POST", "/api/session")
            .with_header("Cookie", "theme=gold")
            .with_header("User-Agent", "AxonyxTest")
            .with_body(b"title=Hello+Axonyx".to_vec());

        let response = execute_backend_route_request(&state, &request)
            .expect("backend route request should succeed")
            .expect("backend route should match");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers.get("X-Agent").map(String::as_str),
            Some("AxonyxTest")
        );
        assert!(response
            .set_cookies
            .iter()
            .any(|cookie| cookie == "theme=gold; Path=/"));
        let body = String::from_utf8(response.body).expect("json response should be utf-8");
        assert_eq!(body, "\"Hello Axonyx\"");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_dynamic_page_route_with_loader_params_and_query_context() {
        let root = make_temp_dir("dynamic-page-render");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/loader.ax"),
            "loader PostDetail\n  data posts = Db.Stream(\"posts\")\n    where slug = params.slug\n    where status = query.status\n  return posts\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n  data posts = load PostDetail\n  each post in posts\n    Copy -> post.title\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/posts/draft-preview?status=draft")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("dynamic route should render");

        assert!(html.contains("Draft Preview"));
        assert!(!html.contains("Hello Axonyx"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_html_injects_action_runtime_for_action_forms() {
        let root = make_temp_dir("action-runtime-render");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/actions.ax"),
            r#"
action SetTheme
  input:
    theme: string

  patch theme = input.theme
  return ok
"#,
        )
        .expect("actions should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
  form method: "post", action: action SetTheme
    input name: "theme", value: "gold"
    Button type: "submit" -> "Set theme"
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("route should render");

        assert!(html.contains("/__axonyx/action?path=%2F&amp;name=SetTheme"));
        assert!(html.contains("data-ax-runtime=\"actions\""));
        assert!(html.contains("window.__axonyxActionRuntime"));
        assert!(html.contains("application/ax-patch+json"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_response_wraps_page_html_in_http_response() {
        let root = make_temp_dir("route-response");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\n<Copy>Hello response</Copy>\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let response = render_route_response(&state, &route, true, false)
            .expect("route response should render");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        let body = response
            .body
            .chunks_iter()
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<String>();
        assert!(body.contains("Hello response"));
        assert!(body.contains("/__axonyx/version"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_response_can_stream_page_html_chunks() {
        let root = make_temp_dir("route-response-stream");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\n<Copy>Hello streamed page</Copy>\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/?__ax_stream=1")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let response = render_route_response(
            &state,
            &route,
            false,
            should_stream_page_route(&root, &route.request_target),
        )
        .expect("streamed route response should render");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert!(response.body.is_streaming());
        assert!(response.body.chunks_iter().count() >= 2);
        let body = response
            .body
            .chunks_iter()
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<String>();
        assert!(body.contains("Hello streamed page"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn server_config_can_enable_page_streaming_by_default() {
        let root = make_temp_dir("route-response-stream-config");
        fs::write(
            root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[server]\nstream_pages = true\n",
        )
        .expect("config should write");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("app/page.ax"),
            "page Home\n<Copy>Hello config stream</Copy>\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let response = render_route_response(
            &state,
            &route,
            false,
            should_stream_page_route(&root, &route.request_target),
        )
        .expect("streamed route response should render");

        assert!(response.body.is_streaming());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_app_component() {
        let root = make_temp_dir("imported-app-component");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SiteCard } from "@/components/site-card.ax"

page Home
<SiteCard title="Hello from import">
  <Copy>Inner body</Copy>
</SiteCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html =
            render_route_html(&state, &route).expect("imported component route should render");

        assert!(html.contains("Hello from import"));
        assert!(html.contains("Inner body"));
        assert!(!html.contains("data-import-source"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_applies_theme_config_when_head_has_no_theme() {
        let root = make_temp_dir("theme-config-render");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[theme]
active = "silver"
stylesheet = "/css/axonyx-ui/index.css"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Hello theme</Copy>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("route should render");

        assert!(html.contains(r#"<html data-theme="silver" lang="en">"#));
        assert!(html.contains(r#"<link rel="stylesheet" href="/css/axonyx-ui/index.css">"#));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_keeps_explicit_head_theme_over_config_theme() {
        let root = make_temp_dir("theme-config-explicit");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[theme]
active = "silver"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

<Head>
  <Theme>gold</Theme>
</Head>

<Copy>Hello explicit theme</Copy>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("route should render");

        assert!(html.contains(r#"data-theme="gold""#));
        assert!(!html.contains(r#"data-theme="silver""#));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn route_version_changes_when_imported_component_changes() {
        let root = make_temp_dir("route-version-imports");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title="Initial" />
"#,
        )
        .expect("component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SiteCard } from "@/components/site-card.ax"

page Home
<SiteCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let before = route_version(&root, &route).expect("initial version should hash");

        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title="Updated" />
"#,
        )
        .expect("component should update");

        let after = route_version(&root, &route).expect("updated version should hash");
        assert_ne!(before, after);

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component() {
        let workspace = make_temp_dir("ui-package-workspace");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/ax/foundry")).expect("ui ax dir should exist");
        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported from UI">
  <Copy>Silver contract</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html =
            render_route_html(&state, &route).expect("package component route should render");

        assert!(html.contains("Imported from UI"));
        assert!(html.contains("Silver contract"));
        assert!(!html.contains("data-import-source"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component_from_src_foundry_layout() {
        let workspace = make_temp_dir("ui-package-src-foundry");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        fs::create_dir_all(ui_root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::write(
            ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported from src/foundry">
  <Copy>Modern package layout</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html =
            render_route_html(&state, &route).expect("src/foundry package route should render");

        assert!(html.contains("Imported from src/foundry"));
        assert!(html.contains("Modern package layout"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component_from_cargo_dependency() {
        let workspace = make_temp_dir("ui-package-cargo-dependency");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");
        fs::write(
            ui_root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(ui_root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            ui_root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
css_root = "src/css"
css_entry = "src/css/index.css"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported through Cargo">
  <Copy>No package override needed</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html =
            render_route_html(&state, &route).expect("cargo package component route should render");

        assert!(html.contains("Imported through Cargo"));
        assert!(html.contains("No package override needed"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn use_axonyx_ui_injects_package_css_and_js() {
        let root = make_temp_dir("use-axonyx-ui-assets");
        let ui_root = root.join("vendor/axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Use UI", "body { color: silver; }");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n<Slot />\n")
            .expect("layout should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
use "@axonyx/ui"

page Home

<Card title="Use directive">
  <Copy>Package assets should be automatic.</Copy>
</Card>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("route should render");
        let css_file_name = hashed_asset_file_name(&ui_root.join("src/css/index.css"))
            .expect("css hash should compute")
            .expect("css hashed file name should exist");
        let js_file_name = hashed_asset_file_name(&ui_root.join("src/js/index.js"))
            .expect("js hash should compute")
            .expect("js hashed file name should exist");

        assert!(html.contains(&format!(
            r#"<link rel="stylesheet" href="/_ax/pkg/axonyx-ui/{}">"#,
            css_file_name.to_string_lossy()
        )));
        assert!(html.contains(&format!(
            r#"<script src="/_ax/pkg/axonyx-ui/js/{}" defer="true"></script>"#,
            js_file_name.to_string_lossy()
        )));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn vendored_axonyx_ui_component_wins_over_cargo_dependency() {
        let workspace = make_temp_dir("ui-package-vendor-before-cargo");
        let root = workspace.join("axonyx-site");
        let cargo_ui_root = workspace.join("axonyx-ui");
        let vendor_ui_root = root.join("vendor/axonyx-ui");
        let cargo_ui_path = cargo_ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("src")).expect("app src dir should exist");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("app target should write");
        fs::create_dir_all(cargo_ui_root.join("src/foundry")).expect("cargo ui dir should exist");
        fs::create_dir_all(vendor_ui_root.join("src/foundry")).expect("vendor ui dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{cargo_ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");
        fs::write(
            cargo_ui_root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(cargo_ui_root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            cargo_ui_root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            cargo_ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Cargo package" />
"#,
        )
        .expect("cargo ui component should write");
        fs::write(
            vendor_ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Vendored package" />
"#,
        )
        .expect("vendor ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = test_dev_state(&root);
        let html = render_route_html(&state, &route).expect("vendored package route should render");

        assert!(html.contains("Vendored package"));
        assert!(!html.contains("Cargo package"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn route_version_changes_when_imported_axonyx_ui_component_changes() {
        let workspace = make_temp_dir("ui-package-version");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/ax/foundry")).expect("ui ax dir should exist");
        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Version A" />
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let before = route_version(&root, &route).expect("initial version should hash");

        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Version B" />
"#,
        )
        .expect("ui component should update");

        let after = route_version(&root, &route).expect("updated version should hash");
        assert_ne!(before, after);

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_page_parse_error_line() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(&path, "page Home\n<Copy></Card>\n", None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-parse");
    }

    #[test]
    fn check_ax_source_reports_reserved_import_name() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
import { Link } from "@axonyx/ui/foundry/Link.ax"

page Home
<Link href="/">Home</Link>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].code, "axonyx-parse");
        assert!(diagnostics[0].message.contains("reserved"));
        assert!(diagnostics[0].message.contains("Link"));
    }

    #[test]
    fn check_ax_source_reports_invalid_type_annotation() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

let posts: List<Post>> = load PostsList

<Copy>Body</Copy>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].code, "axonyx-type");
    }

    #[test]
    fn check_ax_source_reports_unknown_typed_each_member() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.summary} />
</Each>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-type");
        assert_eq!(diagnostics[0].line, 11);
        assert!(diagnostics[0].message.contains("post.summary"));
        assert!(diagnostics[0].message.contains("summary"));
        assert!(diagnostics[0].message.contains("unknown field"));
    }

    #[test]
    fn check_ax_source_allows_optional_typed_each_member() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post?.summary} />
</Each>
"#,
            None,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn check_ax_source_allows_optional_type_field() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
  summary?: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.summary} />
</Each>
"#,
            None,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn check_ax_source_reports_duplicate_type_field() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
  title: String
}

<Copy>Body</Copy>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-type");
        assert!(diagnostics[0].message.contains("duplicate field"));
        assert!(diagnostics[0].message.contains("title"));
    }

    #[test]
    fn check_ax_source_reports_backend_parse_error_line() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics =
            check_ax_source_with_root(&path, "route GET \"/api/posts\"\n    return posts\n", None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-backend-parse");
    }

    #[test]
    fn check_ax_source_accepts_backend_return_contracts() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
loader PostsList -> List<Post>
  return posts

route GET "/api/posts" -> Post[]
  return json(posts)

action CreatePost -> Post
  input:
    title: string

  return json(post)
"#,
            None,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn check_ax_source_reports_invalid_backend_return_contract() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
route GET "/api/posts" -> List<>
  return json(posts)
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-return-contract-type");
        assert_eq!(diagnostics[0].line, 2);
        assert!(diagnostics[0].message.contains("List<>"));
    }

    #[test]
    fn check_app_sources_accepts_known_backend_return_contract_type() {
        let root = make_temp_dir("known-return-contract-type");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("routes/api")).expect("api dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

type Post {
  title: String
}

<Copy>Home</Copy>
"#,
        )
        .expect("page should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts" -> Post[]
  return json(posts)
"#,
        )
        .expect("route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_unknown_backend_return_contract_type() {
        let root = make_temp_dir("unknown-return-contract-type");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("routes/api")).expect("api dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("page should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts" -> Post[]
  return json(posts)
"#,
        )
        .expect("route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-return-contract-unknown-type");
        assert_eq!(diagnostics[0].line, 2);
        assert!(diagnostics[0].message.contains("Post"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_missing_signed_session_secret() {
        let _guard = lock_test_env();
        let secret_prev = std::env::var("AX_SECRET_SESSION_KEY").ok();
        std::env::remove_var("AX_SECRET_SESSION_KEY");

        let root = make_temp_dir("check-auth-signed-session-secret");
        fs::create_dir_all(root.join("routes/api")).expect("routes dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("routes/api/admin.ax"),
            r#"
route GET "/api/admin"
  require Auth.signedSession else redirect("/login")
  return json("ok")
"#,
        )
        .expect("route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].code, "axonyx-auth-secret");
        assert!(diagnostics[0].message.contains("AX_SECRET_SESSION_KEY"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
        if let Some(value) = secret_prev {
            std::env::set_var("AX_SECRET_SESSION_KEY", value);
        }
    }

    #[test]
    fn check_app_sources_accepts_signed_session_secret_from_env_file() {
        let _guard = lock_test_env();
        let secret_prev = std::env::var("AX_SECRET_SESSION_KEY").ok();
        std::env::remove_var("AX_SECRET_SESSION_KEY");

        let root = make_temp_dir("check-auth-signed-session-env-file");
        fs::create_dir_all(root.join("routes/api")).expect("routes dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(root.join(".env"), "AX_SECRET_SESSION_KEY=local-secret\n")
            .expect("env should write");
        fs::write(
            root.join("routes/api/admin.ax"),
            r#"
route GET "/api/admin"
  require Auth.signedSession else redirect("/login")
  return json("ok")
"#,
        )
        .expect("route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");

        fs::remove_dir_all(root).expect("temp dir should clean up");
        if let Some(value) = secret_prev {
            std::env::set_var("AX_SECRET_SESSION_KEY", value);
        }
    }

    #[test]
    fn check_ax_source_reports_route_input_missing_section() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
route POST "/api/posts"
  return json(input.title)
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].code, "axonyx-route-input-missing");
        assert!(diagnostics[0].message.contains("input:"));
    }

    #[test]
    fn check_ax_source_reports_route_input_unsupported_type() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
route POST "/api/posts"
  input:
    title: PostTitle

  return json(input.title)
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].code, "axonyx-route-input-type");
        assert!(diagnostics[0].message.contains("PostTitle"));
    }

    #[test]
    fn check_ax_source_reports_duplicate_route_input_field() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
route POST "/api/posts"
  input:
    title: string
    title: string

  return json(input.title)
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].code, "axonyx-route-input-duplicate");
        assert!(diagnostics[0].message.contains("title"));
    }

    #[test]
    fn check_ax_source_reports_missing_app_import() {
        let root = make_temp_dir("check-missing-app-import");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import");
        assert!(diagnostics[0].message.contains("@/components/SiteCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_accepts_existing_app_import() {
        let root = make_temp_dir("check-existing-app-import");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SiteCard\n<Card />\n",
        )
        .expect("component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn component_override_resolves_exact_import_source() {
        let root = make_temp_dir("component-override");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("override should resolve");

        assert_eq!(resolved, root.join("app/components/SiteCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn package_override_resolves_package_namespace_import() {
        let root = make_temp_dir("package-override");
        fs::create_dir_all(root.join("vendor/custom-ui/src/ax/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("package override should resolve");

        assert_eq!(
            resolved,
            root.join("vendor/custom-ui/src/ax/foundry/SectionCard.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn package_override_resolves_package_namespace_import_from_src_layout() {
        let root = make_temp_dir("package-override-src-layout");
        fs::create_dir_all(root.join("vendor/custom-ui/src/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("package override should resolve");

        assert_eq!(
            resolved,
            root.join("vendor/custom-ui/src/foundry/SectionCard.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_accepts_component_override_import() {
        let root = make_temp_dir("check-component-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SectionCard\n<Card />\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_invalid_component_override_target() {
        let root = make_temp_dir("check-invalid-component-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SectionCard\n<Copy></Card>\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-parse");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("line 2"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_invalid_package_override_target() {
        let root = make_temp_dir("check-invalid-package-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("vendor/custom-ui/src/ax/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("vendor/custom-ui/src/ax/foundry/SectionCard.ax"),
            "page SectionCard\n<Copy></Card>\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-parse");
        assert!(diagnostics[0]
            .message
            .contains("vendor/custom-ui/src/ax/foundry/SectionCard.ax"));
        assert!(diagnostics[0].message.contains("line 2"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_nested_missing_import_chain() {
        let root = make_temp_dir("check-nested-missing-import-chain");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            r#"
import { InnerCard } from "@/components/InnerCard.ax"

page SiteCard
<InnerCard />
"#,
        )
        .expect("component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-chain");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("InnerCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_import_cycle() {
        let root = make_temp_dir("check-import-cycle");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            r#"
import { InnerCard } from "@/components/InnerCard.ax"

page SiteCard
<InnerCard />
"#,
        )
        .expect("site card should write");
        fs::write(
            root.join("app/components/InnerCard.ax"),
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page InnerCard
<SiteCard />
"#,
        )
        .expect("inner card should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-cycle");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("InnerCard.ax"));
        assert!(diagnostics[0].message.contains("cycle"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn content_manifest_indexes_configured_collections() {
        let root = make_temp_dir("content-manifest");
        fs::create_dir_all(root.join("content/docs/nested")).expect("content dirs should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "content/docs"
extensions = ["md", "mdx"]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("content/docs/intro.md"),
            "---\ntitle: Intro\ndescription: Start here\n---\n# Intro\n",
        )
        .expect("intro should write");
        fs::write(root.join("content/docs/nested/setup.mdx"), "# Setup\n")
            .expect("setup should write");
        fs::write(root.join("content/docs/ignored.txt"), "skip").expect("ignored should write");

        let manifest = collect_content_manifest(&root).expect("manifest should collect");

        assert_eq!(manifest.collections.len(), 1);
        let collection = &manifest.collections[0];
        assert_eq!(collection.name, "docs");
        assert_eq!(collection.path, "content/docs");
        assert_eq!(collection.extensions, vec!["md", "mdx"]);
        assert_eq!(collection.entries.len(), 2);
        assert_eq!(collection.entries[0].path, "content/docs/intro.md");
        assert_eq!(collection.entries[0].slug, "intro");
        assert_eq!(collection.entries[0].content_type, "markdown");
        assert_eq!(collection.entries[0].title, "Intro");
        assert_eq!(collection.entries[0].excerpt, "Start here");
        assert_eq!(collection.entries[0].word_count, 1);
        assert_eq!(collection.entries[0].html, "<h1>Intro</h1>");
        assert_eq!(
            collection.entries[0].frontmatter.get("title"),
            Some(&"Intro".to_string())
        );
        assert_eq!(collection.entries[0].body, "# Intro\n");
        assert_eq!(collection.entries[1].path, "content/docs/nested/setup.mdx");
        assert_eq!(collection.entries[1].slug, "nested/setup");
        assert_eq!(collection.entries[1].title, "Setup");
        assert_eq!(collection.entries[1].excerpt, "Setup");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn preview_store_loads_configured_content_collections() {
        let root = make_temp_dir("content-preview-store");
        fs::create_dir_all(root.join("content/docs")).expect("content dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[content.collections.docs]
path = "content/docs"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("content/docs/getting-started.md"),
            "---\ntitle: Getting Started\ndescription: Build your first page\n---\n# Start\n",
        )
        .expect("doc should write");

        let store = preview_store_from_content(&root).expect("preview store should load content");
        let items = store.collection_items("docs");

        assert_eq!(items.len(), 1);
        let AxValue::Record(fields) = &items[0] else {
            panic!("expected content item record");
        };
        assert_eq!(fields.get("slug"), Some(&AxValue::from("getting-started")));
        assert_eq!(fields.get("extension"), Some(&AxValue::from("md")));
        assert_eq!(fields.get("content_type"), Some(&AxValue::from("markdown")));
        assert_eq!(fields.get("title"), Some(&AxValue::from("Getting Started")));
        assert_eq!(
            fields.get("excerpt"),
            Some(&AxValue::from("Build your first page"))
        );
        assert_eq!(fields.get("word_count"), Some(&AxValue::from(1i64)));
        assert_eq!(
            fields.get("description"),
            Some(&AxValue::from("Build your first page"))
        );
        assert_eq!(fields.get("body"), Some(&AxValue::from("# Start\n")));
        assert_eq!(fields.get("html"), Some(&AxValue::from("<h1>Start</h1>")));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_writes_content_manifest() {
        let root = make_temp_dir("static-build-content-manifest");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("content/docs")).expect("content dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "content/docs"
extensions = ["md"]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Home page</Copy>
"#,
        )
        .expect("page should write");
        fs::write(root.join("content/docs/intro.md"), "# Intro\n").expect("intro should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                content_collection_count,
                melt_graph_written,
                ..
            } => {
                assert_eq!(content_collection_count, 1);
                assert!(melt_graph_written);
            }
            StaticBuildStatus::NoPages { .. } => panic!("static pages should be found"),
        }

        let manifest = fs::read_to_string(root.join("dist/_ax/content/manifest.json"))
            .expect("content manifest should exist");
        assert!(manifest.contains("\"name\": \"docs\""));
        assert!(manifest.contains("\"path\": \"content/docs/intro.md\""));
        assert!(manifest.contains("\"slug\": \"intro\""));
        let melt_graph = fs::read_to_string(root.join("dist/_ax/melt/graph.json"))
            .expect("Melt graph should exist");
        assert!(melt_graph.contains("\"content_entries\": 1"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn content_collection_path_must_stay_inside_app_root() {
        let root = make_temp_dir("content-path-safety");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "../outside"
"#,
        )
        .expect("config should write");

        let error = load_content_collection_configs(&root).expect_err("path should fail");
        assert!(error.to_string().contains("must stay inside app root"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn schema_inference_generates_ax_type_from_json_list() {
        let value = serde_json::json!([
            {
                "title": "Hello",
                "slug": "hello",
                "summary": null
            },
            {
                "title": "Second",
                "slug": "second"
            }
        ]);

        let schema = infer_schema_from_json("Post", &value).expect("schema should infer");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("type Post {"));
        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  slug: String"));
        assert!(ax.contains("  summary?: Unknown"));
        assert!(ax.contains("// root: List<Post>"));
    }

    #[test]
    fn schema_inference_marks_missing_list_fields_optional() {
        let value = serde_json::json!([
            {
                "title": "Hello",
                "views": 3
            },
            {
                "title": "Second",
                "summary": "Short"
            }
        ]);

        let schema = infer_schema_from_json("Post", &value).expect("schema should infer");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("  views?: Number"));
    }

    #[test]
    fn schema_pull_prefers_typed_envelope_schema() {
        let value = serde_json::json!({
            "type": "List<Post>",
            "schemaHash": "sha256:test",
            "schema": {
                "Post": {
                    "title": "String",
                    "slug": "String",
                    "summary": "Optional<String>"
                }
            },
            "data": [
                {
                    "title": "Hello",
                    "slug": "hello",
                    "summary": null
                }
            ]
        });

        let schema = schema_from_typed_envelope("Post", &value)
            .expect("schema envelope should parse")
            .expect("schema envelope should be detected");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("type Post {"));
        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  slug: String"));
        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("// root: List<Post>"));
    }

    #[test]
    fn schema_pull_accepts_object_field_descriptors() {
        let value = serde_json::json!({
            "schema": {
                "Post": {
                    "title": { "type": "String" },
                    "summary": { "type": "String", "optional": true }
                }
            }
        });

        let schema = schema_from_typed_envelope("Post", &value)
            .expect("schema envelope should parse")
            .expect("schema envelope should be detected");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("// root: Post"));
    }

    #[test]
    fn schema_pull_can_read_inline_json() {
        let source = read_schema_source(r#"[{"title":"Hello"}]"#).expect("inline JSON works");

        assert_eq!(source, r#"[{"title":"Hello"}]"#);
    }
}
