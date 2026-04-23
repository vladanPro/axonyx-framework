import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page DocsHome

<Head>
  <Title>{{APP_NAME}} | Docs</Title>
</Head>

<Container max="xl">
  <PageHeader title="{{APP_NAME}} Docs">
    <Copy slot="eyebrow">Documentation</Copy>
    <Copy tone="lead">
      Start with a docs shell that already knows about Axonyx UI imports,
      silver theming, and route-based pages.
    </Copy>
    <Copy>
      Use this starter to explain your framework, product, or internal platform
      without rebuilding the shell patterns from scratch.
    </Copy>
    <a slot="actions" href="/getting-started">Get started</a>
    <a slot="actions" href="/reference">Open reference</a>
  </PageHeader>

  <ContentGrid cols={3} gap="lg">
    <SectionCard title="Getting Started">
      <Copy>
        Explain install, scaffold, runtime sources, and the first Axonyx page
        loop.
      </Copy>
      <a href="/getting-started">Open section</a>
    </SectionCard>
    <SectionCard title="Reference">
      <Copy>
        Document components, layout rules, metadata directives, and runtime
        behavior.
      </Copy>
      <a href="/reference">Open section</a>
    </SectionCard>
    <SectionCard title="Examples">
      <Copy>
        Collect small complete examples that show how Axonyx should feel in
        practice.
      </Copy>
      <a href="/examples">Open section</a>
    </SectionCard>
  </ContentGrid>
</Container>
