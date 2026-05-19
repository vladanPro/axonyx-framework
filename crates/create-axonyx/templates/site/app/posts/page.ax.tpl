page Posts

page state draftStatus: String = "ready"

<Container max="xl">
  <Card title="Publish from Axonyx">
    <Copy tone="muted">
      This route uses ActionForm, typed action inputs, and the small action
      runtime without a client-side framework shell.
    </Copy>
    <ActionForm name="CreatePost">
      <input type="text" name="title" placeholder="New story title" class="ax-input" />
      <textarea name="excerpt" placeholder="Short story excerpt" class="ax-textarea">
      </textarea>
      <select name="status" class="ax-select">
        <option value="draft">draft</option>
        <option value="published">published</option>
      </select>
      <Button type="submit" tone="primary">Add story</Button>
      <ActionStatus state="pending">Saving story...</ActionStatus>
      <ActionStatus state="complete">Story saved.</ActionStatus>
      <ActionStatus state="error">Story could not be saved.</ActionStatus>
    </ActionForm>
    <Copy tone="muted">Last submitted status:</Copy>
    <strong bind:text={draftStatus}>{draftStatus}</strong>
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
