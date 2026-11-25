action CreatePost(title: String, excerpt: String, status: String = "draft") {
  require input.title != "" && input.title.length <= 160 else invalid({ title: "Enter a title of at most 160 characters." })
  data id = Uuid.new()
  db.posts.insert({ id: id, title: input.title, excerpt: input.excerpt, status: input.status })
  patch draftStatus = input.status
  revalidate "/posts"
  return ok()
}

action PublishPost(id: String) {
  db.posts.where({ id: input.id }).update({ status: "published" })
  revalidate "/posts"
  return ok()
}
