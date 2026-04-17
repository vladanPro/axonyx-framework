use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxDocument {
    pub page: AxPage,
}

impl AxDocument {
    pub fn page(name: impl Into<String>, body: impl IntoIterator<Item = AxStatement>) -> Self {
        Self {
            page: AxPage::new(name, body),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxPage {
    pub name: String,
    pub body: Vec<AxStatement>,
}

impl AxPage {
    pub fn new(name: impl Into<String>, body: impl IntoIterator<Item = AxStatement>) -> Self {
        Self {
            name: name.into(),
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxStatement {
    Data(AxDataBinding),
    Each(AxEachBlock),
    Component(AxComponent),
    Pipeline(AxPipeline),
}

impl AxStatement {
    pub fn data(name: impl Into<String>, value: AxExpr) -> Self {
        Self::Data(AxDataBinding::new(name, value))
    }

    pub fn each(binding: impl Into<String>, source: AxExpr, body: impl IntoIterator<Item = AxStatement>) -> Self {
        Self::Each(AxEachBlock::new(binding, source, body))
    }

    pub fn component(component: AxComponent) -> Self {
        Self::Component(component)
    }

    pub fn pipeline(pipeline: AxPipeline) -> Self {
        Self::Pipeline(pipeline)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxDataBinding {
    pub name: String,
    pub value: AxExpr,
}

impl AxDataBinding {
    pub fn new(name: impl Into<String>, value: AxExpr) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxEachBlock {
    pub binding: String,
    pub source: AxExpr,
    pub body: Vec<AxStatement>,
}

impl AxEachBlock {
    pub fn new(
        binding: impl Into<String>,
        source: AxExpr,
        body: impl IntoIterator<Item = AxStatement>,
    ) -> Self {
        Self {
            binding: binding.into(),
            source,
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AxStyle {
    pub recipe: Option<AxExpr>,
    pub class: Option<AxExpr>,
}

impl AxStyle {
    pub fn recipe(mut self, value: impl Into<AxExpr>) -> Self {
        self.recipe = Some(value.into());
        self
    }

    pub fn class(mut self, value: impl Into<AxExpr>) -> Self {
        self.class = Some(value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxComponent {
    pub name: String,
    pub props: Vec<AxProp>,
    pub style: AxStyle,
    pub body: AxBody,
}

impl AxComponent {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            props: Vec::new(),
            style: AxStyle::default(),
            body: AxBody::Empty,
        }
    }

    pub fn prop(mut self, name: impl Into<String>, value: impl Into<AxExpr>) -> Self {
        self.props.push(AxProp::new(name, value));
        self
    }

    pub fn recipe(mut self, value: impl Into<AxExpr>) -> Self {
        self.style = self.style.recipe(value);
        self
    }

    pub fn class(mut self, value: impl Into<AxExpr>) -> Self {
        self.style = self.style.class(value);
        self
    }

    pub fn inline(mut self, value: impl Into<AxExpr>) -> Self {
        self.body = AxBody::Inline(value.into());
        self
    }

    pub fn block(mut self, body: impl IntoIterator<Item = AxStatement>) -> Self {
        self.body = AxBody::Block(body.into_iter().collect());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxProp {
    pub name: String,
    pub value: AxExpr,
}

impl AxProp {
    pub fn new(name: impl Into<String>, value: impl Into<AxExpr>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxBody {
    Empty,
    Inline(AxExpr),
    Block(Vec<AxStatement>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxPipeline {
    pub source: AxExpr,
    pub stages: Vec<AxPipelineStage>,
}

impl AxPipeline {
    pub fn new(source: impl Into<AxExpr>) -> Self {
        Self {
            source: source.into(),
            stages: Vec::new(),
        }
    }

    pub fn stage(mut self, stage: AxPipelineStage) -> Self {
        self.stages.push(stage);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxPipelineStage {
    Component(AxComponent),
    Each(AxEachStage),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxEachStage {
    pub binding: String,
}

impl AxEachStage {
    pub fn new(binding: impl Into<String>) -> Self {
        Self {
            binding: binding.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxExpr {
    String(String),
    Number(i64),
    Bool(bool),
    Identifier(String),
    Member {
        object: Box<AxExpr>,
        property: String,
    },
    Call {
        path: Vec<String>,
        args: Vec<AxExpr>,
    },
}

impl AxExpr {
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn number(value: i64) -> Self {
        Self::Number(value)
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn ident(value: impl Into<String>) -> Self {
        Self::Identifier(value.into())
    }

    pub fn member(self, property: impl Into<String>) -> Self {
        Self::Member {
            object: Box::new(self),
            property: property.into(),
        }
    }

    pub fn call(path: impl IntoIterator<Item = impl Into<String>>, args: impl IntoIterator<Item = AxExpr>) -> Self {
        Self::Call {
            path: path.into_iter().map(Into::into).collect(),
            args: args.into_iter().collect(),
        }
    }
}

impl From<&str> for AxExpr {
    fn from(value: &str) -> Self {
        AxExpr::String(value.to_string())
    }
}

impl From<String> for AxExpr {
    fn from(value: String) -> Self {
        AxExpr::String(value)
    }
}

impl From<i64> for AxExpr {
    fn from(value: i64) -> Self {
        AxExpr::Number(value)
    }
}

impl From<bool> for AxExpr {
    fn from(value: bool) -> Self {
        AxExpr::Bool(value)
    }
}

pub mod prelude {
    pub use super::AxBody;
    pub use super::AxComponent;
    pub use super::AxDataBinding;
    pub use super::AxDocument;
    pub use super::AxEachBlock;
    pub use super::AxEachStage;
    pub use super::AxExpr;
    pub use super::AxPage;
    pub use super::AxPipeline;
    pub use super::AxPipelineStage;
    pub use super::AxProp;
    pub use super::AxStatement;
    pub use super::AxStyle;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ast_for_indentation_first_page() {
        let posts = AxExpr::ident("posts");
        let post = AxExpr::ident("post");

        let document = AxDocument::page(
            "Home",
            [
                AxStatement::data(
                    "posts",
                    AxExpr::call(["Db", "Stream"], [AxExpr::string("posts")]),
                ),
                AxStatement::component(
                    AxComponent::new("Container")
                        .prop("max", "xl")
                        .block([AxStatement::component(
                            AxComponent::new("Grid")
                                .prop("cols", 3_i64)
                                .prop("gap", "md")
                                .block([AxStatement::each(
                                    "post",
                                    posts.clone(),
                                    [AxStatement::component(
                                        AxComponent::new("Card")
                                            .prop("title", post.clone().member("title"))
                                            .block([AxStatement::component(
                                                AxComponent::new("Copy")
                                                    .inline(post.clone().member("excerpt")),
                                            )]),
                                    )],
                                )]),
                        )]),
                ),
            ],
        );

        assert_eq!(document.page.name, "Home");
        assert_eq!(document.page.body.len(), 2);
        assert_eq!(
            document.page.body[0],
            AxStatement::Data(AxDataBinding::new(
                "posts",
                AxExpr::Call {
                    path: vec!["Db".to_string(), "Stream".to_string()],
                    args: vec![AxExpr::String("posts".to_string())],
                }
            ))
        );
    }

    #[test]
    fn component_keeps_style_layers_separate_from_semantic_props() {
        let node = AxComponent::new("Button")
            .prop("tone", "primary")
            .prop("size", "lg")
            .recipe("hero-cta")
            .class("w-full")
            .inline("Launch");

        assert_eq!(
            node,
            AxComponent {
                name: "Button".to_string(),
                props: vec![
                    AxProp::new("tone", "primary"),
                    AxProp::new("size", "lg"),
                ],
                style: AxStyle {
                    recipe: Some(AxExpr::String("hero-cta".to_string())),
                    class: Some(AxExpr::String("w-full".to_string())),
                },
                body: AxBody::Inline(AxExpr::String("Launch".to_string())),
            }
        );
    }

    #[test]
    fn pipeline_ast_can_represent_each_and_component_stages() {
        let pipeline = AxPipeline::new(AxExpr::call(["Db", "Stream"], [AxExpr::string("posts")]))
            .stage(AxPipelineStage::Component(
                AxComponent::new("Grid")
                    .prop("cols", 3_i64)
                    .prop("gap", "md"),
            ))
            .stage(AxPipelineStage::Each(AxEachStage::new("post")))
            .stage(AxPipelineStage::Component(
                AxComponent::new("Card")
                    .prop("title", AxExpr::ident("post").member("title")),
            ));

        assert_eq!(pipeline.stages.len(), 3);
        assert_eq!(
            pipeline.stages[1],
            AxPipelineStage::Each(AxEachStage {
                binding: "post".to_string(),
            })
        );
    }
}
