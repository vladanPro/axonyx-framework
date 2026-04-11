[app]
name = "{{APP_NAME}}"
runtime = "native"

[ui]
entry = "app/page.ax"
layout = "app/layout.ax"
render_mode = "server"

[server]
generated_dir = "src/generated"
routes_dir = "routes"
jobs_dir = "jobs"
