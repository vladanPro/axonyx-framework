use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Content {
    text: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    if std::env::var_os("AXONYX_TEST_NATIVE_FAIL").is_some() {
        return Err(AxNativeError::InvalidRegistration);
    }
    registry.register("content.prepare", |input: Content| {
        if input.text == "reject" {
            return Err("private callback diagnostic");
        }
        Ok(Content { text: input.text.trim().to_owned() })
    })
}
