use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use axonyx_runtime::preview_ax_route;
use clap::{Parser, Subcommand, ValueEnum};

const DOCS_LAYOUT_AX: &str = include_str!("../templates/docs/app/docs/layout.ax.tpl");
const DOCS_HOME_AX: &str = include_str!("../templates/docs/app/docs/page.ax.tpl");
const DOCS_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/docs/getting-started/page.ax.tpl");
const DOCS_REFERENCE_AX: &str = include_str!("../templates/docs/app/docs/reference/page.ax.tpl");
const DOCS_EXAMPLES_AX: &str = include_str!("../templates/docs/app/docs/examples/page.ax.tpl");

#[derive(Debug, Parser)]
#[command(name = "ax")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Add(AddArgs),
    Dev(DevArgs),
    Run(RunArgs),
}

#[derive(Debug, Parser)]
struct AddArgs {
    #[arg(value_enum)]
    module: ModuleKind,
}

#[derive(Debug, Parser)]
struct DevArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Debug, Parser)]
struct RunArgs {
    #[command(subcommand)]
    command: RunCommands,
}

#[derive(Debug, Subcommand)]
enum RunCommands {
    Dev(DevArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleKind {
    Docs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRoute {
    request_path: String,
    page_path: PathBuf,
    layout_paths: Vec<PathBuf>,
}

pub fn main_entry() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add(args) => add_module(args.module),
        Commands::Dev(args) => run_dev_server(args),
        Commands::Run(args) => run_command(args),
    }
}

fn run_command(args: RunArgs) -> Result<()> {
    match args.command {
        RunCommands::Dev(args) => run_dev_server(args),
    }
}

fn add_module(module: ModuleKind) -> Result<()> {
    let root = std::env::current_dir().context("unable to resolve current directory")?;
    let axonyx_toml = root.join("Axonyx.toml");

    if !axonyx_toml.exists() {
        bail!(
            "Axonyx.toml was not found in '{}'; run this command from an Axonyx app root",
            root.display()
        );
    }

    match module {
        ModuleKind::Docs => {
            scaffold_docs_module(&root)?;
            enable_module(&axonyx_toml, "docs")?;
            println!("Added docs module.");
        }
    }

    Ok(())
}

fn run_dev_server(args: DevArgs) -> Result<()> {
    let root = std::env::current_dir().context("unable to resolve current directory")?;
    let axonyx_toml = root.join("Axonyx.toml");

    if !axonyx_toml.exists() {
        bail!(
            "Axonyx.toml was not found in '{}'; run this command from an Axonyx app root",
            root.display()
        );
    }

    let bind = format!("{}:{}", args.host, args.port);
    let listener =
        TcpListener::bind(&bind).with_context(|| format!("failed to bind dev server at {bind}"))?;
    let shared_root = Arc::new(root);

    println!("Axonyx dev server listening at http://{bind}");
    println!("Routes come from app/**/page.ax with nested app/**/layout.ax composition.");
    println!("Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &shared_root) {
                    eprintln!("dev server error: {error:#}");
                }
            }
            Err(error) => eprintln!("dev server connection error: {error}"),
        }
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
    let source = fs::read_to_string(axonyx_toml)
        .with_context(|| format!("failed to read '{}'", axonyx_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", axonyx_toml.display()))?;

    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Axonyx.toml root must be a TOML table"))?;

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

    let rendered = toml::to_string_pretty(&value).context("failed to render Axonyx.toml")?;
    fs::write(axonyx_toml, rendered)
        .with_context(|| format!("failed to write '{}'", axonyx_toml.display()))?;
    Ok(())
}

fn handle_connection(mut stream: TcpStream, root: &Path) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set read timeout")?;

    let mut buffer = [0_u8; 4096];
    let read = stream
        .read(&mut buffer)
        .context("failed to read request from dev client")?;
    if read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(request_line) = request.lines().next() else {
        return Ok(());
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method != "GET" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        )?;
        return Ok(());
    }

    if target == "/favicon.ico" {
        write_response(
            &mut stream,
            "204 No Content",
            "text/plain; charset=utf-8",
            b"",
        )?;
        return Ok(());
    }

    if target.starts_with("/__axonyx/version") {
        let request_path = extract_version_path(target).unwrap_or_else(|| "/".to_string());
        let Some(route) = resolve_route(root, &request_path)? else {
            write_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"route not found",
            )?;
            return Ok(());
        };

        let version = route_version(&route)?;
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            version.as_bytes(),
        )?;
        return Ok(());
    }

    let Some(route) = resolve_route(root, target)? else {
        let html = format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Axonyx 404</title></head><body><h1>Route not found</h1><p>No <code>page.ax</code> matched <code>{}</code>.</p></body></html>",
            html_escape(target)
        );
        write_response(
            &mut stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            html.as_bytes(),
        )?;
        return Ok(());
    };

    let html = render_route_html(&route)?;
    let html = inject_dev_client(&html, &route.request_path);
    write_response(
        &mut stream,
        "200 OK",
        "text/html; charset=utf-8",
        html.as_bytes(),
    )?;
    Ok(())
}

fn resolve_route(root: &Path, request_path: &str) -> Result<Option<ResolvedRoute>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    let app_root = root.join("app");
    let page_path = segments
        .iter()
        .fold(app_root.clone(), |current, segment| current.join(segment))
        .join("page.ax");

    if !page_path.exists() {
        return Ok(None);
    }

    let mut layout_paths = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layout_paths.push(root_layout);
    }

    let mut current = app_root;
    for segment in &segments {
        current = current.join(segment);
        let layout_path = current.join("layout.ax");
        if layout_path.exists() {
            layout_paths.push(layout_path);
        }
    }

    Ok(Some(ResolvedRoute {
        request_path: normalized,
        page_path,
        layout_paths,
    }))
}

fn render_route_html(route: &ResolvedRoute) -> Result<String> {
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

    preview_ax_route(&layout_refs, &page_source).with_context(|| {
        format!(
            "failed to render route '{}' from '{}'",
            route.request_path,
            route.page_path.display()
        )
    })
}

fn route_version(route: &ResolvedRoute) -> Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    route.request_path.hash(&mut hasher);

    hash_file(&route.page_path, &mut hasher)?;
    for path in &route.layout_paths {
        hash_file(path, &mut hasher)?;
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn hash_file(path: &Path, hasher: &mut impl Hasher) -> Result<()> {
    path.to_string_lossy().hash(hasher);
    let contents = fs::read(path)
        .with_context(|| format!("failed to read '{}' for hashing", path.display()))?;
    contents.hash(hasher);
    Ok(())
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

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );

    stream
        .write_all(header.as_bytes())
        .context("failed to write response headers")?;
    stream
        .write_all(body)
        .context("failed to write response body")?;
    stream.flush().context("failed to flush response")?;
    Ok(())
}

fn extract_version_path(target: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if key == "path" {
            return Some(url_decode(value));
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn resolve_route_collects_nested_layouts() {
        let root = make_temp_dir("route");
        fs::create_dir_all(root.join("app/docs")).expect("app/docs should exist");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n  Slot\n")
            .expect("root layout should write");
        fs::write(root.join("app/docs/layout.ax"), "page DocsLayout\n  Slot\n")
            .expect("nested layout should write");
        fs::write(
            root.join("app/docs/page.ax"),
            "page DocsHome\n  Copy -> \"Docs\"\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/docs")
            .expect("route resolution should work")
            .expect("route should exist");

        assert_eq!(route.request_path, "/docs");
        assert_eq!(route.layout_paths.len(), 2);
        assert!(route
            .page_path
            .ends_with(Path::new("app").join("docs").join("page.ax")));

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
}
