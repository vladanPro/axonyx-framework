use super::{diagnostic, AxTranslationCatalog, CheckDiagnostic};
use axonyx_core::ax_ast::{
    AxBody, AxComponent, AxComponentParamDef, AxDocument, AxExpr, AxPipelineStage, AxStatement,
};
use std::path::Path;

pub(crate) fn check_document(
    path: &Path,
    source: &str,
    document: &AxDocument,
    catalog: &AxTranslationCatalog,
) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut report = |pattern: &str, message: String| {
        let line = crate::line_for_source_pattern(source, pattern);
        diagnostics.push(diagnostic(path, line, 1, message));
    };
    if document
        .functions
        .iter()
        .any(|function| function.name == "t")
    {
        report(
            "fn t",
            "function `t` is reserved for translation in .asx when [i18n] is enabled".into(),
        );
    }

    let mut check_call = |args: &[AxExpr]| match args {
        [AxExpr::String(_), AxExpr::Bool(_)
        | AxExpr::Number(_)
        | AxExpr::Float(_)
        | AxExpr::List(_)
        | AxExpr::Object(_)] => {
            report("t(", "the optional `t()` locale must be a string".into());
        }
        [AxExpr::String(key)] | [AxExpr::String(key), _] if catalog.contains_key(key) => {}
        [AxExpr::String(key)] | [AxExpr::String(key), _] => report(
            &format!("{key:?}"),
            format!("unknown translation key {key:?}; define it in the fallback catalog"),
        ),
        _ => report(
            "t(",
            "`t()` expects a literal string key and an optional locale".into(),
        ),
    };
    let mut expressions = |expr: &AxExpr| visit_translation_calls(expr, &mut check_call);
    visit_defaults(&document.page.params, &mut expressions);
    visit_statements(&document.page.body, &mut expressions);
    for function in &document.functions {
        visit_defaults(&function.params, &mut expressions);
        expressions(&function.body);
    }
    for component in &document.components {
        visit_defaults(&component.params, &mut expressions);
        for state in &component.states {
            expressions(&state.initial);
        }
        visit_statements(&component.body, &mut expressions);
    }

    let head = &document.head;
    for expr in [&head.title, &head.theme, &head.theme_storage_key]
        .into_iter()
        .flatten()
        .chain(
            head.metas
                .iter()
                .chain(&head.links)
                .chain(&head.scripts)
                .flat_map(|tag| tag.attrs.iter().map(|prop| &prop.value)),
        )
    {
        visit_translation_calls(expr, &mut |_| {
            report(
                "t(",
                "translation calls in Head are not supported yet".into(),
            );
        });
    }
    diagnostics
}

fn visit_defaults(params: &[AxComponentParamDef], visit: &mut impl FnMut(&AxExpr)) {
    for default in params.iter().filter_map(|param| param.default.as_ref()) {
        visit(default);
    }
}

fn visit_statements(statements: &[AxStatement], visit: &mut impl FnMut(&AxExpr)) {
    for statement in statements {
        match statement {
            AxStatement::Data(binding) => visit(&binding.value),
            AxStatement::Text(expr) => visit(expr),
            AxStatement::Each(block) => {
                visit(&block.source);
                if let Some(key) = &block.key {
                    visit(key);
                }
                visit_statements(&block.body, visit);
                visit_statements(&block.empty_body, visit);
            }
            AxStatement::If(block) => {
                visit(&block.condition);
                visit_statements(&block.body, visit);
                visit_statements(&block.else_body, visit);
            }
            AxStatement::Match(block) => {
                visit(&block.value);
                for case in &block.cases {
                    visit_statements(&case.body, visit);
                }
                if let Some(body) = &block.default_body {
                    visit_statements(body, visit);
                }
            }
            AxStatement::Component(component) => visit_component(component, visit),
            AxStatement::Pipeline(pipeline) => {
                visit(&pipeline.source);
                for stage in &pipeline.stages {
                    if let AxPipelineStage::Component(component) = stage {
                        visit_component(component, visit);
                    }
                }
            }
        }
    }
}

fn visit_component(component: &AxComponent, visit: &mut impl FnMut(&AxExpr)) {
    for prop in &component.props {
        visit(&prop.value);
    }
    for expr in [&component.style.recipe, &component.style.class]
        .into_iter()
        .flatten()
    {
        visit(expr);
    }
    match &component.body {
        AxBody::Empty => {}
        AxBody::Inline(expr) => visit(expr),
        AxBody::Block(statements) => visit_statements(statements, visit),
    }
}

fn visit_translation_calls(expr: &AxExpr, visit: &mut impl FnMut(&[AxExpr])) {
    match expr {
        AxExpr::Call { path, args } => {
            if path == &["t"] {
                visit(args);
            }
            for arg in args {
                visit_translation_calls(arg, visit);
            }
        }
        AxExpr::List(items) => {
            for item in items {
                visit_translation_calls(item, visit);
            }
        }
        AxExpr::Object(fields) => {
            for field in fields.values() {
                visit_translation_calls(field, visit);
            }
        }
        AxExpr::Binary { left, right, .. }
        | AxExpr::Index {
            object: left,
            index: right,
        } => {
            visit_translation_calls(left, visit);
            visit_translation_calls(right, visit);
        }
        AxExpr::Unary { expr, .. }
        | AxExpr::Member { object: expr, .. }
        | AxExpr::OptionalMember { object: expr, .. } => visit_translation_calls(expr, visit),
        AxExpr::String(_)
        | AxExpr::Number(_)
        | AxExpr::Float(_)
        | AxExpr::Bool(_)
        | AxExpr::Identifier(_) => {}
    }
}
