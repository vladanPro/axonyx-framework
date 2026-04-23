page Posts

<Container max="xl">
  <Card title="Publish from Axonyx">
    <Copy tone="muted">
      This route can load and mutate without a client-side framework shell.
    </Copy>
    <form method="post" action={action CreatePost} class="ax-form">
      <input type="text" name="title" placeholder="New story title" class="ax-input" />
      <textarea name="excerpt" placeholder="Short story excerpt" class="ax-textarea"></textarea>
      <Button type="submit" tone="primary">Add story</Button>
    </form>
  </Card>
  <Copy tone="muted">Featured writing</Copy>
  <Grid cols={2} gap="lg">
    <If when={load PostsList}>
      <Each items={load PostsList} as="post">
        <Card title={post.title}>
          <Copy>{post.excerpt}</Copy>
        </Card>
      </Each>
      <Else>
        <Card title="No stories yet">
          <Copy>Use the form above to publish the first story and populate this grid.</Copy>
        </Card>
      </Else>
    </If>
  </Grid>
</Container>
