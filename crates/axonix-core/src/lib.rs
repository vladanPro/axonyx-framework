use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pipeline {
    pub stages: Vec<Call>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Call {
    pub path: Vec<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("pipeline is empty")]
    EmptyPipeline,
    #[error("invalid stage syntax: {0}")]
    InvalidStage(String),
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
}

