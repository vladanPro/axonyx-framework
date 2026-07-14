query loadPosts()
  data posts = Content.Collection("posts")
    order date desc
  return posts
