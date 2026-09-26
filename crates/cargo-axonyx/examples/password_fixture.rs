//! Test-only fixture generator, never an account provisioning endpoint.
use axonyx_runtime::password::AxPassword;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", AxPassword::hash("compiled-smoke-password")?);
    Ok(())
}
