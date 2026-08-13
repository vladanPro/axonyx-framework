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

[modules]
enabled = []
