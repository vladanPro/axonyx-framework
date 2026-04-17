use serde::{Deserialize, Serialize};

use crate::ax_ast::prelude::AxExpr;
use crate::ax_query_ast::prelude::{AxQueryFilter, AxQuerySpec};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxBackendDocument {
    pub blocks: Vec<AxBackendBlock>,
}

impl AxBackendDocument {
    pub fn new(blocks: impl IntoIterator<Item = AxBackendBlock>) -> Self {
        Self {
            blocks: blocks.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxBackendBlock {
    Route(AxRoute),
    Loader(AxLoader),
    Action(AxAction),
    Job(AxJob),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxRoute {
    pub method: String,
    pub path: String,
    pub body: Vec<AxBackendStmt>,
}

impl AxRoute {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        body: impl IntoIterator<Item = AxBackendStmt>,
    ) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxLoader {
    pub name: String,
    pub body: Vec<AxBackendStmt>,
}

impl AxLoader {
    pub fn new(name: impl Into<String>, body: impl IntoIterator<Item = AxBackendStmt>) -> Self {
        Self {
            name: name.into(),
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxAction {
    pub name: String,
    pub input: Vec<AxField>,
    pub body: Vec<AxBackendStmt>,
}

impl AxAction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn input(mut self, fields: impl IntoIterator<Item = AxField>) -> Self {
        self.input = fields.into_iter().collect();
        self
    }

    pub fn body(mut self, body: impl IntoIterator<Item = AxBackendStmt>) -> Self {
        self.body = body.into_iter().collect();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxJob {
    pub name: String,
    pub body: Vec<AxBackendStmt>,
}

impl AxJob {
    pub fn new(name: impl Into<String>, body: impl IntoIterator<Item = AxBackendStmt>) -> Self {
        Self {
            name: name.into(),
            body: body.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxBackendStmt {
    Data(AxBackendData),
    Insert(AxMutation),
    Update(AxMutation),
    Delete(AxMutation),
    Revalidate(AxExpr),
    Return(AxReturn),
    Send(AxSend),
}

impl AxBackendStmt {
    pub fn data(name: impl Into<String>, value: impl Into<AxBackendValue>) -> Self {
        Self::Data(AxBackendData::new(name, value))
    }

    pub fn insert(collection: impl Into<String>, fields: impl IntoIterator<Item = AxAssignment>) -> Self {
        Self::Insert(AxMutation::new(collection, fields))
    }

    pub fn update(collection: impl Into<String>, fields: impl IntoIterator<Item = AxAssignment>) -> Self {
        Self::Update(AxMutation::new(collection, fields))
    }

    pub fn delete(collection: impl Into<String>) -> Self {
        Self::Delete(AxMutation::new(collection, []))
    }

    pub fn revalidate(value: impl Into<AxExpr>) -> Self {
        Self::Revalidate(value.into())
    }

    pub fn r#return(value: impl Into<AxReturn>) -> Self {
        Self::Return(value.into())
    }

    pub fn send(target: impl Into<String>, payload: impl Into<AxExpr>) -> Self {
        Self::Send(AxSend::new(target, payload))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxBackendData {
    pub name: String,
    pub value: AxBackendValue,
}

impl AxBackendData {
    pub fn new(name: impl Into<String>, value: impl Into<AxBackendValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxBackendValue {
    Expr(AxExpr),
    Query(AxQuerySpec),
}

impl From<AxExpr> for AxBackendValue {
    fn from(value: AxExpr) -> Self {
        Self::Expr(value)
    }
}

impl From<AxQuerySpec> for AxBackendValue {
    fn from(value: AxQuerySpec) -> Self {
        Self::Query(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxMutation {
    pub collection: String,
    pub fields: Vec<AxAssignment>,
    pub filters: Vec<AxQueryFilter>,
}

impl AxMutation {
    pub fn new(
        collection: impl Into<String>,
        fields: impl IntoIterator<Item = AxAssignment>,
    ) -> Self {
        Self {
            collection: collection.into(),
            fields: fields.into_iter().collect(),
            filters: Vec::new(),
        }
    }

    pub fn filter(mut self, filter: AxQueryFilter) -> Self {
        self.filters.push(filter);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxAssignment {
    pub name: String,
    pub value: AxExpr,
}

impl AxAssignment {
    pub fn new(name: impl Into<String>, value: impl Into<AxExpr>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxField {
    pub name: String,
    pub ty: String,
}

impl AxField {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxReturn {
    Expr(AxExpr),
    Ok,
}

impl From<AxExpr> for AxReturn {
    fn from(value: AxExpr) -> Self {
        Self::Expr(value)
    }
}

impl From<&str> for AxReturn {
    fn from(value: &str) -> Self {
        if value == "ok" {
            Self::Ok
        } else {
            Self::Expr(AxExpr::from(value))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxSend {
    pub target: String,
    pub payload: AxExpr,
}

impl AxSend {
    pub fn new(target: impl Into<String>, payload: impl Into<AxExpr>) -> Self {
        Self {
            target: target.into(),
            payload: payload.into(),
        }
    }
}

pub mod prelude {
    pub use super::AxAction;
    pub use super::AxAssignment;
    pub use super::AxBackendBlock;
    pub use super::AxBackendData;
    pub use super::AxBackendDocument;
    pub use super::AxBackendStmt;
    pub use super::AxBackendValue;
    pub use super::AxField;
    pub use super::AxJob;
    pub use super::AxLoader;
    pub use super::AxMutation;
    pub use super::AxReturn;
    pub use super::AxRoute;
    pub use super::AxSend;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ax_query_ast::prelude::*;

    #[test]
    fn builds_loader_action_and_route_blocks() {
        let document = AxBackendDocument::new([
            AxBackendBlock::Loader(AxLoader::new(
                "PostsList",
                [AxBackendStmt::data(
                    "posts",
                    AxQuerySpec::new(AxQuerySource::Stream {
                        collection: "posts".to_string(),
                    })
                    .filter(AxQueryFilter::new(
                        "status",
                        AxQueryFilterOp::Eq,
                        AxExpr::string("published"),
                    ))
                    .order(AxQueryOrder::new(
                        "created_at",
                        AxQueryOrderDirection::Desc,
                    ))
                    .limit(20),
                ), AxBackendStmt::r#return(AxExpr::ident("posts"))],
            )),
            AxBackendBlock::Action(
                AxAction::new("CreatePost")
                    .input([
                        AxField::new("title", "string"),
                        AxField::new("excerpt", "string"),
                    ])
                    .body([
                        AxBackendStmt::insert(
                            "posts",
                            [
                                AxAssignment::new("title", AxExpr::ident("input").member("title")),
                                AxAssignment::new("excerpt", AxExpr::ident("input").member("excerpt")),
                            ],
                        ),
                        AxBackendStmt::revalidate("/posts"),
                        AxBackendStmt::r#return("ok"),
                    ]),
            ),
            AxBackendBlock::Route(AxRoute::new(
                "GET",
                "/api/posts",
                [AxBackendStmt::data(
                    "posts",
                    AxExpr::call(["Db", "Stream"], [AxExpr::string("posts")]),
                ), AxBackendStmt::r#return(AxExpr::ident("posts"))],
            )),
        ]);

        assert_eq!(document.blocks.len(), 3);

        let AxBackendBlock::Action(action) = &document.blocks[1] else {
            panic!("expected action block");
        };

        assert_eq!(action.name, "CreatePost");
        assert_eq!(action.input.len(), 2);
        assert_eq!(action.body.len(), 3);
    }

    #[test]
    fn job_can_model_send_step() {
        let job = AxJob::new(
            "PublishDailyDigest",
            [
                AxBackendStmt::data(
                    "posts",
                    AxExpr::call(["Query", "PublishedPosts"], []),
                ),
                AxBackendStmt::send("DigestEmail", AxExpr::ident("posts")),
            ],
        );

        assert_eq!(job.name, "PublishDailyDigest");
        assert_eq!(job.body.len(), 2);
    }
}
