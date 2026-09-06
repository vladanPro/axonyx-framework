use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use axonyx_core::ax_formatter_prelude::format_ax_source;
use axonyx_core::ax_language_service_prelude::{
    ax_source_imports, classify_ax_source, diagnose_ax_source, diagnose_ax_workspace_imports,
    resolve_ax_import_path, AxSourceKind,
};
use serde_json::{json, Value};

const JSON_RPC_VERSION: &str = "2.0";

#[derive(Default)]
struct ServerState {
    documents: HashMap<String, OpenDocument>,
    workspace_root: Option<PathBuf>,
    package_roots: BTreeMap<String, PathBuf>,
    shutdown_requested: bool,
}

struct OpenDocument {
    text: String,
    version: Option<i64>,
}

pub fn run_server<R, W>(mut reader: R, mut writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let mut state = ServerState::default();

    while let Some(message) = read_message(&mut reader)? {
        if handle_message(&mut state, &mut writer, message)? {
            break;
        }
    }

    Ok(())
}

fn handle_message<W: Write>(
    state: &mut ServerState,
    writer: &mut W,
    message: Value,
) -> io::Result<bool> {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").cloned();

    if state.shutdown_requested && method != Some("exit") {
        if let Some(id) = id {
            write_error(writer, id, -32600, "server has shut down".to_string())?;
        }
        return Ok(false);
    }

    match method {
        Some("initialize") => {
            state.workspace_root = message
                .pointer("/params/rootUri")
                .and_then(Value::as_str)
                .and_then(file_uri_to_path)
                .or_else(|| {
                    message
                        .pointer("/params/rootPath")
                        .and_then(Value::as_str)
                        .map(PathBuf::from)
                });
            state.package_roots = state
                .workspace_root
                .as_deref()
                .map(discover_axonyx_package_roots)
                .unwrap_or_default();
            if let Some(id) = id {
                write_response(
                    writer,
                    id,
                    json!({
                        "capabilities": {
                            "positionEncoding": "utf-16",
                            "textDocumentSync": {
                                "openClose": true,
                                "change": 1
                            },
                            "documentFormattingProvider": true,
                            "definitionProvider": true
                        },
                        "serverInfo": {
                            "name": "axonyx-lsp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                )?;
            }
        }
        Some("initialized") => {}
        Some("shutdown") => {
            state.shutdown_requested = true;
            if let Some(id) = id {
                write_response(writer, id, Value::Null)?;
            }
        }
        Some("exit") => return Ok(true),
        Some("textDocument/didOpen") => {
            if let Some(document) = message
                .pointer("/params/textDocument")
                .and_then(Value::as_object)
            {
                if let (Some(uri), Some(text)) = (
                    document.get("uri").and_then(Value::as_str),
                    document.get("text").and_then(Value::as_str),
                ) {
                    let version = document.get("version").and_then(Value::as_i64);
                    state.documents.insert(
                        uri.to_string(),
                        OpenDocument {
                            text: text.to_string(),
                            version,
                        },
                    );
                    publish_diagnostics(state, writer, uri, text, version)?;
                }
            }
        }
        Some("textDocument/didChange") => {
            let uri = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str);
            let text = message
                .pointer("/params/contentChanges")
                .and_then(Value::as_array)
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str);
            if let (Some(uri), Some(text)) = (uri, text) {
                let version = message
                    .pointer("/params/textDocument/version")
                    .and_then(Value::as_i64);
                let is_newer = state.documents.get(uri).is_none_or(|document| {
                    !matches!((document.version, version), (Some(current), Some(next)) if next <= current)
                });
                if is_newer {
                    state.documents.insert(
                        uri.to_string(),
                        OpenDocument {
                            text: text.to_string(),
                            version,
                        },
                    );
                    publish_diagnostics(state, writer, uri, text, version)?;
                }
            }
        }
        Some("textDocument/didClose") => {
            if let Some(uri) = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
            {
                state.documents.remove(uri);
                write_notification(
                    writer,
                    "textDocument/publishDiagnostics",
                    json!({ "uri": uri, "diagnostics": [] }),
                )?;
            }
        }
        Some("textDocument/formatting") => {
            if let Some(id) = id {
                let edits = message
                    .pointer("/params/textDocument/uri")
                    .and_then(Value::as_str)
                    .and_then(|uri| state.documents.get(uri))
                    .map(|document| formatting_edits(&document.text))
                    .unwrap_or_default();
                write_response(writer, id, Value::Array(edits))?;
            }
        }
        Some("textDocument/definition") => {
            if let Some(id) = id {
                write_response(
                    writer,
                    id,
                    import_definition(state, &message).unwrap_or(Value::Null),
                )?;
            }
        }
        Some(method) if id.is_some() => {
            write_error(
                writer,
                id.expect("request id checked"),
                -32601,
                format!("method not found: {method}"),
            )?;
        }
        Some(_) | None => {}
    }

    Ok(false)
}

fn publish_diagnostics<W: Write>(
    state: &ServerState,
    writer: &mut W,
    uri: &str,
    source: &str,
    version: Option<i64>,
) -> io::Result<()> {
    let mut source_diagnostics = diagnose_ax_source(uri, source);
    if source_diagnostics.is_empty() {
        if let (Some(root), Some(path)) = (&state.workspace_root, file_uri_to_path(uri)) {
            source_diagnostics.extend(diagnose_ax_workspace_imports(
                root,
                &path,
                source,
                &state.package_roots,
            ));
        }
    }

    let diagnostics = source_diagnostics
        .into_iter()
        .map(|diagnostic| {
            let line = diagnostic.line.saturating_sub(1);
            let character = diagnostic.column.saturating_sub(1);
            let line_length = utf16_line_length(source, line).unwrap_or(0);
            let end_character = if character < line_length {
                character + 1
            } else {
                character
            };
            json!({
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": end_character }
                },
                "severity": 1,
                "code": diagnostic.code,
                "source": "axonyx",
                "message": diagnostic.message
            })
        })
        .collect::<Vec<_>>();

    write_notification(
        writer,
        "textDocument/publishDiagnostics",
        json!({ "uri": uri, "version": version, "diagnostics": diagnostics }),
    )
}

fn import_definition(state: &ServerState, message: &Value) -> Option<Value> {
    let uri = message.pointer("/params/textDocument/uri")?.as_str()?;
    let line = message.pointer("/params/position/line")?.as_u64()? as usize;
    let character = message.pointer("/params/position/character")?.as_u64()? as usize;
    let document = state.documents.get(uri)?;
    let importing_path = file_uri_to_path(uri)?;
    let root = state.workspace_root.as_ref()?;
    let source_line = normalized_line(&document.text, line)?;
    if character > source_line.encode_utf16().count() {
        return None;
    }

    let import = ax_source_imports(&importing_path.to_string_lossy(), &document.text)
        .into_iter()
        .find(|import| import.line.saturating_sub(1) == line)?;
    if !cursor_is_on_import_source(source_line, &import.source, character) {
        return None;
    }
    let kind = classify_ax_source(&importing_path.to_string_lossy(), &document.text);
    if kind == AxSourceKind::Page
        && (import.source.starts_with("./") || import.source.starts_with("../"))
    {
        return None;
    }

    let target = resolve_ax_import_path(
        root,
        &importing_path,
        kind,
        &import.source,
        &state.package_roots,
    )?;
    if !target.is_file() {
        return None;
    }

    Some(json!({
        "uri": path_to_file_uri(&target),
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        }
    }))
}

fn cursor_is_on_import_source(line: &str, import_source: &str, character: usize) -> bool {
    let Some(source_start) = line.find(import_source) else {
        return false;
    };
    let source_end = source_start + import_source.len();
    let quoted_start = source_start
        .checked_sub(1)
        .filter(|index| matches!(line.as_bytes().get(*index), Some(b'\"' | b'\'')))
        .unwrap_or(source_start);
    let quoted_end = if matches!(line.as_bytes().get(source_end), Some(b'\"' | b'\'')) {
        source_end + 1
    } else {
        source_end
    };
    let start = line[..quoted_start].encode_utf16().count();
    let end = line[..quoted_end].encode_utf16().count();
    (start..=end).contains(&character)
}

fn normalized_line(source: &str, target_line: usize) -> Option<&str> {
    source
        .split('\n')
        .nth(target_line)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

fn discover_axonyx_package_roots(root: &Path) -> BTreeMap<String, PathBuf> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.is_file() {
        return BTreeMap::new();
    }

    let Ok(output) = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(root)
        .output()
    else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    let Ok(metadata) = serde_json::from_slice::<Value>(&output.stdout) else {
        return BTreeMap::new();
    };
    metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(package_ax_root)
        .collect()
}

fn package_ax_root(package: &Value) -> Option<(String, PathBuf)> {
    let manifest = PathBuf::from(package.get("manifest_path")?.as_str()?);
    let package_root = manifest.parent()?;
    let package_config = fs::read_to_string(package_root.join("Axonyx.package.toml")).ok()?;
    let config = package_config.parse::<toml::Value>().ok()?;
    let namespace = config
        .get("package")?
        .get("namespace")?
        .as_str()?
        .to_string();
    let relative = config
        .get("exports")
        .and_then(|exports| exports.get("ax_root"))
        .and_then(toml::Value::as_str)
        .unwrap_or("src/ax");
    Some((namespace, package_root.join(relative)))
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let decoded = percent_decode(encoded)?;

    #[cfg(windows)]
    let decoded = if decoded.starts_with('/') && decoded.as_bytes().get(2).copied() == Some(b':') {
        &decoded[1..]
    } else {
        decoded.as_str()
    };

    Some(PathBuf::from(decoded))
}

fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    format!("{prefix}{}", percent_encode_path(&normalized))
}

fn percent_encode_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~' | b'/' | b':')
        {
            encoded.push(char::from(*byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn utf16_line_length(source: &str, target_line: usize) -> Option<usize> {
    source
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .nth(target_line)
        .map(|line| line.encode_utf16().count())
}

fn formatting_edits(source: &str) -> Vec<Value> {
    let formatted = format_ax_source(source);
    if formatted == source {
        return Vec::new();
    }

    let (line, character) = document_end_position(source);
    vec![json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": line, "character": character }
        },
        "newText": formatted
    })]
}

fn document_end_position(source: &str) -> (usize, usize) {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized.split('\n').collect::<Vec<_>>();
    if normalized.ends_with('\n') {
        lines.pop();
        (lines.len(), 0)
    } else {
        let line = lines.len().saturating_sub(1);
        let character = lines
            .last()
            .map(|value| value.encode_utf16().count())
            .unwrap_or(0);
        (line, character)
    }
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut saw_header = false;

    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header)?;
        if read == 0 {
            if saw_header {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "LSP headers ended before the blank separator",
                ));
            }
            return Ok(None);
        }
        saw_header = true;

        let header = header.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid LSP Content-Length")
                })?);
            }
        }
    }

    let content_length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing LSP Content-Length"))?;
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_response<W: Write>(writer: &mut W, id: Value, result: Value) -> io::Result<()> {
    write_message(
        writer,
        &json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "result": result }),
    )
}

fn write_error<W: Write>(writer: &mut W, id: Value, code: i64, message: String) -> io::Result<()> {
    write_message(
        writer,
        &json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": { "code": code, "message": message }
        }),
    )
}

fn write_notification<W: Write>(writer: &mut W, method: &str, params: Value) -> io::Result<()> {
    write_message(
        writer,
        &json!({ "jsonrpc": JSON_RPC_VERSION, "method": method, "params": params }),
    )
}

fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufReader, Cursor};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn frame(message: Value) -> Vec<u8> {
        let body = serde_json::to_vec(&message).expect("message should serialize");
        let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed
    }

    fn run(messages: Vec<Value>) -> Vec<Value> {
        let input = messages.into_iter().flat_map(frame).collect::<Vec<u8>>();
        let mut output = Vec::new();
        run_server(BufReader::new(Cursor::new(input)), &mut output)
            .expect("server should complete");

        let mut reader = BufReader::new(Cursor::new(output));
        let mut messages = Vec::new();
        while let Some(message) = read_message(&mut reader).expect("response should be framed") {
            messages.push(message);
        }
        messages
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be available")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("axonyx-lsp-{name}-{nonce}"));
        fs::create_dir_all(root.join("app/components")).expect("workspace should be created");
        root
    }

    fn file_uri(path: &Path) -> String {
        let path = path
            .to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "%20");
        if path.starts_with('/') {
            format!("file://{path}")
        } else {
            format!("file:///{path}")
        }
    }

    #[test]
    fn initializes_and_shuts_down() {
        let messages = run(vec![
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["id"], 1);
        assert_eq!(
            messages[0]["result"]["capabilities"]["positionEncoding"],
            "utf-16"
        );
        assert_eq!(
            messages[0]["result"]["capabilities"]["documentFormattingProvider"],
            true
        );
        assert_eq!(
            messages[0]["result"]["capabilities"]["definitionProvider"],
            true
        );
        assert_eq!(
            messages[1],
            json!({ "jsonrpc": "2.0", "id": 2, "result": null })
        );
    }

    #[test]
    fn publishes_and_clears_document_diagnostics() {
        let uri = "file:///workspace/app/page.asx";
        let messages = run(vec![
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": uri, "version": 1, "text": "page Broken() {\n  return ASX {\n    <Card>\n    </Grid>\n  }\n}\n" } }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "text": "page Fixed() {\n  return ASX {\n    <Card />\n  }\n}\n" }]
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": { "textDocument": { "uri": uri } }
            }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["params"]["version"], 1);
        assert_eq!(
            messages[0]["params"]["diagnostics"][0]["range"]["start"]["line"],
            3
        );
        assert_eq!(
            messages[0]["params"]["diagnostics"][0]["code"],
            "axonyx-parse"
        );
        assert_eq!(messages[1]["params"]["version"], 2);
        assert_eq!(messages[1]["params"]["diagnostics"], json!([]));
        assert_eq!(messages[2]["params"]["diagnostics"], json!([]));
    }

    #[test]
    fn formats_the_open_document_with_utf16_end_position() {
        let uri = "file:///workspace/app/page.asx";
        let source = "page Home() {\nreturn ASX {\n<Copy>Hello 🛠</Copy>\n}\n}";
        let messages = run(vec![
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": uri, "text": source } }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": "format-1",
                "method": "textDocument/formatting",
                "params": { "textDocument": { "uri": uri }, "options": { "tabSize": 2, "insertSpaces": true } }
            }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(messages.len(), 2);
        let edit = &messages[1]["result"][0];
        assert_eq!(edit["range"]["end"], json!({ "line": 4, "character": 1 }));
        assert_eq!(
            edit["newText"],
            "page Home() {\n  return ASX {\n    <Copy>Hello 🛠</Copy>\n  }\n}\n"
        );
    }

    #[test]
    fn rejects_unknown_requests_with_json_rpc_error() {
        let messages = run(vec![
            json!({ "jsonrpc": "2.0", "id": 7, "method": "axonyx/unknown" }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(messages[0]["error"]["code"], -32601);
    }

    #[test]
    fn publishes_workspace_import_diagnostics_and_clears_them_after_change() {
        let root = temp_workspace("imports");
        let page = root.join("app/page.asx");
        fs::write(
            root.join("app/components/Card.asx"),
            "component Card { render ASX { <article /> } }",
        )
        .expect("component should be written");
        let root_uri = file_uri(&root);
        let page_uri = file_uri(&page);
        let messages = run(vec![
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "rootUri": root_uri } }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": {
                    "uri": page_uri,
                    "version": 1,
                    "text": "import { Missing } from \"@/components/Missing\"\n\npage Home() { return ASX { <Missing /> } }"
                } }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": page_uri, "version": 2 },
                    "contentChanges": [{
                        "text": "import { Card } from \"@/components/Card\"\n\npage Home() { return ASX { <Card /> } }"
                    }]
                }
            }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(
            messages[1]["params"]["diagnostics"][0]["code"],
            "axonyx-import"
        );
        assert_eq!(
            messages[1]["params"]["diagnostics"][0]["range"]["start"]["line"],
            0
        );
        assert_eq!(messages[2]["params"]["diagnostics"], json!([]));

        fs::remove_dir_all(root).expect("workspace should be removed");
    }

    #[test]
    fn reads_axonyx_package_namespace_and_ax_root() {
        let root = temp_workspace("package-root");
        let package_root = root.join("ui-package");
        fs::create_dir_all(package_root.join("components"))
            .expect("package files should be created");
        fs::write(
            package_root.join("Axonyx.package.toml"),
            "[package]\nnamespace = \"@axonyx/ui\"\n\n[exports]\nax_root = \"components\"\n",
        )
        .expect("package config should be written");
        let manifest = package_root.join("Cargo.toml");
        let package = json!({ "manifest_path": manifest.to_string_lossy() });

        assert_eq!(
            package_ax_root(&package),
            Some(("@axonyx/ui".to_string(), package_root.join("components")))
        );

        fs::remove_dir_all(root).expect("workspace should be removed");
    }

    #[test]
    fn discovers_path_dependency_package_roots_from_cargo_metadata() {
        let root = temp_workspace("package-metadata");
        let package_root = root.join("packages/axonyx-ui");
        fs::create_dir_all(package_root.join("src/foundry"))
            .expect("package files should be created");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"lsp-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naxonyx-ui = { path = \"packages/axonyx-ui\" }\n",
        )
        .expect("app manifest should be written");
        fs::create_dir_all(root.join("src")).expect("app source directory should be created");
        fs::write(root.join("src/main.rs"), "fn main() {}").expect("app source should be written");
        fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname = \"axonyx-ui\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
        )
        .expect("package manifest should be written");
        fs::write(package_root.join("src/lib.rs"), "").expect("package source should be written");
        fs::write(
            package_root.join("Axonyx.package.toml"),
            "[package]\nnamespace = \"@axonyx/ui\"\n\n[exports]\nax_root = \"src\"\n",
        )
        .expect("package config should be written");

        let roots = discover_axonyx_package_roots(&root);

        assert_eq!(roots.get("@axonyx/ui"), Some(&package_root.join("src")));
        fs::remove_dir_all(root).expect("workspace should be removed");
    }

    #[test]
    fn returns_import_target_location_for_app_alias() {
        let root = temp_workspace("definition");
        let page = root.join("app/page.asx");
        let component = root.join("app/components/Card.asx");
        fs::write(&component, "component Card { render ASX { <article /> } }")
            .expect("component should be written");
        let root_uri = file_uri(&root);
        let page_uri = file_uri(&page);
        let messages = run(vec![
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "rootUri": root_uri } }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": {
                    "uri": page_uri,
                    "version": 1,
                    "text": "import { Card } from \"@/components/Card\"\n\npage Home() { return ASX { <Card /> } }"
                } }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": page_uri },
                    "position": { "line": 0, "character": 28 }
                }
            }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(messages[2]["id"], 2);
        assert_eq!(messages[2]["result"]["uri"], path_to_file_uri(&component));
        assert_eq!(
            messages[2]["result"]["range"]["start"],
            json!({ "line": 0, "character": 0 })
        );
        fs::remove_dir_all(root).expect("workspace should be removed");
    }

    #[test]
    fn returns_null_definition_for_missing_import() {
        let root = temp_workspace("missing-definition");
        let page = root.join("app/page.asx");
        let root_uri = file_uri(&root);
        let page_uri = file_uri(&page);
        let messages = run(vec![
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "rootUri": root_uri } }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": {
                    "uri": page_uri,
                    "version": 1,
                    "text": "import { Missing } from \"@/components/Missing\"\n\npage Home() { return ASX { <Missing /> } }"
                } }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": page_uri },
                    "position": { "line": 0, "character": 30 }
                }
            }),
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ]);

        assert_eq!(
            messages[2],
            json!({ "jsonrpc": "2.0", "id": 2, "result": null })
        );
        fs::remove_dir_all(root).expect("workspace should be removed");
    }

    #[test]
    fn file_uri_round_trip_preserves_spaces_and_unicode() {
        let path = std::env::temp_dir().join("Axonyx UI").join("Čelik.asx");
        let uri = path_to_file_uri(&path);

        assert_eq!(file_uri_to_path(&uri), Some(path));
        assert!(uri.contains("%20"));
    }

    #[test]
    fn import_definition_cursor_is_limited_to_the_source_literal() {
        let line = "import { Card } from \"@/components/Card\"";

        assert!(!cursor_is_on_import_source(line, "@/components/Card", 2));
        assert!(cursor_is_on_import_source(line, "@/components/Card", 28));
    }
}
