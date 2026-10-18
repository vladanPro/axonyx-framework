use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use axonyx_runtime::native_response::{
    AxNativeBinaryResponse, AxNativeBinaryType, AxNativeResponseError,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageRequest {
    id: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register_private_response("media.read", 1024, |context, input: ImageRequest| {
        if context.subject() != "fixture-owner" {
            return Err(AxNativeResponseError::Forbidden);
        }
        match input.id.as_str() {
            "image" => AxNativeBinaryResponse::new(
                AxNativeBinaryType::OctetStream,
                vec![0, 255, 128, 42],
            ),
            "missing" => Err(AxNativeResponseError::NotFound),
            _ => Err(AxNativeResponseError::Unavailable),
        }
    })
}
