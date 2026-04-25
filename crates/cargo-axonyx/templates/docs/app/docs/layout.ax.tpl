import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page DocsLayout

<Container max="xl">
  <ContentGrid cols={4} gap="lg">
    <SectionCard title="Docs" surface="brushed" brush="vertical" border="forged">
      <Stack gap="md" align="start">
        <Copy tone="eyebrow">Axonyx</Copy>
        <Button href="/docs" variant="primary" surface="forged">Overview</Button>
        <Button href="/docs/getting-started" variant="ghost">Getting Started</Button>
        <Button href="/docs/reference" variant="ghost">Reference</Button>
        <Button href="/docs/examples" variant="ghost">Examples</Button>
      </Stack>
    </SectionCard>

    <SectionCard title="Content" surface="inset">
      <Slot />
    </SectionCard>
  </ContentGrid>
</Container>
