use thiserror::Error;

use crate::ax_backend_lowering::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxSqlDialect {
    Postgres,
    MySql,
    Sqlite,
}

impl AxSqlDialect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }

    fn quote_ident(&self, ident: &str) -> String {
        match self {
            Self::Postgres | Self::Sqlite => format!("\"{ident}\""),
            Self::MySql => format!("`{ident}`"),
        }
    }

    fn placeholder(&self, index: usize) -> String {
        match self {
            Self::Postgres => format!("${index}"),
            Self::MySql | Self::Sqlite => "?".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxSqlQuery {
    pub sql: String,
    pub params: Vec<AxSqlParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxSqlMutation {
    pub sql: String,
    pub params: Vec<AxSqlParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxSqlParam {
    pub index: usize,
    pub value: AxRustExpr,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxSqlCompileError {
    #[error("query collection cannot be empty")]
    EmptyCollection,
    #[error("identifier `{ident}` contains unsupported characters")]
    InvalidIdentifier { ident: String },
    #[error("unsupported query filter operator")]
    UnsupportedFilterOperator,
    #[error("mutation must contain at least one field")]
    EmptyMutationFields,
}

pub fn compile_query_plan_to_sql(
    query: &AxQueryPlan,
    dialect: AxSqlDialect,
) -> Result<AxSqlQuery, AxSqlCompileError> {
    let collection = match &query.source {
        AxQuerySourcePlan::Stream { collection } => collection,
    };
    validate_ident(collection)?;

    let mut sql = format!("select * from {}", dialect.quote_ident(collection));
    let mut params = Vec::new();

    if !query.filters.is_empty() {
        let mut clauses = Vec::with_capacity(query.filters.len());

        for filter in &query.filters {
            validate_ident(&filter.field)?;
            let placeholder = dialect.placeholder(params.len() + 1);
            let op = match filter.op {
                AxQueryFilterOpPlan::Eq => "=",
            };

            clauses.push(format!(
                "{} {} {}",
                dialect.quote_ident(&filter.field),
                op,
                placeholder
            ));
            params.push(AxSqlParam {
                index: params.len() + 1,
                value: filter.value.clone(),
            });
        }

        sql.push_str(" where ");
        sql.push_str(&clauses.join(" and "));
    }

    if !query.orders.is_empty() {
        let mut clauses = Vec::with_capacity(query.orders.len());

        for order in &query.orders {
            validate_ident(&order.field)?;
            clauses.push(format!(
                "{} {}",
                dialect.quote_ident(&order.field),
                order_direction_name(order.direction)
            ));
        }

        sql.push_str(" order by ");
        sql.push_str(&clauses.join(", "));
    }

    if let Some(limit) = query.limit {
        sql.push_str(&format!(" limit {limit}"));
    }

    if let Some(offset) = query.offset {
        sql.push_str(&format!(" offset {offset}"));
    }

    Ok(AxSqlQuery { sql, params })
}

pub fn compile_insert_plan_to_sql(
    collection: &str,
    fields: &[AxAssignmentPlan],
    dialect: AxSqlDialect,
) -> Result<AxSqlMutation, AxSqlCompileError> {
    validate_ident(collection)?;
    if fields.is_empty() {
        return Err(AxSqlCompileError::EmptyMutationFields);
    }

    let mut columns = Vec::with_capacity(fields.len());
    let mut placeholders = Vec::with_capacity(fields.len());
    let mut params = Vec::with_capacity(fields.len());

    for field in fields {
        validate_ident(&field.name)?;
        columns.push(dialect.quote_ident(&field.name));
        placeholders.push(dialect.placeholder(params.len() + 1));
        params.push(AxSqlParam {
            index: params.len() + 1,
            value: field.value.clone(),
        });
    }

    Ok(AxSqlMutation {
        sql: format!(
            "insert into {} ({}) values ({})",
            dialect.quote_ident(collection),
            columns.join(", "),
            placeholders.join(", ")
        ),
        params,
    })
}

pub fn compile_update_plan_to_sql(
    collection: &str,
    fields: &[AxAssignmentPlan],
    dialect: AxSqlDialect,
) -> Result<AxSqlMutation, AxSqlCompileError> {
    validate_ident(collection)?;
    if fields.is_empty() {
        return Err(AxSqlCompileError::EmptyMutationFields);
    }

    let mut assignments = Vec::with_capacity(fields.len());
    let mut params = Vec::with_capacity(fields.len());

    for field in fields {
        validate_ident(&field.name)?;
        assignments.push(format!(
            "{} = {}",
            dialect.quote_ident(&field.name),
            dialect.placeholder(params.len() + 1)
        ));
        params.push(AxSqlParam {
            index: params.len() + 1,
            value: field.value.clone(),
        });
    }

    Ok(AxSqlMutation {
        sql: format!(
            "update {} set {}",
            dialect.quote_ident(collection),
            assignments.join(", ")
        ),
        params,
    })
}

fn validate_ident(ident: &str) -> Result<(), AxSqlCompileError> {
    let trimmed = ident.trim();
    if trimmed.is_empty() {
        return Err(AxSqlCompileError::EmptyCollection);
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Ok(())
    } else {
        Err(AxSqlCompileError::InvalidIdentifier {
            ident: ident.to_string(),
        })
    }
}

fn order_direction_name(direction: AxQueryOrderDirectionPlan) -> &'static str {
    match direction {
        AxQueryOrderDirectionPlan::Asc => "asc",
        AxQueryOrderDirectionPlan::Desc => "desc",
    }
}

pub mod prelude {
    pub use super::compile_insert_plan_to_sql;
    pub use super::compile_query_plan_to_sql;
    pub use super::compile_update_plan_to_sql;
    pub use super::AxSqlCompileError;
    pub use super::AxSqlDialect;
    pub use super::AxSqlMutation;
    pub use super::AxSqlParam;
    pub use super::AxSqlQuery;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_postgres_query_plan_into_sql() {
        let query = AxQueryPlan {
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
        };

        let sql = compile_query_plan_to_sql(&query, AxSqlDialect::Postgres)
            .expect("query should compile");

        assert_eq!(
            sql.sql,
            r#"select * from "posts" where "status" = $1 order by "created_at" desc limit 20 offset 40"#
        );
        assert_eq!(
            sql.params,
            vec![AxSqlParam {
                index: 1,
                value: AxRustExpr::new(r#""published".to_string()"#),
            }]
        );
    }

    #[test]
    fn compiles_mysql_query_plan_into_sql() {
        let query = AxQueryPlan {
            source: AxQuerySourcePlan::Stream {
                collection: "posts".to_string(),
            },
            filters: vec![
                AxQueryFilterPlan {
                    field: "status".to_string(),
                    op: AxQueryFilterOpPlan::Eq,
                    value: AxRustExpr::new(r#""published".to_string()"#),
                },
                AxQueryFilterPlan {
                    field: "featured".to_string(),
                    op: AxQueryFilterOpPlan::Eq,
                    value: AxRustExpr::new("true"),
                },
            ],
            orders: vec![AxQueryOrderPlan {
                field: "created_at".to_string(),
                direction: AxQueryOrderDirectionPlan::Desc,
            }],
            limit: Some(12),
            offset: None,
        };

        let sql =
            compile_query_plan_to_sql(&query, AxSqlDialect::MySql).expect("query should compile");

        assert_eq!(
            sql.sql,
            "select * from `posts` where `status` = ? and `featured` = ? order by `created_at` desc limit 12"
        );
        assert_eq!(sql.params.len(), 2);
        assert_eq!(sql.params[0].index, 1);
        assert_eq!(sql.params[1].index, 2);
    }

    #[test]
    fn compiles_sqlite_query_plan_into_sql() {
        let query = AxQueryPlan {
            source: AxQuerySourcePlan::Stream {
                collection: "posts".to_string(),
            },
            filters: Vec::new(),
            orders: vec![AxQueryOrderPlan {
                field: "created_at".to_string(),
                direction: AxQueryOrderDirectionPlan::Asc,
            }],
            limit: Some(5),
            offset: Some(10),
        };

        let sql =
            compile_query_plan_to_sql(&query, AxSqlDialect::Sqlite).expect("query should compile");

        assert_eq!(
            sql.sql,
            r#"select * from "posts" order by "created_at" asc limit 5 offset 10"#
        );
        assert!(sql.params.is_empty());
    }

    #[test]
    fn rejects_invalid_identifiers() {
        let query = AxQueryPlan {
            source: AxQuerySourcePlan::Stream {
                collection: "blog-posts".to_string(),
            },
            filters: Vec::new(),
            orders: Vec::new(),
            limit: None,
            offset: None,
        };

        let error = compile_query_plan_to_sql(&query, AxSqlDialect::Postgres)
            .expect_err("invalid identifier should fail");

        assert_eq!(
            error,
            AxSqlCompileError::InvalidIdentifier {
                ident: "blog-posts".to_string(),
            }
        );
    }

    #[test]
    fn compiles_postgres_insert_plan_into_sql() {
        let mutation = compile_insert_plan_to_sql(
            "posts",
            &[
                AxAssignmentPlan {
                    name: "title".to_string(),
                    value: AxRustExpr::new("input.title"),
                },
                AxAssignmentPlan {
                    name: "featured".to_string(),
                    value: AxRustExpr::new("input.featured"),
                },
            ],
            AxSqlDialect::Postgres,
        )
        .expect("insert should compile");

        assert_eq!(
            mutation.sql,
            r#"insert into "posts" ("title", "featured") values ($1, $2)"#
        );
        assert_eq!(mutation.params.len(), 2);
        assert_eq!(mutation.params[0].index, 1);
        assert_eq!(mutation.params[1].index, 2);
    }

    #[test]
    fn compiles_mysql_update_plan_into_sql() {
        let mutation = compile_update_plan_to_sql(
            "posts",
            &[
                AxAssignmentPlan {
                    name: "title".to_string(),
                    value: AxRustExpr::new("input.title"),
                },
                AxAssignmentPlan {
                    name: "featured".to_string(),
                    value: AxRustExpr::new("input.featured"),
                },
            ],
            AxSqlDialect::MySql,
        )
        .expect("update should compile");

        assert_eq!(
            mutation.sql,
            "update `posts` set `title` = ?, `featured` = ?"
        );
        assert_eq!(mutation.params.len(), 2);
    }

    #[test]
    fn rejects_empty_mutation_fields() {
        let error = compile_insert_plan_to_sql("posts", &[], AxSqlDialect::Sqlite)
            .expect_err("empty insert should fail");

        assert_eq!(error, AxSqlCompileError::EmptyMutationFields);
    }
}
