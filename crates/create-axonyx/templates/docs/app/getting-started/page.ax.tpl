import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page GettingStarted

<Head>
  <Title>Getting Started | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <SectionCard title="Getting Started">
    <Copy tone="lead">
      Start by generating the app, running cargo ax run dev, and editing
      app/page.ax or a nested route page.
    </Copy>
    <ContentGrid cols={2} gap="md">
      <SectionCard title="Scaffold">
        <Copy>
          create-axonyx already wires the runtime, UI vendor snapshot, and
          silver theme starter shell for this template.
        </Copy>
      </SectionCard>
      <SectionCard title="Preview">
        <Copy>
          Use cargo run for quick preview generation or cargo ax run dev for
          route-aware local serving.
        </Copy>
      </SectionCard>
      <SectionCard title="Edit">
        <Copy>
          Start with app/layout.ax for the shell, then move through app/page.ax
          and nested route pages as the docs grow.
        </Copy>
      </SectionCard>
      <SectionCard title="Expand">
        <Copy>
          Add routes/ or jobs/ later if your docs app needs APIs, ingestion, or
          scheduled work.
        </Copy>
      </SectionCard>
    </ContentGrid>
  </SectionCard>
</Container>
