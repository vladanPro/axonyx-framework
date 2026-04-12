action CreatePost
  input:
    title: string
    excerpt: string

  insert "posts"
    title: input.title
    excerpt: input.excerpt

  revalidate "/posts"
  return ok

action PublishPost
  input:
    id: string

  update "posts"
    status: "published"
    where id = input.id

  revalidate "/posts"
  return ok
