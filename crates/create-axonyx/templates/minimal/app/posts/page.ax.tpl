page Posts

type Post {
  id: String
  title: String
  excerpt: String
  status: String
}

page state draftStatus: String = "ready"
data posts: List<Post> = loadPosts()

<Container max="xl">
  <Card title="Create post" recipe="hero-card">
    <Copy tone="muted">
      This form uses ActionForm, typed inputs, and the small action runtime
      during dev preview.
    </Copy>
    <ActionForm name="CreatePost">
      <input type="text" name="title" placeholder="Post title" class="ax-input" />
      <textarea name="excerpt" placeholder="Short excerpt" class="ax-textarea">
      </textarea>
      <select name="status" class="ax-select">
        <option value="draft">draft</option>
        <option value="published">published</option>
      </select>
      <Button type="submit" tone="primary">Create post</Button>
      <ActionStatus state="pending">Saving post...</ActionStatus>
      <ActionStatus state="complete">Post saved.</ActionStatus>
      <ActionStatus state="error">Post could not be saved.</ActionStatus>
    </ActionForm>
    <Copy tone="muted">Last submitted status:</Copy>
    <strong bind:text={draftStatus}>{draftStatus}</strong>
  </Card>
  <Grid cols={3} gap="md" recipe="content-grid">
    <If when={posts}>
      <Each items={posts} as="post">
        <Card title={post.title}>
          <Copy>{post.excerpt}</Copy>
          <Button tone="primary">Read more</Button>
        </Card>
      </Each>
      <Else>
        <Card title="No posts yet">
          <Copy>Add your first post to watch route-local actions and rendering click together.</Copy>
        </Card>
      </Else>
    </If>
  </Grid>
</Container>
