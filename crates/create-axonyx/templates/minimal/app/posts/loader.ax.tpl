query loadPosts() -> Post[]
  data posts = db.posts.all()
  return posts
