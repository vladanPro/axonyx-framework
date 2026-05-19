action CreatePost
  input:
    title: string
    excerpt: string
    status?: string = "draft"

  insert "posts"
    title: input.title
    excerpt: input.excerpt
    status: input.status

  patch draftStatus = input.status
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
