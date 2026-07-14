use "@axonyx/ui"
import { AppShell } from "@axonyx/ui/foundry/AppShell.ax"
import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Navbar } from "@axonyx/ui/foundry/Navbar.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"
import { ThemeSwitcher } from "@axonyx/ui/foundry/ThemeSwitcher.ax"

page DocsLayout() -> ASX {

return {
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

  <AppShell rail="left" railWidth="md">
    <aside slot="sidebar" class="ax-sidebar">
      <div class="ax-sidebar__header">
        <span class="ax-sidebar__title">Docs starter</span>
        <Badge tone="success">Foundry</Badge>
      </div>

      <div class="ax-sidebar__body">
        <section class="ax-sidebar-section">
          <span class="ax-sidebar-section__title">Start</span>
          <div class="ax-sidebar-section__body">
            <a class="ax-sidebar-item" href="/">
              <span class="ax-sidebar-item__content">Overview</span>
            </a>
            <a class="ax-sidebar-item" href="/getting-started">
              <span class="ax-sidebar-item__content">Getting started</span>
            </a>
          </div>
        </section>

        <section class="ax-sidebar-section">
          <span class="ax-sidebar-section__title">Build</span>
          <div class="ax-sidebar-section__body">
            <a class="ax-sidebar-item" href="/components">
              <span class="ax-sidebar-item__content">Components</span>
              <span class="ax-sidebar-item__meta">UI</span>
            </a>
            <a class="ax-sidebar-item" href="/reference">
              <span class="ax-sidebar-item__content">Reference</span>
            </a>
            <a class="ax-sidebar-item" href="/examples">
              <span class="ax-sidebar-item__content">Examples</span>
            </a>
          </div>
        </section>

        <Button href="/components" variant="primary" size="sm">Explore UI</Button>
      </div>
    </aside>

    <main class="ax-main">
      <Slot />
    </main>
  </AppShell>
</SiteShell>
}
}
