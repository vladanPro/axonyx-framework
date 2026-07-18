import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Card } from "@axonyx/ui/foundry/Card.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page BlogPost() {
  data posts = loadPost(params.slug)

  return ASX {
    <Container max="md">
      <Stack gap="xl">
        <Button href="/" variant="ghost" size="sm">Back to all notes</Button>
        <If when={posts}>
          <Each items={posts} as="post">
            <article class="ax-article">
              <Stack gap="lg">
                <div>
                  <Badge tone="warning">{post.category}</Badge>
                  <Copy tone="muted">{post.date} / {post.reading_time}</Copy>
                  <h1>{post.title}</h1>
                  <Copy tone="lead">{post.excerpt}</Copy>
                </div>
                <Html content={post.html} />
              </Stack>
            </article>
          </Each>
          <Else>
            <Card title="Article not found"><Copy>No Markdown entry matched this slug.</Copy></Card>
          </Else>
        </If>
      </Stack>
    </Container>
  }
}
