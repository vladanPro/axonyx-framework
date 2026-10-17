use axonyx_runtime::backend::{AxRuntimeError, AxRuntimeResult};
use axonyx_runtime::server::{AxFileRef, AxFileStorage, AxIncomingFile};
use std::sync::Arc;

struct CustomStorage(Arc<dyn AxFileStorage>);

pub fn configure(
    fallback: Arc<dyn AxFileStorage>,
) -> Result<Arc<dyn AxFileStorage>, Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var_os("AXONYX_TEST_STORAGE_FAIL").is_some() {
        return Err(std::io::Error::other("fixture storage configuration failed").into());
    }
    // create_new makes a second startup invocation fail this isolated proof.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open("storage-configured")?;
    Ok(Arc::new(CustomStorage(fallback)))
}

impl AxFileStorage for CustomStorage {
    fn save_file(&self, capability: &str, file: &AxIncomingFile) -> AxRuntimeResult<AxFileRef> {
        if capability != "custom" {
            return self.0.save_file(capability, file);
        }
        if file.bytes != b"source" {
            return Err(AxRuntimeError::invalid_input("image"));
        }
        // This generic startup fixture is not a media decoder or CMS provider.
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open("custom-saved")
            .map_err(|_| AxRuntimeError::message("fixture storage failed"))?;
        std::io::Write::write_all(&mut output, b"canonical")
            .map_err(|_| AxRuntimeError::message("fixture storage failed"))?;
        Ok(AxFileRef {
            id: "custom-id".into(),
            storage: "custom".into(),
            file_name: "canonical.bin".into(),
            content_type: Some("application/octet-stream".into()),
            size: 9,
        })
    }
}
