use "@axonyx/ui"
import { Header } from "@axonyx/ui/foundry/Header.ax"
import { Navbar } from "@axonyx/ui/foundry/Navbar.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"
import { TextLink } from "@axonyx/ui/foundry/TextLink.ax"
import { ThemeSwitcher } from "@axonyx/ui/foundry/ThemeSwitcher.ax"

page RootLayout() -> ASX {

return {
<Head>
  <Title>{{APP_NAME}}</Title>
  <Theme storageKey="axonyx-site-theme" default="silver" preflight="true" />
  <Meta
    name="description"
    content="{{APP_NAME}} is an Axonyx site starter with Foundry UI, route-first pages, and minimal browser-side JavaScript."
  />
  <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
</Head>

<SiteShell max="xl">
  <Header sticky="top">
    <Navbar brandHref="/">
      <span slot="brand">{{APP_NAME}}</span>
      <TextLink href="/">Home</TextLink>
      <TextLink href="/posts">Posts</TextLink>
      <TextLink href="/api/posts">API</TextLink>
      <ThemeSwitcher
        label="Theme"
        size="sm"
        surface="raised"
        storageKey="axonyx-site-theme"
        ariaLabel="Choose Foundry theme"
      />
    </Navbar>
  </Header>
  <Slot />
</SiteShell>
}
}
