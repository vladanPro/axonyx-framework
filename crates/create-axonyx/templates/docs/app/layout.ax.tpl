use "@axonyx/ui"
import { AppShell } from "@axonyx/ui/foundry/AppShell.ax"
import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { DocsNav } from "@axonyx/ui/foundry/DocsNav.ax"
import { Navbar } from "@axonyx/ui/foundry/Navbar.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"
import { ThemeSwitcher } from "@axonyx/ui/foundry/ThemeSwitcher.ax"

page DocsLayout

<Head>
  <Title>{{APP_NAME}} | Axonyx Docs Starter</Title>
  <Theme default="silver" storageKey="{{APP_SLUG}}-theme" preflight="true" />
  <Meta
    name="description"
    content="{{APP_NAME}} is an Axonyx-powered documentation site with Foundry UI."
  />
  <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
</Head>

<SiteShell max="xl">
  <Navbar brandHref="/">
    <span slot="brand">
      <img src="/brand-mark.svg" alt="{{APP_NAME}}" width={36} height={30} />
      {{APP_NAME}}
    </span>
    <a href="/getting-started">Docs</a>
    <a href="/components">Components</a>
    <a href="/reference">Reference</a>
    <ThemeSwitcher label="Theme" size="sm" surface="forged" storageKey="{{APP_SLUG}}-theme" ariaLabel="Theme switcher" />
  </Navbar>

  <AppShell rail="left" railWidth="18rem">
    <DocsNav slot="sidebar" title="Docs starter">
      <a href="/">Overview</a>
      <a href="/getting-started">Getting started</a>
      <a href="/components">Components</a>
      <a href="/reference">Reference</a>
      <a href="/examples">Examples</a>
      <a href="/feedback">Feedback action</a>
      <Badge slot="actions" tone="success">Foundry ready</Badge>
      <Button slot="actions" href="/components" variant="primary" size="sm">Explore UI</Button>
    </DocsNav>

    <main>
      <Slot />
    </main>
  </AppShell>
</SiteShell>
