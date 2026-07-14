import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Card } from "@axonyx/ui/foundry/Card.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page BlogHome() -> ASX {
  data posts = loadPosts()

  return {
    <Head><Title>{{APP_NAME}} | Field Notes</Title></Head>
    <Container max="lg">
      <Stack gap="xl">
        <PageHeader title="Notes from the workbench">
          <Copy slot="eyebrow">{{APP_NAME}} Journal</Copy>
          <Copy tone="lead">
            Practical essays about building carefully, learning in public, and
            choosing simple tools that last.
          </Copy>
          <Badge slot="actions" tone="warning">Markdown collection</Badge>
        </PageHeader>

        <ContentGrid cols={2} gap="lg">
          <If when={posts}>
            <Each items={posts} as="post">
              <Card title={post.title} border="forged">
                <Copy tone="muted">{post.date} / {post.category}</Copy>
                <Copy>{post.excerpt}</Copy>
                <a href={"/blog/" + post.slug}>Read article</a>
              </Card>
            </Each>
            <Else>
              <Card title="No notes yet"><Copy>Add a Markdown file under `content/posts`.</Copy></Card>
            </Else>
          </If>
        </ContentGrid>
      </Stack>
    </Container>
  }
}
