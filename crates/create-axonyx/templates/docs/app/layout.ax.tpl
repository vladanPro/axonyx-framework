import { HeroCard } from "@axonyx/ui/foundry/HeroCard.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"

page DocsLayout

<Head>
  <Title>{{APP_NAME}}</Title>
  <Theme>silver</Theme>
  <Meta
    name="description"
    content="{{APP_NAME}} is an Axonyx-powered documentation site."
  />
  <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  <Link rel="stylesheet" href="/css/axonyx-ui/index.css" />
</Head>

<SiteShell max="xl">
  <HeroCard title="{{APP_NAME}} Docs">
    <img
      src="/brand-mark.svg"
      alt="{{APP_NAME}} brand mark"
      width={80}
      height={80}
    />
    <Copy tone="lead">
      A docs-first Axonyx starter with semantic routes, real Foundry imports,
      and minimal browser-side JavaScript.
    </Copy>
    <nav class="docs-nav">
      <a href="/">Home</a>
      <a href="/getting-started">Getting Started</a>
      <a href="/reference">Reference</a>
      <a href="/examples">Examples</a>
    </nav>
  </HeroCard>
  <Slot />
</SiteShell>
