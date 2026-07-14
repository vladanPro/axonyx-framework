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
stream_pages = false
max_body_bytes = "1mb"

[modules]
enabled = ["ui"]

[content.collections.posts]
path = "content/posts"
extensions = ["md", "mdx"]

[prerender.collections.posts]
route = "/blog/:slug"
param = "slug"
field = "slug"
