loader PostsList
  data posts = Db.Stream("posts")
    where status = "published"
    order created_at desc
    limit 6
  return posts
