route GET "/api/posts"
  data posts = Db.Stream("posts")
    order created_at desc
    limit 20
  return posts

route POST "/api/posts"
  input:
    title: string
    excerpt?: string = ""
    featured?: bool = false

  return json(input.title)
