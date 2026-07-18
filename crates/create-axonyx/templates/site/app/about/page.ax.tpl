import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page About() {
  return ASX {
    <Head><Title>{{APP_NAME}} | About</Title></Head>
    <Container max="lg">
      <Stack gap="xl">
        <PageHeader title="Small team. Deliberate work.">
          <Copy slot="eyebrow">About {{APP_NAME}}</Copy>
          <Copy tone="lead">We turn ambitious ideas into useful, durable products.</Copy>
        </PageHeader>
        <ContentGrid cols={3} gap="lg">
          <SectionCard title="Clarity"><Badge>01</Badge><Copy>Understand the real problem before choosing the shape.</Copy></SectionCard>
          <SectionCard title="Craft"><Badge>02</Badge><Copy>Sweat the details that make a product calm and trustworthy.</Copy></SectionCard>
          <SectionCard title="Momentum"><Badge>03</Badge><Copy>Ship useful steps early and improve them with evidence.</Copy></SectionCard>
        </ContentGrid>
      </Stack>
    </Container>
  }
}
