base_url = "http://127.0.0.1:3000"

[[fast]]
name = "home"
goto = "/"
expect_text = "Axonyx"
expect_not = ["Internal Server Error", "Application error"]

[[fast]]
name = "posts"
goto = "/posts"
expect_text = "Posts"
expect_not = ["Internal Server Error", "Application error"]
