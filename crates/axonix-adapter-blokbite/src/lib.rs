use axonix_core::Pipeline;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlokbiteBlock {
    #[serde(rename = "blockType")]
    pub block_type: String,
    pub pipeline: Pipeline,
}

pub fn pipeline_to_axonix_block(pipeline: Pipeline) -> BlokbiteBlock {
    BlokbiteBlock {
        block_type: "axonix".to_string(),
        pipeline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axonix_core::parse_pipeline;

    #[test]
    fn wraps_pipeline_in_axonix_block() {
        let pipeline = parse_pipeline("Db.Stream(\"collection\") |> layout.Grid() |> Card()")
            .expect("parse should work");
        let block = pipeline_to_axonix_block(pipeline);
        assert_eq!(block.block_type, "axonix");
        assert_eq!(block.pipeline.stages.len(), 3);
    }
}

