use super::action_selector::{parse, ActionSelector};

#[test]
fn strict_action_selectors_preserve_valid_routes_names_and_nested_query_values() {
    for target in [
        "/__axonyx/action?path=%2Fposts%3Fsearch%3Dhello%2Bworld&name=CreatePost",
        "/__axonyx/action?name=CreatePost&path=%2Fposts%3Fsearch%3Dhello%2Bworld&extra=yes",
    ] {
        assert_eq!(
            parse(target),
            Some(ActionSelector {
                path: "/posts?search=hello+world".into(),
                name: "CreatePost".into(),
            })
        );
    }
    assert_eq!(
        parse("/__axonyx/action?path=%2Fcaf%C3%A9&name=Save"),
        Some(ActionSelector {
            path: "/café".into(),
            name: "Save".into(),
        })
    );
    assert_eq!(
        parse("/__axonyx/action?path=%2Fdocs%252Fintro&name=Save")
            .unwrap()
            .path,
        "/docs%2Fintro"
    );
}

#[test]
fn strict_action_selectors_reject_duplicates_encoded_aliases_and_missing_fields() {
    for target in [
        "/__axonyx/action?path=%2F&name=Save&name=Other",
        "/__axonyx/action?path=%2F&path=%2Fother&name=Save",
        "/__axonyx/action?path=%2F&name=Save&%6Eame=Other",
        "/__axonyx/action?%70ath=%2F&name=Save",
        "/__axonyx/action?path=%2F&%6Eame=Save",
        "/__axonyx/action?name=Save",
        "/__axonyx/action?path=%2F",
        "/__axonyx/action?path=&name=Save",
        "/__axonyx/action?path=%2F&name=",
    ] {
        assert!(
            parse(target).is_none(),
            "accepted ambiguous selector: {target}"
        );
    }
}

#[test]
fn strict_action_selectors_reject_malformed_encoding_without_lossy_replacement() {
    for suffix in [
        "%",
        "%A",
        "%GG",
        "%ff",
        "%C3%28",
        "Save%00",
        "Save%0a",
        "Save+Post",
    ] {
        assert!(parse(&format!("/__axonyx/action?path=%2F&name={suffix}")).is_none());
    }
    assert!(parse("/__axonyx/action?path=%2F&name=Save&extra=%GG").is_none());
}

#[test]
fn strict_action_selectors_reject_unsafe_paths_and_wrong_endpoints() {
    for path in [
        "relative",
        "%2F%2Fother",
        "%2F..%2Fprivate",
        "%2F.%2Fprivate",
        "%2F%5Cprivate",
        "%20%2Fposts",
        "%2Fposts%20",
        "%2Fposts%23fragment",
        "%2Fposts%0A",
    ] {
        assert!(parse(&format!("/__axonyx/action?path={path}&name=Save")).is_none());
    }
    assert!(parse("/other?path=%2F&name=Save").is_none());
    assert!(parse("/__axonyx/action?path=%2F&name=Save#fragment").is_none());
}
