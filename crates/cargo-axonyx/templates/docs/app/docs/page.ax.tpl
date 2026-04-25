import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page DocsHome

<Head>
  <Title>Docs | Axonyx</Title>
</Head>

<Container max="xl">
  <SectionCard title="Documentation" surface="forged" border="forged">
    <Stack gap="md" align="start">
      <Copy tone="eyebrow">Axonyx Docs</Copy>
      <Copy tone="lead">
        Learn how Axonyx turns readable .ax files into Rust-backed pages,
        Foundry UI surfaces, and optional JavaScript only where behavior needs it.
      </Copy>
      <Button href="/getting-started" variant="primary" surface="forged">
        Start building
      </Button>
    </Stack>
  </SectionCard>

  <ContentGrid cols={3} gap="lg">
    <SectionCard title="Getting Started" surface="brushed" brush="horizontal">
      <Stack gap="md" align="start">
        <Copy>
          Install the tooling, scaffold your first app, and learn the local development loop.
        </Copy>
        <Button href="/getting-started" variant="ghost">Open guide</Button>
      </Stack>
    </SectionCard>

    <SectionCard title="Reference" surface="brushed" brush="vertical">
      <Stack gap="md" align="start">
        <Copy>
          Understand .ax files, routing, public assets, metadata, runtime output, and package imports.
        </Copy>
        <Button href="/reference" variant="ghost">Open reference</Button>
      </Stack>
    </SectionCard>

    <SectionCard title="Examples" surface="brushed" brush="diagonal">
      <Stack gap="md" align="start">
        <Copy>
          Explore practical pages for docs sites, product surfaces, and future CMS-style flows.
        </Copy>
        <Button href="/examples" variant="ghost">Open examples</Button>
      </Stack>
    </SectionCard>
  </ContentGrid>

  <SectionCard title="Axonyx UI" surface="inset">
    <Stack gap="md" align="start">
      <Copy>
        This docs scaffold consumes Foundry components from axonyx-ui through package imports.
      </Copy>
      <Button href="/components/button" surface="forged" border="forged" variant="primary">
        View Button docs
      </Button>
    </Stack>
  </SectionCard>
</Container>
