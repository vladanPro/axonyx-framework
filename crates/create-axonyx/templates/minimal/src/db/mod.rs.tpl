use axonyx_runtime::backend_prelude::{AxEnv, AxRuntimeResult};

pub fn db_url(env: &AxEnv) -> AxRuntimeResult<String> {
    env.secret("db_url")
}
