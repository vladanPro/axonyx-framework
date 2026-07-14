import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ButtonGroup } from "@axonyx/ui/foundry/ButtonGroup.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { FeatureSection } from "@axonyx/ui/foundry/FeatureSection.ax"
import { HeroCard } from "@axonyx/ui/foundry/HeroCard.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page Home() -> ASX {
  return {
    <Head>
      <Title>{{APP_NAME}} | Built for the next move</Title>
    </Head>

    <Container max="xl">
      <Stack gap="xl">
        <HeroCard title="Build something people remember.">
          <Copy slot="eyebrow">{{APP_NAME}}</Copy>
          <Copy tone="lead">
            A focused static site starter for products, studios, teams, and new
            ideas. Fast HTML, expressive Foundry components, and almost no
            browser JavaScript.
          </Copy>
          <ButtonGroup>
            <Button href="/contact" variant="primary">Start a conversation</Button>
            <Button href="/about" variant="ghost">Meet the team</Button>
          </ButtonGroup>
        </HeroCard>

        <ContentGrid cols={3} gap="lg">
          <SectionCard title="Fast by construction">
            <Badge tone="success">Static output</Badge>
            <Copy>Every route becomes deployable HTML with cache-friendly assets.</Copy>
          </SectionCard>
          <SectionCard title="Designed as a system">
            <Badge tone="info">Foundry UI</Badge>
            <Copy>Use consistent layout, typography, surfaces, and theme tokens.</Copy>
          </SectionCard>
          <SectionCard title="Easy to own">
            <Badge tone="warning">Three clear routes</Badge>
            <Copy>No database, API server, jobs, or hidden application machinery.</Copy>
          </SectionCard>
        </ContentGrid>

        <ContentGrid cols={2} gap="lg">
          <FeatureSection title="A clear story, not a component demo">
            <Copy slot="eyebrow">Purposeful starter</Copy>
            <Copy>
              Replace this copy with your product promise, use the about page
              for trust, and turn the contact page into one obvious next step.
            </Copy>
          </FeatureSection>
          <FeatureSection title="Deploy anywhere static files can live">
            <Copy slot="eyebrow">Portable output</Copy>
            <Copy>
              Run `cargo ax build --clean`, then serve `dist/` from a CDN,
              object storage, Docker, Render, or your own server.
            </Copy>
          </FeatureSection>
        </ContentGrid>

        <SectionCard title="Ready for your identity">
          <Copy tone="lead">
            Change the brand mark, choose bronze, silver, or gold, and make the
            first version yours without rebuilding a frontend stack.
          </Copy>
          <Button href="/contact" variant="primary">Make the first move</Button>
        </SectionCard>
      </Stack>
    </Container>
  }
}
