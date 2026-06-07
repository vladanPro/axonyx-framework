route GET "/api/posts"
  data posts = db.posts.all()
  return posts

route POST "/api/posts"
  input:
    title: string
    excerpt?: string = ""
    featured?: bool = false

  return json(input.title)
