page Posts

<Container max="xl">
  <Card title="Create post" recipe="hero-card">
    <Copy tone="muted">
      This form posts into route-local actions.ax during dev preview.
    </Copy>
    <form method="post" action={action CreatePost} class="ax-form">
      <input type="text" name="title" placeholder="Post title" class="ax-input" />
      <textarea name="excerpt" placeholder="Short excerpt" class="ax-textarea"></textarea>
      <Button type="submit" tone="primary">Create post</Button>
    </form>
  </Card>
  <Grid cols={3} gap="md" recipe="content-grid">
    <If when={load PostsList}>
      <Each items={load PostsList} as="post">
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
