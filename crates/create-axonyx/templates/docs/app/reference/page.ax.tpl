import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { DocsSection } from "@axonyx/ui/foundry/DocsSection.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Reference

<Head>
  <Title>Reference | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <PageHeader title="Reference">
    <Copy slot="eyebrow">Surface Area</Copy>
    <Copy tone="lead">
      Document your framework surface here: .ax syntax, metadata directives,
      layout primitives, and dev server behavior.
    </Copy>
    <Copy>
      Keep the contract explicit so new users can see what belongs in authoring,
      what belongs in runtime, and what the CLI handles for them.
    </Copy>
  </PageHeader>

  <ContentGrid cols={3} gap="md">
    <SectionCard title="Authoring">
      <Copy>
        Explain components, native HTML tags, title/meta/link/script metadata,
        and the shape of route files.
      </Copy>
    </SectionCard>
    <DocsSection title="Runtime">
      <Copy slot="eyebrow">Execution</Copy>
      <Copy>
        Describe lowering, preview rendering, and how the runtime package flows
        into generated apps.
      </Copy>
      <Copy slot="aside" tone="muted">Call out what is compile-time, what is server-rendered, and what stays optional on the client.</Copy>
      <a slot="actions" href="/getting-started">Connect it to setup</a>
    </DocsSection>
    <SectionCard title="CLI">
      <Copy>
        Document create-axonyx, cargo ax add ..., and cargo ax run dev.
      </Copy>
    </SectionCard>
  </ContentGrid>
</Container>
