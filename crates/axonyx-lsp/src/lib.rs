use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use axonyx_core::ax_formatter_prelude::format_ax_source;
use axonyx_core::ax_language_service_prelude::diagnose_ax_source;
use serde_json::{json, Value};

const JSON_RPC_VERSION: &str = "2.0";

#[derive(Default)]
struct ServerState {
    documents: HashMap<String, OpenDocument>,
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
                            "documentFormattingProvider": true
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
                    publish_diagnostics(writer, uri, text, version)?;
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
                    publish_diagnostics(writer, uri, text, version)?;
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
    writer: &mut W,
    uri: &str,
    source: &str,
    version: Option<i64>,
) -> io::Result<()> {
    let diagnostics = diagnose_ax_source(uri, source)
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
    use std::io::{BufReader, Cursor};

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
}
