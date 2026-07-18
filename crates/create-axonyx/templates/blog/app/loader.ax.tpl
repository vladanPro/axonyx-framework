type Post {
  title: String
  slug: String
  excerpt: String
  date: String
  category: String
  reading_time: String
  html: String
}

query loadPosts() -> Post[] {
  data posts = Content.Collection("posts")
    order date desc
  return posts
}
