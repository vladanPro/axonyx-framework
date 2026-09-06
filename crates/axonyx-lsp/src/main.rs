use std::io;

fn main() {
    if let Err(error) = axonyx_lsp::run_server(io::stdin().lock(), io::stdout().lock()) {
        eprintln!("axonyx-lsp stopped: {error}");
        std::process::exit(1);
    }
}
