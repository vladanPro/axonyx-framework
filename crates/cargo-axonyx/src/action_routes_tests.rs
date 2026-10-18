use super::action_routes::{allows, body_limit, resolve, Route};

#[test]
fn action_ownership_is_checked_after_page_resolution() {
    let routes: &[Route<'_>] = &[
        ("/posts/:slug", &["Upload", "Edit"]),
        ("/posts/new", &[]),
        ("/posts", &["Create"]),
    ];
    assert!(allows(routes, "/posts/hello?tab=edit", "Upload"));
    assert!(allows(routes, "/posts//hello/", "Edit"));
    assert!(!allows(routes, "/posts/new", "Upload"));
    assert!(!allows(routes, "/posts", "Upload"));
    assert!(!allows(routes, "/missing", "Create"));
    assert!(!allows(routes, "/posts/hello", "Missing"));
    assert_eq!(resolve(routes, "/posts/new"), Some(1));
}

#[test]
fn route_order_does_not_depend_on_manifest_enumeration() {
    let forward: &[Route<'_>] = &[
        ("/a/:id/end", &["Dynamic"]),
        ("/a/static/:tail", &["Static"]),
    ];
    let reverse = [forward[1], forward[0]];
    for routes in [forward, reverse.as_slice()] {
        assert!(allows(routes, "/a/static/end", "Static"));
        assert!(!allows(routes, "/a/static/end", "Dynamic"));
    }
}

#[test]
fn dynamic_siblings_follow_filesystem_branch_priority() {
    let routes: &[Route<'_>] = &[
        ("/:z/exact", &["LaterBranch"]),
        ("/:a/:tail", &["FirstBranch"]),
    ];
    assert!(allows(routes, "/value/exact", "FirstBranch"));
    assert!(!allows(routes, "/value/exact", "LaterBranch"));
    assert!(allows(
        &[("/:z/exact", &["Fallback"])],
        "/value/exact",
        "Fallback"
    ));
}

#[test]
fn duplicate_winning_patterns_fail_closed() {
    assert!(!allows(&[("/", &["Root"]), ("/", &["Root"])], "/", "Root"));
    assert!(allows(&[("/", &["Root"])], "/?tab=home", "Root"));
    assert!(!allows(&[("/", &["Root"])], "/a", "Root"));
}

#[test]
fn scoped_body_limit_requires_known_route_action_and_unambiguous_post_selector() {
    let routes: &[Route<'_>] = &[("/posts/:id", &["Upload"]), ("/posts/new", &[])];
    let limits = [("/posts/:id", "Upload", 9 * 1024 * 1024)];
    assert_eq!(
        body_limit(
            routes,
            &limits,
            "POST",
            "/__axonyx/action?path=%2Fposts%2F42&name=Upload"
        ),
        Some(9 * 1024 * 1024)
    );
    for target in [
        "/__axonyx/action?path=%2Fposts%2Fnew&name=Upload",
        "/__axonyx/action?path=%2Fmissing&name=Upload",
        "/__axonyx/action?path=%2Fposts%2F42&name=Missing",
        "/__axonyx/action?path=%2Fposts%2F42&name=Upload&name=Upload",
        "/__axonyx/action?path=%2Fposts%2F42&name=Upload&%6eame=Upload",
        "/__axonyx/action?path=%2Fposts%2F42&name=Upload&extra=%GG",
        "/__axonyx/action?path=%2Fposts%2F42&name=Upload&extra=%FF",
        "/api/posts?path=%2Fposts%2F42&name=Upload",
    ] {
        assert_eq!(
            body_limit(routes, &limits, "POST", target),
            None,
            "{target}"
        );
    }
    assert_eq!(
        body_limit(
            routes,
            &limits,
            "GET",
            "/__axonyx/action?path=%2Fposts%2F42&name=Upload"
        ),
        None
    );
}

#[test]
fn scoped_config_and_doctor_reject_unknown_duplicate_or_invalid_limits() {
    use super::{
        doctor_action_body_limits_check, ensure_no_preview_action_body_limits,
        parse_action_body_limits, DoctorSeverity,
    };
    let routes = vec![("/upload".to_string(), vec!["Upload".to_string()])];
    let root = super::tests::make_temp_dir("action-body-limit-config");
    std::fs::create_dir_all(root.join("app/upload")).unwrap();
    std::fs::write(
        root.join("app/upload/page.asx"),
        "page Upload() { return ASX { <p>Upload</p> } }",
    )
    .unwrap();
    std::fs::write(
        root.join("app/upload/actions.ax"),
        "action Upload() {\n  return ok()\n}\n",
    )
    .unwrap();
    for entry in [
        "path = '/upload'\naction = 'Missing'\nmax_body_bytes = '9mb'",
        "path = '/missing'\naction = 'Upload'\nmax_body_bytes = '9mb'",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = 0",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = -1",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = 'overflow'",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = true",
        "path = '/upload'\naction = 'Upload'",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = '9mb'\nunknown = 1",
        "path = '/upload'\naction = 'Upload'\nmax_body_bytes = '9mb'\n[[server.action_body_limits]]\npath = '/upload'\naction = 'Upload'\nmax_body_bytes = '8mb'",
    ] {
        let source = format!("[[server.action_body_limits]]\n{entry}\n");
        let value: toml::Value = source.parse().unwrap();
        assert!(parse_action_body_limits(&value["server"]["action_body_limits"], &routes).is_err(), "{entry}");
        std::fs::write(root.join("Axonyx.toml"), source).unwrap();
        assert!(super::check_axonyx_config(&root).unwrap().iter().any(|diagnostic| diagnostic.code == "axonyx-config-action-body-limits"));
        assert!(matches!(doctor_action_body_limits_check(&root).severity, DoctorSeverity::Error));
    }
    std::fs::write(root.join("Axonyx.toml"), "[server]\nmax_body_bytes = '64kb'\n[[server.action_body_limits]]\npath = '/upload'\naction = 'Upload'\nmax_body_bytes = '9mb'\n").unwrap();
    assert_eq!(
        super::configured_action_body_limits(&root).unwrap(),
        vec![("/upload".into(), "Upload".into(), 9 * 1024 * 1024)]
    );
    assert!(matches!(
        doctor_action_body_limits_check(&root).severity,
        DoctorSeverity::Ok
    ));
    assert!(ensure_no_preview_action_body_limits(&root)
        .unwrap_err()
        .to_string()
        .contains("--compiled"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_table_preserves_actionless_pages_and_matches_filesystem_routing() {
    let root = super::tests::make_temp_dir("action-route-table");
    for route in ["posts/[slug]", "posts/new", "[a]/[tail]", "[z]/exact"] {
        let dir = root.join("app").join(route);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("page.asx"),
            "page Example() { return ASX { <p>Example</p> } }",
        )
        .unwrap();
    }
    std::fs::write(
        root.join("app/posts/[slug]/actions.ax"),
        "action Upload() {\n  return ok()\n}\n",
    )
    .unwrap();
    let owned = super::compiled_action_routes(&root).unwrap();
    let names = owned
        .iter()
        .map(|(_, names)| names.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let routes = owned
        .iter()
        .zip(&names)
        .map(|((path, _), names)| (path.as_str(), names.as_slice()))
        .collect::<Vec<_>>();
    for path in [
        "/posts/new",
        "/posts/hello",
        "/foo/exact",
        "/foo/else",
        "/posts//hello/",
    ] {
        let index = resolve(&routes, path).unwrap();
        let resolved = super::resolve_route(&root, path).unwrap().unwrap();
        let expected = super::collect_page_route_manifest(&root)
            .unwrap()
            .into_iter()
            .find(|route| root.join(&route.file) == resolved.page_path)
            .unwrap();
        assert_eq!(routes[index].0, expected.route, "{path}");
    }
    assert!(!allows(&routes, "/posts/new", "Upload"));
    assert!(allows(&routes, "/posts/hello", "Upload"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_host_embeds_policy_only_for_explicit_scoped_limits() {
    let routes = vec![
        ("/upload".into(), vec!["Upload".into()]),
        ("/upload/new".into(), vec![]),
    ];
    let limits = vec![("/upload".into(), "Upload".into(), 9 * 1024 * 1024)];
    let source = super::compiled_production_source(
        "\"dist\"",
        &[],
        &[],
        &[],
        super::CompiledProductionOptions {
            native_registration: false,
            native_storage_registration: false,
            max_body_bytes: 64 * 1024,
            sessions_required: false,
            public_origin: None,
            database_runtime_defaults: "",
            database_required: false,
            validate_api_responses: false,
            storage_configs: &[],
            action_routes: &routes,
            action_body_limits: &limits,
        },
    );
    assert!(source.contains(include_str!("action_routes.rs")));
    assert!(source.contains("(\"/upload\", &[\"Upload\"]),"));
    assert!(source.contains("(\"/upload/new\", &[]),"));
    assert!(source.contains("(\"/upload\", \"Upload\", 9437184),"));
    assert!(source.contains("serve_compiled_axum_with_body_limits(bind, 65536, handler, resolver)"));
    assert!(!source.contains("server_prelude::{serve_compiled_axum,"));
}
