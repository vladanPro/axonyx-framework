pub mod backend;

use axonyx_core::{AxonyxIr, SourceKind, TransformKind, ViewKind};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub use backend::prelude as backend_prelude;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderPlan {
    pub source: String,
    pub layout: LayoutPlan,
    pub view: ViewPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutPlan {
    pub kind: String,
    pub columns: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewPlan {
    pub component: String,
    pub props: serde_json::Value,
}

pub fn execute(ir: &AxonyxIr) -> RenderPlan {
    let source = match &ir.source.kind {
        SourceKind::Collection { name } => name.clone(),
    };

    let mut columns = 1;
    for transform in &ir.transforms {
        match transform.kind {
            TransformKind::Grid { columns: c } => columns = c,
        }
    }

    let component = match &ir.view.kind {
        ViewKind::Card => "Card".to_string(),
        ViewKind::Named { name } => name.clone(),
    };

    RenderPlan {
        source,
        layout: LayoutPlan {
            kind: "grid".to_string(),
            columns,
        },
        view: ViewPlan {
            component,
            props: json!({
                "runtime": "axonyx-runtime-v1",
            }),
        },
    }
}

pub fn execute_json(ir_json: &str) -> Result<RenderPlan, serde_json::Error> {
    let ir: AxonyxIr = serde_json::from_str(ir_json)?;
    Ok(execute(&ir))
}

#[cfg(test)]
mod tests {
    use axonyx_core::compile_pipeline;

    use super::*;

    #[test]
    fn builds_render_plan_from_ir() {
        let ir = compile_pipeline(r#"Db.Stream("posts") |> layout.Grid(3) |> Card()"#)
            .expect("pipeline should compile");
        let plan = execute(&ir);

        assert_eq!(plan.source, "posts");
        assert_eq!(plan.layout.kind, "grid");
        assert_eq!(plan.layout.columns, 3);
        assert_eq!(plan.view.component, "Card");
    }

    #[test]
    fn builds_render_plan_from_json() {
        let ir = compile_pipeline(r#"Db.Stream("users") |> layout.Grid(2) |> ProfileCard()"#)
            .expect("pipeline should compile");
        let ir_json = serde_json::to_string(&ir).expect("serialize");
        let plan = execute_json(&ir_json).expect("json execution should work");

        assert_eq!(plan.source, "users");
        assert_eq!(plan.layout.columns, 2);
        assert_eq!(plan.view.component, "ProfileCard");
    }
}
