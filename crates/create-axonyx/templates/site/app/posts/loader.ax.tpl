loader PostsList
  data posts = db.posts.all()
    where status = "published"
    order created_at desc
    limit 6
  return posts
