base_url = "http://127.0.0.1:3000"
check_links = true

[[fast]]
name = "blog index"
goto = "/"
expect_text = "Notes from the workbench"
expect_not = ["Internal Server Error", "Application error"]

[[fast]]
name = "blog article"
goto = "/blog/hello-axonyx"
expect_text = "Hello from the Axonyx workbench"
expect_not = ["Internal Server Error", "Application error"]

[[fast]]
name = "blog about"
goto = "/about"
expect_text = "A public workbench"
expect_not = ["Internal Server Error", "Application error"]
