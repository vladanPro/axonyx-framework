import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Reference

<Head>
  <Title>Reference | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <SectionCard title="Reference">
    <Copy tone="lead">
      Document your framework surface here: .ax syntax, metadata directives,
      layout primitives, and dev server behavior.
    </Copy>
  </SectionCard>

  <ContentGrid cols={3} gap="md">
    <SectionCard title="Authoring">
      <Copy>
        Explain components, native HTML tags, title/meta/link/script metadata,
        and the shape of route files.
      </Copy>
    </SectionCard>
    <SectionCard title="Runtime">
      <Copy>
        Describe lowering, preview rendering, and how the runtime package flows
        into generated apps.
      </Copy>
    </SectionCard>
    <SectionCard title="CLI">
      <Copy>
        Document create-axonyx, cargo ax add ..., and cargo ax run dev.
      </Copy>
    </SectionCard>
  </ContentGrid>
</Container>
