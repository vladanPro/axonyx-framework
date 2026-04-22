import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page DocsHome

<Head>
  <Title>{{APP_NAME}} | Docs</Title>
</Head>

<Container max="xl">
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
