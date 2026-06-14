import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ButtonGroup } from "@axonyx/ui/foundry/ButtonGroup.ax"
import { CommandList } from "@axonyx/ui/foundry/CommandList.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { DocsCodeBlock } from "@axonyx/ui/foundry/DocsCodeBlock.ax"
import { FeatureSection } from "@axonyx/ui/foundry/FeatureSection.ax"
import { HeroCard } from "@axonyx/ui/foundry/HeroCard.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page Home() -> ASX {

return {
  <Head>
    <Title>{{APP_NAME}} | Axonyx site starter</Title>
  </Head>

  <Stack gap="xl">
    <HeroCard title="{{APP_NAME}}">
      <Copy slot="eyebrow">Axonyx site starter</Copy>
      <img
        src="/brand-mark.svg"
        alt="{{APP_NAME}} brand mark"
        width={92}
        height={92}
      />
      <Copy tone="lead">
        A polished Foundry starter for product pages, launch sites, framework
        docs, and content-first apps that should not begin life as a heavy React
        shell.
      </Copy>
      <Copy>
        Edit JSX-like `.ax` routes, keep runtime behavior in Rust, and let
        `@axonyx/ui` provide the first visual contract.
      </Copy>
      <ButtonGroup>
        <Button href="/posts" variant="primary">Open posts demo</Button>
        <Button href="#start" variant="ghost">View commands</Button>
      </ButtonGroup>
    </HeroCard>

    <ContentGrid cols={3} gap="lg">
      <SectionCard title="Rust runtime">
        <Badge>Tokio-ready</Badge>
        <Copy>Axonyx serves route-aware pages without a Node server.</Copy>
      </SectionCard>
      <SectionCard title="Foundry UI">
        <Badge>Bronze / Silver / Gold</Badge>
        <Copy>Theme tokens and components are already wired through `use "@axonyx/ui"`.</Copy>
      </SectionCard>
      <SectionCard title="Minimal JS">
        <Badge>Only when needed</Badge>
        <Copy>Interactive pieces ship small package scripts instead of a full app shell.</Copy>
      </SectionCard>
    </ContentGrid>

    <PageHeader title="A starter that already feels like a site">
      <Copy slot="eyebrow">First edit</Copy>
      <Copy tone="lead">
        The generated folder is meant to be changed immediately: swap the hero,
        add sections, keep routes obvious, and grow toward CMS or docs when the
        project needs it.
      </Copy>
    </PageHeader>

    <ContentGrid cols={3} gap="lg">
      <SectionCard title="Route-first pages">
        <Copy>
          `app/page.ax` owns this homepage and `app/posts/page.ax` owns the demo
          content route. Add folders under `app/` to create more pages.
        </Copy>
        <Badge tone="info">app/**/page.ax</Badge>
      </SectionCard>
      <SectionCard title="Foundry components">
        <Copy>
          The template imports cards, stats, buttons, forms, and layout primitives
          from the published Axonyx UI package.
        </Copy>
        <Badge tone="success">@axonyx/ui</Badge>
      </SectionCard>
      <SectionCard title="Backend shape included">
        <Copy>
          The posts route includes a loader, action, and typed API route so the
          starter shows more than static HTML.
        </Copy>
        <Badge tone="warning">routes + actions</Badge>
      </SectionCard>
    </ContentGrid>

    <ContentGrid cols={2} gap="lg">
      <FeatureSection title="Build presentation pages without losing the backend">
        <Copy slot="eyebrow">Full-stack direction</Copy>
        <Copy>
          Axonyx is designed so content pages, forms, API routes, state patches,
          and future CMS workflows can live in one Rust-first application shape.
        </Copy>
        <Button slot="actions" href="/posts" variant="ghost">See action demo</Button>
      </FeatureSection>
      <FeatureSection title="Keep the authoring surface readable">
        <Copy slot="eyebrow">DX</Copy>
        <Copy>
          JSX-like `.ax` should feel familiar to frontend developers while the
          compiler lowers pages into Rust runtime structures behind the scenes.
        </Copy>
        <Button slot="actions" href="#start" variant="ghost">Start locally</Button>
      </FeatureSection>
    </ContentGrid>

    <ContentGrid cols={2} gap="lg" id="start">
      <CommandList title="Local development loop">
        <Copy slot="eyebrow">Commands</Copy>
        <ol>
          <li>
            Start the route-aware dev server.
            <code>cargo ax run dev</code>
          </li>
          <li>
            Run diagnostics before sharing.
            <code>cargo ax check</code>
          </li>
          <li>
            Build deployable output.
            <code>cargo ax build --clean</code>
          </li>
        </ol>
        <a slot="actions" href="/posts">Open posts route</a>
      </CommandList>
      <DocsCodeBlock title="Home Route Shape">
        <Copy slot="eyebrow">app/page.ax</Copy>
        {"page Home() -> ASX {\n  return {\n    <HeroCard title=\"{{APP_NAME}}\">\n      <Copy tone=\"lead\">A polished Axonyx starter.</Copy>\n      <Button href=\"/posts\" variant=\"primary\">Open posts demo</Button>\n    </HeroCard>\n  }\n}"}
      </DocsCodeBlock>
    </ContentGrid>
  </Stack>
}
}
