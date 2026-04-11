route GET "/api/posts"
  data posts = Db.Stream("posts")
  return posts
