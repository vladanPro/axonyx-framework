import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Examples() {
  return ASX {
    <Head>
      <Title>Examples | {{APP_NAME}}</Title>
    </Head>

    <Container max="xl">
      <PageHeader title="Examples">
        <Copy slot="eyebrow">Learn by changing small files</Copy>
        <Copy tone="lead">
          These examples are intentionally small. Copy one pattern, change one
          route, and keep the docs app understandable.
        </Copy>
      </PageHeader>

      <ContentGrid cols={2} gap="md">
        <SectionCard title="Landing page">
          <Copy>
            Combine `PageHeader`, `ContentGrid`, `SectionCard`, `Button`, and
            `Copy` for a marketing-style page without a client app.
          </Copy>
        </SectionCard>
        <SectionCard title="Docs module">
          <Copy>
            Add `app/guides/page.ax`, link it from `app/layout.ax`, then run
            `cargo ax check` to confirm the route tree.
          </Copy>
        </SectionCard>
        <SectionCard title="Component showcase">
          <Copy>
            Use `/components` as a living style guide for the pieces this docs
            starter already imports from Foundry UI.
          </Copy>
        </SectionCard>
        <SectionCard title="Deploy check">
          <Copy>
            Keep the dev server running, then run `cargo ax test` to execute the
            starter's fast route checks before deploy.
          </Copy>
        </SectionCard>
      </ContentGrid>
    </Container>
  }
}
