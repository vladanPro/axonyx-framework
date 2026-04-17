use serde::{Deserialize, Serialize};

use crate::ax_ast::prelude::AxExpr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxQuerySpec {
    pub source: AxQuerySource,
    pub filters: Vec<AxQueryFilter>,
    pub orders: Vec<AxQueryOrder>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl AxQuerySpec {
    pub fn new(source: AxQuerySource) -> Self {
        Self {
            source,
            filters: Vec::new(),
            orders: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    pub fn filter(mut self, filter: AxQueryFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn order(mut self, order: AxQueryOrder) -> Self {
        self.orders.push(order);
        self
    }

    pub fn limit(mut self, value: u32) -> Self {
        self.limit = Some(value);
        self
    }

    pub fn offset(mut self, value: u32) -> Self {
        self.offset = Some(value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxQuerySource {
    Stream { collection: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxQueryFilter {
    pub field: String,
    pub op: AxQueryFilterOp,
    pub value: AxExpr,
}

impl AxQueryFilter {
    pub fn new(field: impl Into<String>, op: AxQueryFilterOp, value: impl Into<AxExpr>) -> Self {
        Self {
            field: field.into(),
            op,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxQueryFilterOp {
    Eq,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxQueryOrder {
    pub field: String,
    pub direction: AxQueryOrderDirection,
}

impl AxQueryOrder {
    pub fn new(field: impl Into<String>, direction: AxQueryOrderDirection) -> Self {
        Self {
            field: field.into(),
            direction,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AxQueryOrderDirection {
    Asc,
    Desc,
}

pub mod prelude {
    pub use super::AxQueryFilter;
    pub use super::AxQueryFilterOp;
    pub use super::AxQueryOrder;
    pub use super::AxQueryOrderDirection;
    pub use super::AxQuerySource;
    pub use super::AxQuerySpec;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_spec_can_model_filters_sorting_and_paging() {
        let query = AxQuerySpec::new(AxQuerySource::Stream {
            collection: "posts".to_string(),
        })
        .filter(AxQueryFilter::new(
            "status",
            AxQueryFilterOp::Eq,
            AxExpr::string("published"),
        ))
        .order(AxQueryOrder::new("created_at", AxQueryOrderDirection::Desc))
        .limit(20)
        .offset(40);

        assert_eq!(
            query,
            AxQuerySpec {
                source: AxQuerySource::Stream {
                    collection: "posts".to_string(),
                },
                filters: vec![AxQueryFilter {
                    field: "status".to_string(),
                    op: AxQueryFilterOp::Eq,
                    value: AxExpr::String("published".to_string()),
                }],
                orders: vec![AxQueryOrder {
                    field: "created_at".to_string(),
                    direction: AxQueryOrderDirection::Desc,
                }],
                limit: Some(20),
                offset: Some(40),
            }
        );
    }
}
