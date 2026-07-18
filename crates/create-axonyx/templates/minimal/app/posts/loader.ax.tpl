export type Post {
  id: String
  title: String
  excerpt: String
  status: String
}

query loadPosts() -> Post[] {
  data posts = db.posts.all()
  return posts
}
