import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

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
    <Button slot="actions" href="/getting-started" variant="primary">Get started</Button>
    <Button slot="actions" href="/reference" variant="ghost">Open reference</Button>
  </PageHeader>

  <ContentGrid cols={3} gap="lg">
    <SectionCard title="Getting Started">
      <Stack gap="md" align="start">
        <Copy>
          Explain install, scaffold, runtime sources, and the first Axonyx page
          loop.
        </Copy>
        <Button href="/getting-started" variant="primary">Open section</Button>
      </Stack>
    </SectionCard>
    <SectionCard title="Reference">
      <Stack gap="md" align="start">
        <Copy>
          Document components, layout rules, metadata directives, and runtime
          behavior.
        </Copy>
        <Button href="/reference" variant="ghost">Open section</Button>
      </Stack>
    </SectionCard>
    <SectionCard title="Examples">
      <Stack gap="md" align="start">
        <Copy>
          Collect small complete examples that show how Axonyx should feel in
          practice.
        </Copy>
        <Button href="/examples" variant="ghost">Open section</Button>
      </Stack>
    </SectionCard>
  </ContentGrid>
</Container>
