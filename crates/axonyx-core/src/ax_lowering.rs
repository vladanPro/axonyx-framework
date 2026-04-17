use std::collections::BTreeMap;

use thiserror::Error;

use crate::ax_ast::prelude::*;
use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxValue {
    Null,
    String(String),
    Number(i64),
    Bool(bool),
    Record(BTreeMap<String, AxValue>),
    List(Vec<AxValue>),
}

impl AxValue {
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, AxValue)>) -> Self {
        let mut map = BTreeMap::new();
        for (key, value) in fields {
            map.insert(key.into(), value);
        }
        Self::Record(map)
    }

    pub fn list(items: impl IntoIterator<Item = AxValue>) -> Self {
        Self::List(items.into_iter().collect())
    }

    pub fn as_string(&self) -> String {
        match self {
            AxValue::Null => String::new(),
            AxValue::String(value) => value.clone(),
            AxValue::Number(value) => value.to_string(),
            AxValue::Bool(value) => value.to_string(),
            AxValue::Record(_) => "[record]".to_string(),
            AxValue::List(_) => "[list]".to_string(),
        }
    }
}

impl From<&str> for AxValue {
    fn from(value: &str) -> Self {
        AxValue::String(value.to_string())
    }
}

impl From<String> for AxValue {
    fn from(value: String) -> Self {
        AxValue::String(value)
    }
}

impl From<i64> for AxValue {
    fn from(value: i64) -> Self {
        AxValue::Number(value)
    }
}

impl From<bool> for AxValue {
    fn from(value: bool) -> Self {
        AxValue::Bool(value)
    }
}

pub trait AxDataResolver {
    fn resolve_call(&self, path: &[String], args: &[AxValue]) -> Option<AxValue>;
}

impl<F> AxDataResolver for F
where
    F: Fn(&[String], &[AxValue]) -> Option<AxValue>,
{
    fn resolve_call(&self, path: &[String], args: &[AxValue]) -> Option<AxValue> {
        self(path, args)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxLowerError {
    #[error("unknown identifier `{name}`")]
    UnknownIdentifier { name: String },
    #[error("unknown member `{property}` on current value")]
    UnknownMember { property: String },
    #[error("unsupported call `{path}`")]
    UnsupportedCall { path: String },
    #[error("`each` requires a list source")]
    EachRequiresList,
}

pub fn lower_document(document: &AxDocument, resolver: &impl AxDataResolver) -> Result<AxNode, AxLowerError> {
    let mut scope = BTreeMap::new();
    let children = lower_statements(&document.page.body, &mut scope, resolver)?;

    Ok(element_with_attrs(
        "main",
        vec![
            attr("data-ax-page", document.page.name.clone()),
            attr("data-ax-root", "page"),
        ],
        children,
    ))
}

fn lower_statements(
    statements: &[AxStatement],
    scope: &mut BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<Vec<AxNode>, AxLowerError> {
    let mut nodes = Vec::new();

    for statement in statements {
        match statement {
            AxStatement::Data(binding) => {
                let value = eval_expr(&binding.value, scope, resolver)?;
                scope.insert(binding.name.clone(), value);
            }
            AxStatement::Each(block) => {
                let source = eval_expr(&block.source, scope, resolver)?;
                let AxValue::List(items) = source else {
                    return Err(AxLowerError::EachRequiresList);
                };

                for item in items {
                    let mut nested = scope.clone();
                    nested.insert(block.binding.clone(), item);
                    nodes.extend(lower_statements(&block.body, &mut nested, resolver)?);
                }
            }
            AxStatement::Component(component) => {
                nodes.push(lower_component(component, scope, resolver)?);
            }
            AxStatement::Pipeline(pipeline) => {
                nodes.push(lower_pipeline(pipeline, scope, resolver)?);
            }
        }
    }

    Ok(nodes)
}

fn lower_component(
    component: &AxComponent,
    scope: &mut BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<AxNode, AxLowerError> {
    let children = match &component.body {
        AxBody::Empty => Vec::new(),
        AxBody::Inline(expr) => vec![text(eval_expr(expr, scope, resolver)?.as_string())],
        AxBody::Block(body) => {
            let mut nested = scope.clone();
            lower_statements(body, &mut nested, resolver)?
        }
    };

    let mut props = eval_props(component, scope, resolver)?;
    let mut attrs = style_attrs(&component.style, scope, resolver)?;

    let node = match component.name.as_str() {
        "Container" => {
            attrs.insert(0, attr("data-layout", "container"));
            attrs.insert(1, attr("data-max-width", prop_string(&mut props, &["max", "max_width"]).unwrap_or_else(|| "xl".to_string())));
            push_remaining_props(&mut attrs, props);
            element_with_attrs("div", attrs, children)
        }
        "Grid" => {
            attrs.insert(0, attr("data-layout", "grid"));
            attrs.insert(1, attr("data-cols", prop_string(&mut props, &["cols"]).unwrap_or_else(|| "1".to_string())));
            attrs.insert(2, attr("data-gap", prop_string(&mut props, &["gap"]).unwrap_or_else(|| "md".to_string())));
            push_remaining_props(&mut attrs, props);
            element_with_attrs("div", attrs, children)
        }
        "Card" => {
            attrs.insert(0, attr("data-ui", "card"));
            let title = prop_string(&mut props, &["title"]);
            push_remaining_props(&mut attrs, props);
            let mut body = Vec::new();
            if let Some(title) = title {
                body.push(element_with_attrs(
                    "header",
                    vec![attr("data-ui", "card-header")],
                    vec![text(title)],
                ));
            }
            body.extend(children);
            element_with_attrs("article", attrs, body)
        }
        "Copy" => {
            attrs.insert(0, attr("data-ui", "copy"));
            let tag = prop_string(&mut props, &["as", "tag"]).unwrap_or_else(|| "p".to_string());
            push_remaining_props(&mut attrs, props);
            element_with_attrs(leak_tag(tag), attrs, children)
        }
        "Button" => {
            attrs.insert(0, attr("data-ui", "button"));
            push_remaining_props(&mut attrs, props);
            element_with_attrs("button", attrs, children)
        }
        other => {
            attrs.insert(0, attr("data-component", other.to_string()));
            push_remaining_props(&mut attrs, props);
            element_with_attrs("div", attrs, children)
        }
    };

    Ok(node)
}

fn lower_pipeline(
    pipeline: &AxPipeline,
    scope: &mut BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<AxNode, AxLowerError> {
    let source = eval_expr(&pipeline.source, scope, resolver)?;
    let source_text = source.as_string();

    let mut attrs = vec![attr("data-ax-pipeline", "true")];
    if let AxValue::List(items) = &source {
        attrs.push(attr("data-items", items.len().to_string()));
    }

    let mut children = vec![text(source_text)];
    for stage in &pipeline.stages {
        match stage {
            AxPipelineStage::Each(each) => {
                children.push(element_with_attrs(
                    "div",
                    vec![attr("data-stage", "each"), attr("data-binding", each.binding.clone())],
                    vec![],
                ));
            }
            AxPipelineStage::Component(component) => {
                let mut nested_scope = scope.clone();
                children.push(lower_component(component, &mut nested_scope, resolver)?);
            }
        }
    }

    Ok(element_with_attrs("section", attrs, children))
}

fn eval_props(
    component: &AxComponent,
    scope: &BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<BTreeMap<String, AxValue>, AxLowerError> {
    let mut props = BTreeMap::new();
    for prop in &component.props {
        props.insert(prop.name.clone(), eval_expr(&prop.value, scope, resolver)?);
    }
    Ok(props)
}

fn style_attrs(
    style: &AxStyle,
    scope: &BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<Vec<Attribute>, AxLowerError> {
    let mut attrs = Vec::new();
    if let Some(recipe) = &style.recipe {
        attrs.push(attr("data-recipe", eval_expr(recipe, scope, resolver)?.as_string()));
    }
    if let Some(class) = &style.class {
        attrs.push(attr("class", eval_expr(class, scope, resolver)?.as_string()));
    }
    Ok(attrs)
}

fn eval_expr(
    expr: &AxExpr,
    scope: &BTreeMap<String, AxValue>,
    resolver: &impl AxDataResolver,
) -> Result<AxValue, AxLowerError> {
    match expr {
        AxExpr::String(value) => Ok(AxValue::String(value.clone())),
        AxExpr::Number(value) => Ok(AxValue::Number(*value)),
        AxExpr::Bool(value) => Ok(AxValue::Bool(*value)),
        AxExpr::Identifier(name) => scope.get(name).cloned().ok_or_else(|| AxLowerError::UnknownIdentifier { name: name.clone() }),
        AxExpr::Member { object, property } => {
            let value = eval_expr(object, scope, resolver)?;
            match value {
                AxValue::Record(fields) => fields.get(property).cloned().ok_or_else(|| AxLowerError::UnknownMember { property: property.clone() }),
                _ => Err(AxLowerError::UnknownMember { property: property.clone() }),
            }
        }
        AxExpr::Call { path, args } => {
            let args = args.iter().map(|arg| eval_expr(arg, scope, resolver)).collect::<Result<Vec<_>, _>>()?;
            resolver.resolve_call(path, &args).ok_or_else(|| AxLowerError::UnsupportedCall { path: path.join(".") })
        }
    }
}

fn prop_string(props: &mut BTreeMap<String, AxValue>, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = props.remove(*name) {
            return Some(value.as_string());
        }
    }
    None
}

fn push_remaining_props(attrs: &mut Vec<Attribute>, props: BTreeMap<String, AxValue>) {
    for (name, value) in props {
        attrs.push(attr_boxed(format!("data-{name}"), value.as_string()));
    }
}

fn attr_boxed(name: String, value: String) -> Attribute {
    Attribute { name: Box::leak(name.into_boxed_str()), value }
}

fn leak_tag(tag: String) -> &'static str {
    Box::leak(tag.into_boxed_str())
}

pub mod prelude {
    pub use super::lower_document;
    pub use super::AxDataResolver;
    pub use super::AxLowerError;
    pub use super::AxValue;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ax_parser::parse_ax;

    #[test]
    fn lowers_indentation_first_page_into_ax_node() {
        let document = parse_ax(
            r#"
page Home
  data posts = Db.Stream("posts")

  Container max: "xl"
    Grid cols: 3, gap: "md", recipe: "screen-center"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
"#,
        )
        .expect("document should parse");

        let resolver = |path: &[String], args: &[AxValue]| -> Option<AxValue> {
            if path == ["Db".to_string(), "Stream".to_string()]
                && args == [AxValue::String("posts".to_string())]
            {
                return Some(AxValue::list([
                    AxValue::record([
                        ("title", AxValue::from("Card A")),
                        ("excerpt", AxValue::from("Intro A")),
                    ]),
                    AxValue::record([
                        ("title", AxValue::from("Card B")),
                        ("excerpt", AxValue::from("Intro B")),
                    ]),
                ]));
            }

            None
        };

        let node = lower_document(&document, &resolver).expect("document should lower");

        assert_eq!(
            node,
            AxNode::Element {
                tag: "main",
                attrs: vec![
                    Attribute { name: "data-ax-page", value: "Home".to_string() },
                    Attribute { name: "data-ax-root", value: "page".to_string() },
                ],
                children: vec![AxNode::Element {
                    tag: "div",
                    attrs: vec![
                        Attribute { name: "data-layout", value: "container".to_string() },
                        Attribute { name: "data-max-width", value: "xl".to_string() },
                    ],
                    children: vec![AxNode::Element {
                        tag: "div",
                        attrs: vec![
                            Attribute { name: "data-layout", value: "grid".to_string() },
                            Attribute { name: "data-cols", value: "3".to_string() },
                            Attribute { name: "data-gap", value: "md".to_string() },
                            Attribute { name: "data-recipe", value: "screen-center".to_string() },
                        ],
                        children: vec![
                            AxNode::Element {
                                tag: "article",
                                attrs: vec![Attribute { name: "data-ui", value: "card".to_string() }],
                                children: vec![
                                    AxNode::Element {
                                        tag: "header",
                                        attrs: vec![Attribute { name: "data-ui", value: "card-header".to_string() }],
                                        children: vec![AxNode::Text("Card A".to_string())],
                                    },
                                    AxNode::Element {
                                        tag: "p",
                                        attrs: vec![Attribute { name: "data-ui", value: "copy".to_string() }],
                                        children: vec![AxNode::Text("Intro A".to_string())],
                                    },
                                ],
                            },
                            AxNode::Element {
                                tag: "article",
                                attrs: vec![Attribute { name: "data-ui", value: "card".to_string() }],
                                children: vec![
                                    AxNode::Element {
                                        tag: "header",
                                        attrs: vec![Attribute { name: "data-ui", value: "card-header".to_string() }],
                                        children: vec![AxNode::Text("Card B".to_string())],
                                    },
                                    AxNode::Element {
                                        tag: "p",
                                        attrs: vec![Attribute { name: "data-ui", value: "copy".to_string() }],
                                        children: vec![AxNode::Text("Intro B".to_string())],
                                    },
                                ],
                            },
                        ],
                    }],
                }],
            }
        );
    }
}
