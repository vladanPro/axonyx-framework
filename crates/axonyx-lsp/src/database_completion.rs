use super::{
    completion_matches, normalized_line, path_starts_with, source_line_start,
    utf16_character_to_byte, ServerState,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

enum Context<'a> {
    Resource,
    Method {
        resource: &'a str,
        direct: bool,
        filters: bool,
        read_only: bool,
    },
    Column {
        resource: &'a str,
        used: Vec<&'a str>,
    },
}

const FILTERS: &[(&str, &str)] = &[
    ("where", "where({ column: value })"),
    ("whereNot", "whereNot({ column: value })"),
    ("whereIn", "whereIn({ column: values })"),
    ("whereNotIn", "whereNotIn({ column: values })"),
    ("whereNull", "whereNull(\"column\")"),
    ("whereNotNull", "whereNotNull(\"column\")"),
];
const READS: &[(&str, &str)] = &[
    ("all", "all()"),
    ("first", "first()"),
    ("one", "one()"),
    ("join", "join(db.table, { column: \"target_column\" })"),
    ("order", "order({ column: \"asc\" })"),
    ("limit", "limit(count)"),
    ("offset", "offset(count)"),
];

pub(super) fn complete(
    state: &ServerState,
    path: &Path,
    source: &str,
    line: usize,
    end: usize,
) -> Option<Vec<Value>> {
    let text = normalized_line(source, line)?;
    let line_byte = utf16_character_to_byte(text, end)?;
    let line_start = source_line_start(source, line)?;
    let before = &source[..line_start + line_byte];
    let prefix_len = text[..line_byte]
        .bytes()
        .rev()
        .take_while(|&b| ident_byte(b))
        .count();
    let prefix_start = line_byte - prefix_len;
    let prefix = &text[prefix_start..line_byte];
    let tail = text[line_byte..]
        .bytes()
        .take_while(|&b| ident_byte(b))
        .count();
    let replacement_end = end + tail;
    let (tokens, in_literal) = tokenize(before);
    if in_literal {
        return None;
    }
    let tokens = if prefix.is_empty() {
        &tokens[..]
    } else {
        &tokens[..tokens.len().checked_sub(1)?]
    };
    let context = tokens.iter().enumerate().rev().find_map(|(index, token)| {
        if *token != "db" || (index > 0 && tokens[index - 1] == ".") {
            return None;
        }
        parse_context(&tokens[index + 1..])
    })?;
    let mut items = BTreeMap::new();
    let mut add = |name: &str, kind: u8, detail: &str| {
        if !completion_matches(name, prefix) {
            return;
        }
        items.entry(name.to_owned()).or_insert_with(|| json!({
            "label": name, "kind": kind, "detail": detail,
            "textEdit": {
                "range": {
                    "start": { "line": line, "character": text[..prefix_start].encode_utf16().count() },
                    "end": { "line": line, "character": replacement_end }
                },
                "newText": name
            }
        }));
    };
    let schema = read_schema(state, path);
    let resources = schema
        .as_ref()
        .and_then(|schema| schema.get("tables"))
        .and_then(Value::as_array);
    let Some(resources) = resources else {
        return Some(Vec::new());
    };
    let get_resource = |name: &str| {
        resources
            .iter()
            .find(|resource| resource.get("name").and_then(Value::as_str) == Some(name))
    };
    match context {
        Context::Resource => {
            for resource in resources {
                if let Some(name) = resource
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| identifier(name))
                {
                    match resource_kind(resource) {
                        Some("table") => add(name, 5, "Database table"),
                        Some("view") => add(name, 5, "Database view"),
                        _ => {}
                    }
                }
            }
        }
        Context::Method {
            resource,
            direct,
            filters,
            read_only,
        } => {
            if let Some(resource) = get_resource(resource) {
                for (name, signature) in FILTERS.iter().chain(READS) {
                    add(name, 2, signature);
                }
                if resource_kind(resource) == Some("table") && !read_only {
                    if filters {
                        add("update", 2, "update({ column: value })");
                        add("delete", 2, "delete()");
                    } else if direct {
                        add("insert", 2, "insert({ column: value })");
                    }
                }
            }
        }
        Context::Column { resource, used } => {
            if let Some(columns) = get_resource(resource)
                .and_then(|r| r.get("columns"))
                .and_then(Value::as_array)
            {
                for column in columns {
                    if let Some(name) = column
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| identifier(name) && !used.contains(name))
                    {
                        let detail = column
                            .get("ax_type")
                            .and_then(Value::as_str)
                            .unwrap_or("Database column");
                        add(name, 5, detail);
                    }
                }
            }
        }
    }
    Some(items.into_values().collect())
}

fn resource_kind(resource: &Value) -> Option<&str> {
    match resource.get("kind") {
        None => Some("table"),
        Some(kind) => kind.as_str(),
    }
}

fn parse_context<'a>(tokens: &[&'a str]) -> Option<Context<'a>> {
    let [".", rest @ ..] = tokens else {
        return None;
    };
    if rest.is_empty() {
        return Some(Context::Resource);
    }
    let [resource, ".", method_tokens @ ..] = rest else {
        return None;
    };
    if !identifier(resource) {
        return None;
    }
    let mut filters = false;
    let mut read_only = false;
    let mut methods = method_tokens;
    let direct = methods.is_empty();
    while !methods.is_empty() {
        let [method, "(", args @ ..] = methods else {
            return None;
        };
        let object_filter = FILTERS[..4].iter().any(|(name, _)| name == method);
        let any_filter = FILTERS.iter().any(|(name, _)| name == method);
        let read_clause = matches!(*method, "join" | "order" | "limit" | "offset");
        if !any_filter && !read_clause {
            return None;
        }
        if let Some(close) = closing_token(args, ")") {
            let [".", rest @ ..] = &args[close + 1..] else {
                return None;
            };
            filters |= any_filter && close != 0 && args[..close] != ["{", "}"];
            read_only |= read_clause;
            methods = rest;
        } else if object_filter || *method == "order" {
            let ["{", keys @ ..] = args else {
                return None;
            };
            return Some(Context::Column {
                resource,
                used: column_keys(keys)?,
            });
        } else {
            return None;
        }
    }
    Some(Context::Method {
        resource,
        direct,
        filters,
        read_only,
    })
}

fn column_keys<'a>(mut tokens: &[&'a str]) -> Option<Vec<&'a str>> {
    let mut used = Vec::new();
    while !tokens.is_empty() {
        let [key, rest @ ..] = tokens else {
            return None;
        };
        if !identifier(key) && !key.starts_with(['"', '\'']) {
            return None;
        }
        let key = key.trim_matches(['"', '\'']);
        used.push(key);
        match rest {
            [",", next @ ..] => tokens = next,
            [":", value @ ..] => {
                let comma = closing_token(value, ",")?;
                if comma == 0 {
                    return None;
                }
                tokens = &value[comma + 1..];
            }
            _ => return None,
        }
    }
    Some(used)
}

// Find the first delimiter at this nesting level, ignoring literals and nested values.
fn closing_token(tokens: &[&str], expected: &str) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, &token) in tokens.iter().enumerate() {
        if stack.is_empty() && token == expected {
            return Some(index);
        }
        match token {
            "(" => stack.push(")"),
            "{" => stack.push("}"),
            "[" => stack.push("]"),
            ")" | "}" | "]" if stack.pop() != Some(token) => return None,
            _ => {}
        }
    }
    None
}

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(ident_byte)
}

fn tokenize(source: &str) -> (Vec<&str>, bool) {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '/' && chars.peek().is_some_and(|&(_, next)| next == '/') {
            if !chars.any(|(_, next)| next == '\n') {
                return (tokens, true);
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            loop {
                match chars.next() {
                    Some((_, '\\')) => {
                        chars.next();
                    }
                    Some((_, end)) if end == ch => break,
                    Some(_) => {}
                    None => return (tokens, true),
                }
            }
        } else if ch.is_ascii() && ident_byte(ch as u8) {
            while chars
                .peek()
                .is_some_and(|&(_, next)| next.is_ascii() && ident_byte(next as u8))
            {
                chars.next();
            }
        }
        let end = chars.peek().map_or(source.len(), |&(index, _)| index);
        tokens.push(&source[start..end]);
    }
    (tokens, false)
}

fn read_schema(state: &ServerState, path: &Path) -> Option<Value> {
    let workspace = state.workspace_root.as_deref()?;
    let parent = path
        .parent()
        .filter(|parent| path_starts_with(parent, workspace))?;
    let root = parent
        .ancestors()
        .take_while(|ancestor| path_starts_with(ancestor, workspace))
        .find(|ancestor| {
            ancestor.join("Axonyx.toml").is_file() || ancestor.join("Cargo.toml").is_file()
        })
        .unwrap_or(workspace);
    // Read on demand so schema pulls are reflected without an editor restart.
    let source = fs::read_to_string(root.join(".axonyx/db/schema.json")).ok()?;
    serde_json::from_str(&source).ok()
}
