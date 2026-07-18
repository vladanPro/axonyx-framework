query loadPost(slug: String) -> Post[] {
  data posts = Content.Collection("posts")
    where slug = input.slug
    limit 1
  return posts
}
