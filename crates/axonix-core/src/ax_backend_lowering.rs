use thiserror::Error;

use crate::ax_ast::prelude::AxExpr;
use crate::ax_backend_ast::prelude::*;
use crate::ax_query_ast::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxBackendPlan {
    pub handlers: Vec<AxHandlerPlan>,
}

impl AxBackendPlan {
    pub fn new(handlers: impl IntoIterator<Item = AxHandlerPlan>) -> Self {
        Self {
            handlers: handlers.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxHandlerPlan {
    pub name: String,
    pub rust_fn: String,
    pub kind: AxHandlerKind,
    pub steps: Vec<AxStepPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxHandlerKind {
    Route { method: String, path: String },
    Loader,
    Action { input: Vec<AxFieldPlan> },
    Job,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxFieldPlan {
    pub name: String,
    pub rust_ty: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxStepPlan {
    Let {
        binding: String,
        value: AxValuePlan,
    },
    Insert {
        collection: String,
        fields: Vec<AxAssignmentPlan>,
    },
    Update {
        collection: String,
        fields: Vec<AxAssignmentPlan>,
        filters: Vec<AxQueryFilterPlan>,
    },
    Revalidate {
        target: AxRustExpr,
    },
    Return(AxReturnPlan),
    Send {
        target: String,
        payload: AxRustExpr,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxValuePlan {
    Expr(AxRustExpr),
    Query(AxQueryPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxAssignmentPlan {
    pub name: String,
    pub value: AxRustExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxReturnPlan {
    Expr(AxRustExpr),
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxQueryPlan {
    pub source: AxQuerySourcePlan,
    pub filters: Vec<AxQueryFilterPlan>,
    pub orders: Vec<AxQueryOrderPlan>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxQuerySourcePlan {
    Stream { collection: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxQueryFilterPlan {
    pub field: String,
    pub op: AxQueryFilterOpPlan,
    pub value: AxRustExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxQueryFilterOpPlan {
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxQueryOrderPlan {
    pub field: String,
    pub direction: AxQueryOrderDirectionPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxQueryOrderDirectionPlan {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxRustExpr {
    pub code: String,
}

impl AxRustExpr {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxBackendLowerError {
    #[error("handler name cannot be empty")]
    EmptyHandlerName,
    #[error("route path cannot be empty")]
    EmptyRoutePath,
    #[error("route method cannot be empty")]
    EmptyRouteMethod,
    #[error("input field type cannot be empty for `{field}`")]
    EmptyInputType { field: String },
    #[error("invalid runtime env path `{path}`")]
    InvalidRuntimeEnvPath { path: String },
}

pub fn lower_backend_document(
    document: &AxBackendDocument,
) -> Result<AxBackendPlan, AxBackendLowerError> {
    let handlers = document
        .blocks
        .iter()
        .map(lower_backend_block)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AxBackendPlan::new(handlers))
}

fn lower_backend_block(block: &AxBackendBlock) -> Result<AxHandlerPlan, AxBackendLowerError> {
    match block {
        AxBackendBlock::Route(route) => lower_route(route),
        AxBackendBlock::Loader(loader) => lower_loader(loader),
        AxBackendBlock::Action(action) => lower_action(action),
        AxBackendBlock::Job(job) => lower_job(job),
    }
}

fn lower_route(route: &AxRoute) -> Result<AxHandlerPlan, AxBackendLowerError> {
    if route.method.trim().is_empty() {
        return Err(AxBackendLowerError::EmptyRouteMethod);
    }
    if route.path.trim().is_empty() {
        return Err(AxBackendLowerError::EmptyRoutePath);
    }

    let name = format!("route {} {}", route.method, route.path);
    let rust_fn = format!(
        "route_{}_{}",
        route_method_ident(&route.method),
        route_path_ident(&route.path)
    );

    Ok(AxHandlerPlan {
        name,
        rust_fn,
        kind: AxHandlerKind::Route {
            method: route.method.clone(),
            path: route.path.clone(),
        },
        steps: lower_steps(&route.body),
    })
}

fn lower_loader(loader: &AxLoader) -> Result<AxHandlerPlan, AxBackendLowerError> {
    let name = loader.name.trim();
    if name.is_empty() {
        return Err(AxBackendLowerError::EmptyHandlerName);
    }

    Ok(AxHandlerPlan {
        name: loader.name.clone(),
        rust_fn: format!("loader_{}", normalize_ident(name)),
        kind: AxHandlerKind::Loader,
        steps: lower_steps(&loader.body),
    })
}

fn lower_action(action: &AxAction) -> Result<AxHandlerPlan, AxBackendLowerError> {
    let name = action.name.trim();
    if name.is_empty() {
        return Err(AxBackendLowerError::EmptyHandlerName);
    }

    let input = action
        .input
        .iter()
        .map(lower_input_field)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AxHandlerPlan {
        name: action.name.clone(),
        rust_fn: format!("action_{}", normalize_ident(name)),
        kind: AxHandlerKind::Action { input },
        steps: lower_steps(&action.body),
    })
}

fn lower_job(job: &AxJob) -> Result<AxHandlerPlan, AxBackendLowerError> {
    let name = job.name.trim();
    if name.is_empty() {
        return Err(AxBackendLowerError::EmptyHandlerName);
    }

    Ok(AxHandlerPlan {
        name: job.name.clone(),
        rust_fn: format!("job_{}", normalize_ident(name)),
        kind: AxHandlerKind::Job,
        steps: lower_steps(&job.body),
    })
}

fn lower_input_field(field: &AxField) -> Result<AxFieldPlan, AxBackendLowerError> {
    if field.ty.trim().is_empty() {
        return Err(AxBackendLowerError::EmptyInputType {
            field: field.name.clone(),
        });
    }

    Ok(AxFieldPlan {
        name: field.name.clone(),
        rust_ty: map_input_type(&field.ty),
    })
}

fn lower_steps(steps: &[AxBackendStmt]) -> Vec<AxStepPlan> {
    steps.iter().map(lower_step).collect()
}

fn lower_step(step: &AxBackendStmt) -> AxStepPlan {
    match step {
        AxBackendStmt::Data(data) => AxStepPlan::Let {
            binding: data.name.clone(),
            value: lower_backend_value(&data.value),
        },
        AxBackendStmt::Insert(mutation) => AxStepPlan::Insert {
            collection: mutation.collection.clone(),
            fields: lower_assignments(&mutation.fields),
        },
        AxBackendStmt::Update(mutation) => AxStepPlan::Update {
            collection: mutation.collection.clone(),
            fields: lower_assignments(&mutation.fields),
            filters: mutation
                .filters
                .iter()
                .map(|filter| AxQueryFilterPlan {
                    field: filter.field.clone(),
                    op: match filter.op {
                        AxQueryFilterOp::Eq => AxQueryFilterOpPlan::Eq,
                    },
                    value: lower_expr(&filter.value),
                })
                .collect(),
        },
        AxBackendStmt::Revalidate(expr) => AxStepPlan::Revalidate {
            target: lower_expr(expr),
        },
        AxBackendStmt::Return(value) => AxStepPlan::Return(lower_return(value)),
        AxBackendStmt::Send(send) => AxStepPlan::Send {
            target: send.target.clone(),
            payload: lower_expr(&send.payload),
        },
    }
}

fn lower_backend_value(value: &AxBackendValue) -> AxValuePlan {
    match value {
        AxBackendValue::Expr(expr) => AxValuePlan::Expr(lower_expr(expr)),
        AxBackendValue::Query(query) => AxValuePlan::Query(lower_query(query)),
    }
}

fn lower_assignments(fields: &[AxAssignment]) -> Vec<AxAssignmentPlan> {
    fields
        .iter()
        .map(|field| AxAssignmentPlan {
            name: field.name.clone(),
            value: lower_expr(&field.value),
        })
        .collect()
}

fn lower_return(value: &AxReturn) -> AxReturnPlan {
    match value {
        AxReturn::Expr(expr) => AxReturnPlan::Expr(lower_expr(expr)),
        AxReturn::Ok => AxReturnPlan::Ok,
    }
}

fn lower_query(query: &AxQuerySpec) -> AxQueryPlan {
    AxQueryPlan {
        source: match &query.source {
            AxQuerySource::Stream { collection } => AxQuerySourcePlan::Stream {
                collection: collection.clone(),
            },
        },
        filters: query
            .filters
            .iter()
            .map(|filter| AxQueryFilterPlan {
                field: filter.field.clone(),
                op: match filter.op {
                    AxQueryFilterOp::Eq => AxQueryFilterOpPlan::Eq,
                },
                value: lower_expr(&filter.value),
            })
            .collect(),
        orders: query
            .orders
            .iter()
            .map(|order| AxQueryOrderPlan {
                field: order.field.clone(),
                direction: match order.direction {
                    AxQueryOrderDirection::Asc => AxQueryOrderDirectionPlan::Asc,
                    AxQueryOrderDirection::Desc => AxQueryOrderDirectionPlan::Desc,
                },
            })
            .collect(),
        limit: query.limit,
        offset: query.offset,
    }
}

fn lower_expr(expr: &AxExpr) -> AxRustExpr {
    AxRustExpr::new(render_expr(expr))
}

fn render_expr(expr: &AxExpr) -> String {
    if let Some(env_expr) = try_render_runtime_env(expr) {
        return env_expr;
    }

    match expr {
        AxExpr::String(value) => format!("{value:?}.to_string()"),
        AxExpr::Number(value) => value.to_string(),
        AxExpr::Bool(value) => value.to_string(),
        AxExpr::Identifier(name) => name.clone(),
        AxExpr::Member { object, property } => format!("{}.{}", render_expr(object), property),
        AxExpr::Call { path, args } => {
            let fn_name = path.join("::");
            let args = args.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("{fn_name}({args})")
        }
    }
}

fn try_render_runtime_env(expr: &AxExpr) -> Option<String> {
    let path = expr_member_path(expr)?;
    let normalized = path.iter().map(|segment| segment.as_str()).collect::<Vec<_>>();

    match normalized.as_slice() {
        ["Runtime", "Env", "public", key] => {
            Some(format!("runtime.env().public({key:?})?"))
        }
        ["Runtime", "Env", "secret", key] => {
            Some(format!("runtime.env().secret({key:?})?"))
        }
        _ => None,
    }
}

fn expr_member_path(expr: &AxExpr) -> Option<Vec<String>> {
    match expr {
        AxExpr::Identifier(name) => Some(vec![name.clone()]),
        AxExpr::Member { object, property } => {
            let mut path = expr_member_path(object)?;
            path.push(property.clone());
            Some(path)
        }
        _ => None,
    }
}

fn map_input_type(ty: &str) -> String {
    match ty.trim() {
        "string" => "String".to_string(),
        "bool" | "boolean" => "bool".to_string(),
        "i64" | "int" | "integer" => "i64".to_string(),
        "u64" => "u64".to_string(),
        "f64" | "float" | "number" => "f64".to_string(),
        other => other.to_string(),
    }
}

fn normalize_ident(input: &str) -> String {
    let mut out = String::new();
    let mut previous_was_sep = true;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !out.is_empty() && !previous_was_sep {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            previous_was_sep = false;
        } else if !previous_was_sep {
            out.push('_');
            previous_was_sep = true;
        }
    }

    out.trim_matches('_').to_string()
}

fn route_path_ident(path: &str) -> String {
    let normalized = normalize_ident(path);
    if normalized.is_empty() {
        "root".to_string()
    } else {
        normalized
    }
}

fn route_method_ident(method: &str) -> String {
    let method = method.trim();
    if method.chars().all(|ch| !ch.is_ascii_alphabetic() || ch.is_ascii_uppercase()) {
        method.to_ascii_lowercase()
    } else {
        normalize_ident(method)
    }
}

pub mod prelude {
    pub use super::lower_backend_document;
    pub use super::AxAssignmentPlan;
    pub use super::AxBackendLowerError;
    pub use super::AxBackendPlan;
    pub use super::AxFieldPlan;
    pub use super::AxHandlerKind;
    pub use super::AxHandlerPlan;
    pub use super::AxQueryFilterPlan;
    pub use super::AxQueryFilterOpPlan;
    pub use super::AxQueryOrderPlan;
    pub use super::AxQueryOrderDirectionPlan;
    pub use super::AxQueryPlan;
    pub use super::AxQuerySourcePlan;
    pub use super::AxReturnPlan;
    pub use super::AxRustExpr;
    pub use super::AxStepPlan;
    pub use super::AxValuePlan;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ax_backend_parser::parse_backend_ax;

    #[test]
    fn lowers_loader_query_into_backend_plan() {
        let document = parse_backend_ax(
            r#"
loader PostsList
  data posts = Db.Stream("posts")
    where status = "published"
    order created_at desc
    limit 20
    offset 40
  return posts
"#,
        )
        .expect("document should parse");

        let plan = lower_backend_document(&document).expect("document should lower");

        assert_eq!(plan.handlers.len(), 1);
        let handler = &plan.handlers[0];
        assert_eq!(handler.name, "PostsList");
        assert_eq!(handler.rust_fn, "loader_posts_list");
        assert_eq!(handler.kind, AxHandlerKind::Loader);

        let AxStepPlan::Let { binding, value } = &handler.steps[0] else {
            panic!("expected let step");
        };
        assert_eq!(binding, "posts");
        assert_eq!(
            value,
            &AxValuePlan::Query(AxQueryPlan {
                source: AxQuerySourcePlan::Stream {
                    collection: "posts".to_string(),
                },
                filters: vec![AxQueryFilterPlan {
                    field: "status".to_string(),
                    op: AxQueryFilterOpPlan::Eq,
                    value: AxRustExpr::new(r#""published".to_string()"#),
                }],
                orders: vec![AxQueryOrderPlan {
                    field: "created_at".to_string(),
                    direction: AxQueryOrderDirectionPlan::Desc,
                }],
                limit: Some(20),
                offset: Some(40),
            })
        );

        assert_eq!(
            handler.steps[1],
            AxStepPlan::Return(AxReturnPlan::Expr(AxRustExpr::new("posts")))
        );
    }

    #[test]
    fn lowers_action_input_and_mutation_into_rust_shaped_plan() {
        let document = parse_backend_ax(
            r#"
action CreatePost
  input:
    title: string
    featured: bool

  insert "posts"
    title: input.title
    featured: input.featured

  revalidate "/posts"
  return ok
"#,
        )
        .expect("document should parse");

        let plan = lower_backend_document(&document).expect("document should lower");

        let handler = &plan.handlers[0];
        assert_eq!(handler.rust_fn, "action_create_post");
        assert_eq!(
            handler.kind,
            AxHandlerKind::Action {
                input: vec![
                    AxFieldPlan {
                        name: "title".to_string(),
                        rust_ty: "String".to_string(),
                    },
                    AxFieldPlan {
                        name: "featured".to_string(),
                        rust_ty: "bool".to_string(),
                    },
                ],
            }
        );
        assert_eq!(
            handler.steps[0],
            AxStepPlan::Insert {
                collection: "posts".to_string(),
                fields: vec![
                    AxAssignmentPlan {
                        name: "title".to_string(),
                        value: AxRustExpr::new("input.title"),
                    },
                    AxAssignmentPlan {
                        name: "featured".to_string(),
                        value: AxRustExpr::new("input.featured"),
                    },
                ],
            }
        );
        assert_eq!(
            handler.steps[1],
            AxStepPlan::Revalidate {
                target: AxRustExpr::new(r#""/posts".to_string()"#),
            }
        );
        assert_eq!(handler.steps[2], AxStepPlan::Return(AxReturnPlan::Ok));
    }

    #[test]
    fn lowers_update_where_clause_into_runtime_filters() {
        let document = parse_backend_ax(
            r#"
action PublishPost
  input:
    id: i64
    title: string

  update "posts"
    title: input.title
    where id = input.id

  return ok
"#,
        )
        .expect("document should parse");

        let plan = lower_backend_document(&document).expect("document should lower");
        let handler = &plan.handlers[0];

        assert_eq!(
            handler.steps[0],
            AxStepPlan::Update {
                collection: "posts".to_string(),
                fields: vec![AxAssignmentPlan {
                    name: "title".to_string(),
                    value: AxRustExpr::new("input.title"),
                }],
                filters: vec![AxQueryFilterPlan {
                    field: "id".to_string(),
                    op: AxQueryFilterOpPlan::Eq,
                    value: AxRustExpr::new("input.id"),
                }],
            }
        );
    }

    #[test]
    fn lowers_route_name_into_stable_rust_fn() {
        let document = AxBackendDocument::new([AxBackendBlock::Route(AxRoute::new(
            "GET",
            "/api/posts",
            [AxBackendStmt::r#return(AxExpr::ident("posts"))],
        ))]);

        let plan = lower_backend_document(&document).expect("document should lower");

        assert_eq!(plan.handlers[0].rust_fn, "route_get_api_posts");
        assert_eq!(
            plan.handlers[0].kind,
            AxHandlerKind::Route {
                method: "GET".to_string(),
                path: "/api/posts".to_string(),
            }
        );
    }

    #[test]
    fn lowers_runtime_env_access_into_runtime_calls() {
        let document = parse_backend_ax(
            r#"
loader PostsList
  data db_url = Runtime.Env.secret.db_url
  data app_name = Runtime.Env.public.app_name
  return app_name
"#,
        )
        .expect("document should parse");

        let plan = lower_backend_document(&document).expect("document should lower");

        assert_eq!(
            plan.handlers[0].steps[0],
            AxStepPlan::Let {
                binding: "db_url".to_string(),
                value: AxValuePlan::Expr(AxRustExpr::new(
                    r#"runtime.env().secret("db_url")?"#,
                )),
            }
        );
        assert_eq!(
            plan.handlers[0].steps[1],
            AxStepPlan::Let {
                binding: "app_name".to_string(),
                value: AxValuePlan::Expr(AxRustExpr::new(
                    r#"runtime.env().public("app_name")?"#,
                )),
            }
        );
    }
}
