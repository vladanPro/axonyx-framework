import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page AboutBlog() {
  return ASX {
    <Head><Title>{{APP_NAME}} | About the journal</Title></Head>
    <Container max="md">
      <Stack gap="xl">
        <PageHeader title="A public workbench">
          <Copy slot="eyebrow">About this journal</Copy>
          <Copy tone="lead">Short notes about decisions, experiments, and lessons worth keeping.</Copy>
        </PageHeader>
        <SectionCard title="Publishing is intentionally boring">
          <Copy>Write Markdown in `content/posts`, add frontmatter, and run `cargo ax build --clean`.</Copy>
        </SectionCard>
      </Stack>
    </Container>
  }
}
