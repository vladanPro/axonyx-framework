import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page Contact() -> ASX {
  return {
    <Head><Title>{{APP_NAME}} | Contact</Title></Head>
    <Container max="md">
      <Stack gap="xl">
        <PageHeader title="Let us build the next useful thing.">
          <Copy slot="eyebrow">Contact</Copy>
          <Copy tone="lead">Tell us what you are making, who it helps, and why now matters.</Copy>
        </PageHeader>
        <SectionCard title="Start with an email">
          <Copy>Replace the address below with your real inbox before publishing.</Copy>
          <Button href="mailto:hello@example.com" variant="primary">hello@example.com</Button>
        </SectionCard>
      </Stack>
    </Container>
  }
}
