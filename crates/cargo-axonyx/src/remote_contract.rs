use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_REMOTE_CONTRACT_BYTES: u64 = 2 * 1024 * 1024;
const REMOTE_CONTRACT_TIMEOUT_SECONDS: u64 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct RemoteApiContractReport {
    pub(super) source: String,
    pub(super) openapi: String,
    pub(super) title: String,
    pub(super) version: String,
    pub(super) contract_hash: String,
    pub(super) document_hash: String,
    pub(super) declared_contract_hash: Option<String>,
    pub(super) response_validation: Option<String>,
    pub(super) paths: usize,
    pub(super) operations: usize,
    pub(super) schemas: usize,
}

pub(super) fn read_api_contract_source(
    source: &str,
    allow_http: bool,
    base: Option<&Path>,
) -> Result<serde_json::Value> {
    let body = if source.starts_with("https://") || source.starts_with("http://") {
        read_remote_contract_text(source, allow_http)?
    } else {
        let source_path = Path::new(source);
        let path = if source_path.is_absolute() {
            source_path.to_path_buf()
        } else if let Some(base) = base {
            base.join(source_path)
        } else {
            source_path.to_path_buf()
        };
        fs::read_to_string(&path)
            .with_context(|| format!("failed to read API contract '{}'", path.display()))?
    };

    serde_json::from_str(&body).with_context(|| {
        format!(
            "failed to parse API contract '{}' as JSON",
            contract_source_label(source)
        )
    })
}

fn read_remote_contract_text(source: &str, allow_http: bool) -> Result<String> {
    validate_remote_contract_url(source, allow_http)?;
    let label = contract_source_label(source);
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(REMOTE_CONTRACT_TIMEOUT_SECONDS)))
        .max_redirects(0)
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(source)
        .header(
            "Accept",
            "application/json, application/vnd.oai.openapi+json",
        )
        .header(
            "User-Agent",
            concat!("cargo-axonyx/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .with_context(|| format!("failed to fetch API contract '{label}'"))?;

    response
        .body_mut()
        .with_config()
        .limit(MAX_REMOTE_CONTRACT_BYTES)
        .read_to_string()
        .with_context(|| {
            format!(
                "failed to read API contract '{label}' (maximum {} bytes)",
                MAX_REMOTE_CONTRACT_BYTES
            )
        })
}

pub(super) fn validate_remote_contract_url(source: &str, allow_http: bool) -> Result<()> {
    let (secure, authority) = remote_contract_url_authority(source)?;
    if authority.contains('@') {
        bail!("API contract URLs must not contain embedded credentials");
    }
    if secure {
        return Ok(());
    }
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback());
    if loopback || allow_http {
        Ok(())
    } else {
        bail!(
            "plain HTTP is allowed only for loopback contract endpoints; use HTTPS or pass --allow-http explicitly"
        )
    }
}

fn remote_contract_url_authority(source: &str) -> Result<(bool, &str)> {
    let (secure, rest) = if let Some(rest) = source.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = source.strip_prefix("http://") {
        (false, rest)
    } else {
        bail!("API contract source must use https://, loopback http://, or a local file");
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        bail!("API contract URL host is empty");
    }
    Ok((secure, authority))
}

pub(super) fn inspect_remote_api_contract(
    source: &str,
    document: &serde_json::Value,
) -> Result<RemoteApiContractReport> {
    let object = document
        .as_object()
        .with_context(|| "API contract root must be a JSON object")?;
    let openapi = object
        .get("openapi")
        .and_then(serde_json::Value::as_str)
        .with_context(|| "API contract is missing string field `openapi`")?;
    if !openapi.starts_with("3.") {
        bail!("unsupported OpenAPI version '{openapi}'; expected OpenAPI 3.x");
    }
    let info = object
        .get("info")
        .and_then(serde_json::Value::as_object)
        .with_context(|| "API contract is missing object field `info`")?;
    let title = info
        .get("title")
        .and_then(serde_json::Value::as_str)
        .with_context(|| "API contract info is missing string field `title`")?;
    let version = info
        .get("version")
        .and_then(serde_json::Value::as_str)
        .with_context(|| "API contract info is missing string field `version`")?;
    let paths = object
        .get("paths")
        .and_then(serde_json::Value::as_object)
        .with_context(|| "API contract is missing object field `paths`")?;
    let operations = paths
        .values()
        .filter_map(serde_json::Value::as_object)
        .map(|item| {
            item.keys()
                .filter(|key| is_openapi_operation_key(key))
                .count()
        })
        .sum();
    let schemas = object
        .get("components")
        .and_then(|value| value.get("schemas"))
        .and_then(serde_json::Value::as_object)
        .map_or(0, serde_json::Map::len);
    let declared_contract_hash = object
        .get("x-axonyx-contract-hash")
        .map(|value| {
            value
                .as_str()
                .with_context(|| "x-axonyx-contract-hash must be a string")
                .and_then(validate_sha256_contract_hash)
        })
        .transpose()?;
    let document_hash = canonical_json_hash(document)?;
    let contract_hash = declared_contract_hash
        .clone()
        .unwrap_or_else(|| document_hash.clone());

    Ok(RemoteApiContractReport {
        source: contract_source_label(source),
        openapi: openapi.to_string(),
        title: title.to_string(),
        version: version.to_string(),
        contract_hash,
        document_hash,
        declared_contract_hash,
        response_validation: object
            .get("x-axonyx-response-validation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        paths: paths.len(),
        operations,
        schemas,
    })
}

fn is_openapi_operation_key(key: &str) -> bool {
    matches!(
        key,
        "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
    )
}

fn validate_sha256_contract_hash(value: &str) -> Result<String> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        bail!("contract hash must use the `sha256:<64 hex characters>` form");
    };
    if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("contract hash must use the `sha256:<64 hex characters>` form");
    }
    Ok(format!("sha256:{}", digest.to_ascii_lowercase()))
}

fn canonical_json_hash(document: &serde_json::Value) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(document)?);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub(super) fn verify_expected_contract_hash(
    expected: Option<&str>,
    report: &RemoteApiContractReport,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let expected = validate_sha256_contract_hash(expected)?;
    if expected != report.document_hash {
        bail!(
            "API contract document hash mismatch: expected {}, received {}",
            expected,
            report.document_hash
        );
    }
    Ok(())
}

pub(super) fn resolve_remote_contract_output(
    root: &Path,
    source: &str,
    name: Option<&str>,
    out: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(out) = out {
        if out.as_os_str().is_empty() || out == Path::new("-") {
            bail!("API contract pull output must be a file path");
        }
        return Ok(if out.is_absolute() {
            out.to_path_buf()
        } else {
            root.join(out)
        });
    }

    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| default_remote_contract_name(source));
    let name = sanitize_contract_file_name(&name)?;
    Ok(root
        .join(".axonyx/contracts")
        .join(format!("{name}.openapi.json")))
}

fn default_remote_contract_name(source: &str) -> String {
    if let Ok((_, authority)) = remote_contract_url_authority(source) {
        let host = if let Some(bracketed) = authority.strip_prefix('[') {
            bracketed.split(']').next().unwrap_or("remote")
        } else {
            authority.split(':').next().unwrap_or("remote")
        };
        return host.to_string();
    }
    Path::new(source)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("remote")
        .to_string()
}

fn sanitize_contract_file_name(value: &str) -> Result<String> {
    let mut name = String::new();
    let mut separator = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
            separator = false;
        } else if !name.is_empty() && !separator {
            name.push('-');
            separator = true;
        }
    }
    let name = name.trim_matches('-');
    if name.is_empty() {
        bail!("API contract name must contain at least one letter or number");
    }
    Ok(name.to_string())
}

fn contract_source_label(source: &str) -> String {
    if source.starts_with("https://") || source.starts_with("http://") {
        source
            .split(['?', '#'])
            .next()
            .unwrap_or(source)
            .to_string()
    } else {
        source.to_string()
    }
}

pub(super) fn print_remote_api_contract(report: &RemoteApiContractReport) {
    println!("Remote API contract:");
    println!("  source={}", report.source);
    println!(
        "  title={} version={} openapi={}",
        report.title, report.version, report.openapi
    );
    println!(
        "  paths={} operations={} schemas={}",
        report.paths, report.operations, report.schemas
    );
    println!("  contract_hash={}", report.contract_hash);
    println!("  document_hash={}", report.document_hash);
    if let Some(mode) = &report.response_validation {
        println!("  response_validation={mode}");
    }
}
