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
    })?;
    let published = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let grant = published.clone();
    registry.register_public_response("media.public", 1024, move |input: ImageRequest| {
        if input.id == "reject" {
            return Err(AxNativeResponseError::Unavailable);
        }
        if input.id != "image" || !grant.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AxNativeResponseError::NotFound);
        }
        AxNativeBinaryResponse::new(AxNativeBinaryType::Png, vec![0, 255, 128, 42])
    })?;
    registry.register("fixture.unpublish", move |_: serde_json::Value| {
        published.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok::<_, ()>(serde_json::json!({ "ok": true }))
    })
}
