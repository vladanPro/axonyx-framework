import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Card } from "@axonyx/ui/foundry/Card.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page Posts

type Post {
  id: String
  title: String
  excerpt: String
  status: String
}

page state draftStatus: String = "ready"
data posts: List<Post> = loadPosts()

<Stack gap="xl">
  <PageHeader title="Posts demo">
    <Copy slot="eyebrow">Action + Loader</Copy>
    <Copy tone="lead">
      This route shows the first Axonyx full-stack loop: typed action inputs,
      state patches, loader data, and server-rendered cards.
    </Copy>
    <Button slot="actions" href="/" variant="ghost">Back home</Button>
  </PageHeader>

  <ContentGrid cols={2} gap="lg">
    <Card title="Publish from Axonyx">
      <Copy tone="muted">
        The form posts to the route-local `CreatePost` action and keeps the
        browser update path small through the action runtime.
      </Copy>
      <ActionForm name="CreatePost">
        <input type="text" name="title" placeholder="New story title" class="ax-input" />
        <textarea name="excerpt" placeholder="Short story excerpt" class="ax-textarea">
        </textarea>
        <select name="status" class="ax-select">
          <option value="draft">draft</option>
          <option value="published">published</option>
        </select>
        <button type="submit" class="ax-button" data-variant="primary">Add story</button>
        <ActionStatus state="pending">Saving story...</ActionStatus>
        <ActionStatus state="complete">Story saved.</ActionStatus>
        <ActionStatus state="error">Story could not be saved.</ActionStatus>
      </ActionForm>
    </Card>
    <Card title="State bridge">
      <Copy>
        The text below is bound to a page state signal. It gives the generated
        app an immediate state manifest under `dist/_ax/state/manifest.json`.
      </Copy>
      <Badge>Last submitted status</Badge>
      <strong bind:text={draftStatus}>{draftStatus}</strong>
    </Card>
  </ContentGrid>

  <Copy tone="eyebrow">Featured writing</Copy>
  <ContentGrid cols={2} gap="lg">
    <If when={posts}>
      <Each items={posts} as="post">
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
  </ContentGrid>
</Stack>
