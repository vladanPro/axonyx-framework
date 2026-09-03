[app]
name = "{{APP_NAME}}"
runtime = "native"

[ui]
entry = "app/page.asx"
layout = "app/layout.asx"
render_mode = "server"

[server]
generated_dir = "src/generated"
routes_dir = "routes"
jobs_dir = "jobs"
stream_pages = false
max_body_bytes = "1mb"

[db]
migrations = "db/migrations"
pool_max_size = 10
pool_timeout_ms = 5000
query_timeout_ms = 5000
read_retry_attempts = 1
read_retry_backoff_ms = 50
sqlite_busy_timeout_ms = 5000

[modules]
enabled = []
