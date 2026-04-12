use crate::runtime::AxEnv;

pub fn db_url(env: &AxEnv) -> Option<&str> {
    env.secret("db_url")
}
