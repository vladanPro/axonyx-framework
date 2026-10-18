use super::*;

const PRIVATE_ROUTE: &str = "route GET \"/api/private/:id\" -> Binary {\n  return Native.privateResponse(\"media.read\", {id: params.id})\n}\n";

#[test]
fn private_binary_contract_is_not_a_named_record_and_requires_a_session() {
    let root = tests::make_temp_dir("private-binary-contract");
    fs::create_dir_all(root.join("routes/api")).unwrap();
    fs::write(root.join("routes/api/image.ax"), PRIVATE_ROUTE).unwrap();
    fs::write(root.join(".env"), "AX_SECRET_SESSION_KEY=fixture-only\n").unwrap();
    let diagnostics = check_ax_source_with_root(
        &root.join("routes/api/image.ax"),
        PRIVATE_ROUTE,
        Some(&root),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(backend_source_refs_use_sessions(&[PRIVATE_ROUTE]).unwrap());
    assert!(api_schema_type_names("Binary").is_empty());
    let plan = lower_backend_document(&parse_backend_ax(PRIVATE_ROUTE).unwrap()).unwrap();
    assert!(backend_plan_uses_signed_session(&plan));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_binary_contract_reports_auth_failures_openapi_and_stable_hashes() {
    let root = tests::make_temp_dir("private-binary-report");
    fs::create_dir_all(root.join("routes/api")).unwrap();
    fs::write(root.join("routes/api/image.ax"), PRIVATE_ROUTE).unwrap();
    let report = collect_api_report(&root).unwrap();
    let route = &report.routes[0];
    assert_eq!(route.returns.as_deref(), Some("Binary"));
    assert_eq!(route.auth[0].scheme, "signedSession");
    assert_eq!(
        route.responses.iter().map(|r| r.status).collect::<Vec<_>>(),
        [403, 404, 500]
    );
    let openapi = api_report_openapi_value(&report);
    let operation = &openapi["paths"]["/api/private/{id}"]["get"];
    let content = &operation["responses"]["200"]["content"];
    assert!(content.get("application/json").is_none());
    assert_eq!(content["image/png"]["schema"]["format"], "binary");
    assert_eq!(content.as_object().unwrap().len(), 3);
    assert!(openapi["components"]["schemas"].get("Binary").is_none());
    assert_eq!(
        operation["security"][0]["signedSessionAuth"],
        serde_json::json!([])
    );
    let repeated = collect_api_report(&root).unwrap();
    assert_eq!(report.contract_hash, repeated.contract_hash);
    let mut json_route = route.clone();
    json_route.returns = Some("Json".into());
    assert_ne!(
        api_route_response_hash(route, &[]).unwrap(),
        api_route_response_hash(&json_route, &[]).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_binary_diagnostics_share_compiler_validation() {
    for source in [
        "route POST \"/private\" -> Binary {\n  return Native.privateResponse(\"media.read\", {})\n}",
        "route GET \"/private\" -> Binary {\n  return json({ok: true})\n}",
        "route GET \"/private\" -> Binary {\n  header \"Cache-Control\" = \"public\"\n  return Native.privateResponse(\"media.read\", {})\n}",
    ] {
        let diagnostics = check_ax_source_with_root(Path::new("routes/private.ax"), source, None);
        assert!(diagnostics.iter().any(|d| d.code == "axonyx-private-response-contract"), "{diagnostics:?}");
    }
}
