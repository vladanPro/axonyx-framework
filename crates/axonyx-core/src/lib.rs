pub mod ax_ast;
pub mod ax_backend_ast;
pub mod ax_backend_codegen;
pub mod ax_backend_lowering;
pub mod ax_backend_parser;
pub mod ax_lowering;
pub mod ax_parser;
pub mod ax_query_ast;
pub mod ax_sql;
pub mod layout;
pub mod pipeline_render;
pub mod reactive;
pub mod ui;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use ax_ast::prelude as ax_ast_prelude;
pub use ax_backend_ast::prelude as ax_backend_ast_prelude;
pub use ax_backend_codegen::prelude as ax_backend_codegen_prelude;
pub use ax_backend_lowering::prelude as ax_backend_lowering_prelude;
pub use ax_backend_parser::prelude as ax_backend_parser_prelude;
pub use ax_lowering::prelude as ax_lowering_prelude;
pub use ax_parser::prelude as ax_parser_prelude;
pub use ax_query_ast::prelude as ax_query_ast_prelude;
pub use ax_sql::prelude as ax_sql_prelude;
pub use axonyx_macros::component;
pub use layout::prelude as layout_prelude;
pub use pipeline_render::prelude as pipeline_prelude;
pub use reactive::prelude;
pub use ui::prelude as ui_prelude;

#[macro_export]
macro_rules! ax {
    ($tag:ident ( $($attrs:tt)* ) [ $($children:tt)* ]) => {
        $crate::reactive::element_with_attrs(
            stringify!($tag),
            $crate::ax!(@attrs [] $($attrs)*),
            $crate::ax!(@children [] $($children)*),
        )
    };
    ($tag:ident [ $($children:tt)* ]) => {
        $crate::reactive::element(
            stringify!($tag),
            $crate::ax!(@children [] $($children)*),
        )
    };
    (@attrs [$($acc:expr,)*]) => {
        vec![$($acc,)*]
    };
    (@attrs [$($acc:expr,)*] , $($rest:tt)*) => {
        $crate::ax!(@attrs [$($acc,)*] $($rest)*)
    };
    (@attrs [$($acc:expr,)*] $name:ident = $value:expr $(, $($rest:tt)*)? ) => {
        $crate::ax!(@attrs [
            $($acc,)*
            $crate::reactive::attr(stringify!($name), $value),
        ] $($($rest)*)? )
    };
    (@children [$($acc:expr,)*]) => {
        vec![$($acc,)*]
    };
    (@children [$($acc:expr,)*] , $($rest:tt)*) => {
        $crate::ax!(@children [$($acc,)*] $($rest)*)
    };
    (@children [$($acc:expr,)*] @node $node:expr $(, $($rest:tt)*)? ) => {
        $crate::ax!(@children [$($acc,)* $node,] $($($rest)*)? )
    };
    (@children [$($acc:expr,)*] $tag:ident [ $($inner:tt)* ] $(, $($rest:tt)*)? ) => {
        $crate::ax!(@children [$($acc,)* $crate::ax!($tag [ $($inner)* ]),] $($($rest)*)? )
    };
    (@children [$($acc:expr,)*] $text:expr $(, $($rest:tt)*)? ) => {
        $crate::ax!(@children [$($acc,)* $crate::reactive::text($text),] $($($rest)*)? )
    };
    (@node $node:expr) => {
        $node
    };
    ($text:expr) => {
        $crate::reactive::text($text)
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pipeline {
    pub stages: Vec<Call>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Call {
    pub path: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxonyxIr {
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub view: View,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub kind: SourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceKind {
    Collection { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transform {
    pub kind: TransformKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransformKind {
    Grid { columns: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct View {
    pub kind: ViewKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViewKind {
    Card,
    Named { name: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("pipeline is empty")]
    EmptyPipeline,
    #[error("invalid stage syntax: {0}")]
    InvalidStage(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("unable to parse pipeline: {0}")]
    ParseFailed(#[from] ParseError),
    #[error("pipeline must contain at least two stages: source and view")]
    TooFewStages,
    #[error("invalid source stage: {0:?}")]
    InvalidSource(Vec<String>),
    #[error("invalid transform stage: {0:?}")]
    InvalidTransform(Vec<String>),
    #[error("invalid view stage: {0:?}")]
    InvalidView(Vec<String>),
    #[error("missing required argument for stage: {0:?}")]
    MissingArgument(Vec<String>),
    #[error("invalid numeric argument in stage: {0:?}")]
    InvalidNumber(Vec<String>),
}

pub fn parse_pipeline(input: &str) -> Result<Pipeline, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyPipeline);
    }

    let mut stages = Vec::new();

    for raw_stage in trimmed.split("|>") {
        let stage = raw_stage.trim();
        if stage.is_empty() {
            return Err(ParseError::InvalidStage(raw_stage.to_string()));
        }
        stages.push(parse_call(stage)?);
    }

    Ok(Pipeline { stages })
}

fn parse_call(stage: &str) -> Result<Call, ParseError> {
    let open_idx = stage
        .find('(')
        .ok_or_else(|| ParseError::InvalidStage(stage.to_string()))?;
    let close_idx = stage
        .rfind(')')
        .ok_or_else(|| ParseError::InvalidStage(stage.to_string()))?;
    if close_idx <= open_idx {
        return Err(ParseError::InvalidStage(stage.to_string()));
    }

    let path_str = stage[..open_idx].trim();
    let args_str = stage[open_idx + 1..close_idx].trim();

    if path_str.is_empty() {
        return Err(ParseError::InvalidStage(stage.to_string()));
    }

    let path: Vec<String> = path_str
        .split('.')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if path.is_empty() {
        return Err(ParseError::InvalidStage(stage.to_string()));
    }

    let args = if args_str.is_empty() {
        Vec::new()
    } else {
        args_str.split(',').map(|x| x.trim().to_string()).collect()
    };

    Ok(Call { path, args })
}

pub fn compile_pipeline(input: &str) -> Result<AxonyxIr, CompileError> {
    let pipeline = parse_pipeline(input)?;
    compile_ir(&pipeline)
}

pub fn compile_ir(pipeline: &Pipeline) -> Result<AxonyxIr, CompileError> {
    if pipeline.stages.len() < 2 {
        return Err(CompileError::TooFewStages);
    }

    let source_call = &pipeline.stages[0];
    let view_call = pipeline.stages.last().expect("len checked");
    let transform_calls = &pipeline.stages[1..pipeline.stages.len() - 1];

    let source = compile_source(source_call)?;
    let mut transforms = Vec::with_capacity(transform_calls.len());
    for call in transform_calls {
        transforms.push(compile_transform(call)?);
    }
    let view = compile_view(view_call)?;

    Ok(AxonyxIr {
        source,
        transforms,
        view,
    })
}

fn compile_source(call: &Call) -> Result<Source, CompileError> {
    let normalized = normalize_path(&call.path);
    let is_collection_source =
        normalized.as_slice() == ["db", "stream"] || normalized.as_slice() == ["from"];
    if !is_collection_source {
        return Err(CompileError::InvalidSource(call.path.clone()));
    }
    let collection = call
        .args
        .first()
        .map(|x| trim_quotes(x))
        .ok_or_else(|| CompileError::MissingArgument(call.path.clone()))?;
    if collection.is_empty() {
        return Err(CompileError::MissingArgument(call.path.clone()));
    }
    Ok(Source {
        kind: SourceKind::Collection { name: collection },
    })
}

fn compile_transform(call: &Call) -> Result<Transform, CompileError> {
    let normalized = normalize_path(&call.path);
    let is_grid = normalized.as_slice() == ["layout", "grid"] || normalized.as_slice() == ["grid"];
    if !is_grid {
        return Err(CompileError::InvalidTransform(call.path.clone()));
    }

    let columns = call
        .args
        .first()
        .map(|x| trim_quotes(x))
        .map(|x| x.parse::<u16>())
        .transpose()
        .map_err(|_| CompileError::InvalidNumber(call.path.clone()))?
        .unwrap_or(3);

    Ok(Transform {
        kind: TransformKind::Grid { columns },
    })
}

fn compile_view(call: &Call) -> Result<View, CompileError> {
    let normalized = normalize_path(&call.path);
    if normalized.as_slice() == ["card"] || normalized.as_slice() == ["view", "card"] {
        return Ok(View {
            kind: ViewKind::Card,
        });
    }

    if let Some(name) = call.path.last() {
        return Ok(View {
            kind: ViewKind::Named { name: name.clone() },
        });
    }

    Err(CompileError::InvalidView(call.path.clone()))
}

fn normalize_path(path: &[String]) -> Vec<String> {
    path.iter()
        .map(|x| x.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
}

fn trim_quotes(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_stage_pipeline() {
        let parsed = parse_pipeline(r#"Db.Stream("posts") |> layout.Grid(3) |> Card()"#)
            .expect("pipeline should parse");
        assert_eq!(parsed.stages.len(), 3);
        assert_eq!(parsed.stages[0].path, vec!["Db", "Stream"]);
        assert_eq!(parsed.stages[1].path, vec!["layout", "Grid"]);
        assert_eq!(parsed.stages[2].path, vec!["Card"]);
    }

    #[test]
    fn compiles_pipeline_to_ir() {
        let parsed = parse_pipeline(r#"Db.Stream("posts") |> layout.Grid(4) |> Card()"#)
            .expect("pipeline should parse");
        let ir = compile_ir(&parsed).expect("pipeline should compile");

        assert_eq!(
            ir.source,
            Source {
                kind: SourceKind::Collection {
                    name: "posts".to_string()
                }
            }
        );
        assert_eq!(
            ir.transforms,
            vec![Transform {
                kind: TransformKind::Grid { columns: 4 }
            }]
        );
        assert_eq!(
            ir.view,
            View {
                kind: ViewKind::Card
            }
        );
    }
}
