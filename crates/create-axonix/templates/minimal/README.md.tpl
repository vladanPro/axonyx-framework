# {{APP_NAME}}

Generated with `create-axonix`.

## Run

```bash
cargo run
```

## Default pipeline

```axonix
Db.Stream("posts") |> layout.Grid(3) |> Card()
```

