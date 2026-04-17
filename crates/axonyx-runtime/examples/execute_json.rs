use axonyx_runtime::execute_json;

fn main() {
    let ir_json = include_str!("../../../examples/ir.posts.card.json");
    let plan = execute_json(ir_json).expect("valid IR JSON");
    println!(
        "source={} layout={} columns={} view={}",
        plan.source, plan.layout.kind, plan.layout.columns, plan.view.component
    );
}

