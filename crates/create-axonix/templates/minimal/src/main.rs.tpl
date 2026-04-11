fn main() {
    let pipeline = r#"Db.Stream("posts") |> layout.Grid(3) |> Card()"#;

    println!("Axonix app '{{APP_NAME}}' is running.");
    println!("Default Algebraic UI pipeline:");
    println!("{pipeline}");
}

