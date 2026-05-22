route GET "/api/posts"
  data posts = Db.Stream("posts")
  return posts

route POST "/api/posts"
  input:
    title: string
    excerpt?: string = ""
    featured?: bool = false

  return json(input.title)
