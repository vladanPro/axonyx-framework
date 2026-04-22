import { HeroCard } from "@axonyx/ui/foundry/HeroCard.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"

page RootLayout

<Head>
  <Title>{{APP_NAME}}</Title>
  <Theme>silver</Theme>
  <Meta
    name="description"
    content="{{APP_NAME}} ships presentation-first pages in Axonyx with minimal browser-side JavaScript."
  />
  <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  <Link rel="stylesheet" href="/css/axonyx-ui/index.css" />
</Head>

<SiteShell max="xl">
  <HeroCard title="{{APP_NAME}}">
    <Copy tone="eyebrow">Axonyx site starter</Copy>
    <img
      src="/brand-mark.svg"
      alt="{{APP_NAME}} brand mark"
      width={80}
      height={80}
    />
    <Copy tone="lead">
      A presentation-first starter that already ships with Axonyx UI Foundry
      imports, silver theme tokens, and minimal browser-side JavaScript.
    </Copy>
    <Copy>
      Start editing content in route files while the shell and visual contract
      stay reusable.
    </Copy>
    <nav class="docs-nav">
      <a href="/">Home</a>
      <a href="/posts">Posts</a>
    </nav>
  </HeroCard>
  <Slot />
</SiteShell>
