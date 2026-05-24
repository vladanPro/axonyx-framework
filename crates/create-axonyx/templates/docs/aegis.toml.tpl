base_url = "http://127.0.0.1:3000"

[[fast]]
name = "home"
goto = "/"
expect_text = "Axonyx"
expect_not = ["Internal Server Error", "Application error"]

[[fast]]
name = "getting started"
goto = "/getting-started"
expect_text = "Getting Started"
expect_not = ["Internal Server Error", "Application error"]

[[fast]]
name = "reference"
goto = "/reference"
expect_text = "Reference"
expect_not = ["Internal Server Error", "Application error"]
