use super::*;

const PUBLIC_ROUTE: &str = "route GET \"/api/public/:id\" -> Binary {\n  return Native.publicResponse(\"media.public\", { id: params.id })\n}\n";

#[test]
fn public_binary_check_needs_neither_database_nor_session_configuration() {
    let root = tests::make_temp_dir("public-binary-contract");
    let diagnostics =
        check_ax_source_with_root(&root.join("routes/api/image.ax"), PUBLIC_ROUTE, Some(&root));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(!backend_source_refs_use_sessions(&[PUBLIC_ROUTE]).unwrap());
    let plan = lower_backend_document(&parse_backend_ax(PUBLIC_ROUTE).unwrap()).unwrap();
    assert!(!backend_plan_uses_signed_session(&plan));
    assert!(!backend_plan_uses_database(&plan));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_report_is_binary_without_security_and_retains_failure_contract() {
    let root = tests::make_temp_dir("public-binary-report");
    fs::create_dir_all(root.join("routes/api")).unwrap();
    fs::write(root.join("routes/api/image.ax"), PUBLIC_ROUTE).unwrap();
    let report = collect_api_report(&root).unwrap();
    let route = &report.routes[0];
    assert_eq!(route.returns.as_deref(), Some("Binary"));
    assert!(route.auth.is_empty());
    assert_eq!(
        route.responses.iter().map(|r| r.status).collect::<Vec<_>>(),
        [403, 404, 500]
    );
    let openapi = api_report_openapi_value(&report);
    let operation = &openapi["paths"]["/api/public/{id}"]["get"];
    assert!(operation.get("security").is_none());
    assert_eq!(
        operation["responses"]["200"]["content"]["image/png"]["schema"]["format"],
        "binary"
    );
    assert_eq!(
        operation["responses"]["200"]["headers"]["Cache-Control"]["schema"]["const"],
        "private, no-store"
    );
    assert!(operation["responses"]["200"]["content"]
        .get("application/json")
        .is_none());
    assert_eq!(
        report.contract_hash,
        collect_api_report(&root).unwrap().contract_hash
    );
    let mut private = route.clone();
    private.auth.push(ApiAuthReport {
        scheme: "signedSession",
    });
    assert_ne!(
        api_route_request_hash(route, &[]).unwrap(),
        api_route_request_hash(&private, &[]).unwrap()
    );
    assert_eq!(
        api_route_response_hash(route, &[]).unwrap(),
        api_route_response_hash(&private, &[]).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_public_subject_payload_needs_session_but_literal_text_does_not() {
    for (payload, expected) in [
        ("{ subject: Auth.subject }", true),
        ("{ text: \"Auth.subject\" }", false),
    ] {
        let source = format!("route GET \"/image\" -> Binary {{\n  return Native.publicResponse(\"media.public\", {payload})\n}}");
        let plan = lower_backend_document(&parse_backend_ax(&source).unwrap()).unwrap();
        assert_eq!(backend_plan_uses_signed_session(&plan), expected);
        assert_eq!(
            backend_source_refs_use_sessions(&[source.as_str()]).unwrap(),
            expected
        );
        let document = parse_backend_ax(&source).unwrap();
        let route = document
            .blocks
            .iter()
            .find_map(|block| match block {
                AxBackendBlock::Route(route) => Some(route),
                _ => None,
            })
            .unwrap();
        assert_eq!(!route_auth_from_body(&route.body).is_empty(), expected);
    }
}

#[test]
fn public_diagnostic_points_to_call_and_env_payload_refs_are_discovered() {
    let invalid = "route POST \"/image\" -> Binary {\n  return Native.publicResponse(\"media.public\", {})\n}";
    let diagnostics = check_ax_source_with_root(Path::new("routes/public.ax"), invalid, None);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "axonyx-public-response-contract" && d.line == 2),
        "{diagnostics:?}"
    );
    let source = "route GET \"/image\" -> Binary {\n  return Native.publicResponse(\"media.public\", { key: env.IMAGE_KEY })\n}";
    let plan = lower_backend_document(&parse_backend_ax(source).unwrap()).unwrap();
    let mut refs = std::collections::BTreeSet::new();
    if let AxStepPlan::Return(value) = &plan.handlers[0].steps[0] {
        collect_env_refs_from_return(value, &mut refs);
    }
    assert!(refs.contains("IMAGE_KEY"));
}
