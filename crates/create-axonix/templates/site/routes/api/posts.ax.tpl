route GET "/api/posts"
  data posts = Db.Stream("posts")
    order created_at desc
    limit 20
  return posts
