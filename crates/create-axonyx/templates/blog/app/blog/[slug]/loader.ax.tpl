query loadPost(slug: String) {
  data posts = Content.Collection("posts")
    where slug = input.slug
    limit 1
  return posts
}
