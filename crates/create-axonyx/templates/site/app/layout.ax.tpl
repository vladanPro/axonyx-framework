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
      <Theme storageKey="{{APP_SLUG}}-theme" default="silver" preflight="true" />
      <Meta name="description" content="{{APP_NAME}} is a fast static site built with Axonyx and Foundry UI." />
      <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
    </Head>

    <SiteShell max="xl">
      <Header sticky="top">
        <Navbar brandHref="/">
          <span slot="brand">
            <img src="/brand-mark.svg" alt="" width={32} height={32} />
            {{APP_NAME}}
          </span>
          <TextLink href="/">Home</TextLink>
          <TextLink href="/about">About</TextLink>
          <TextLink href="/contact">Contact</TextLink>
          <ThemeSwitcher label="Theme" size="sm" surface="raised" storageKey="{{APP_SLUG}}-theme" ariaLabel="Choose theme" />
        </Navbar>
      </Header>

      <main>
        <Slot />
      </main>

      <footer class="ax-site-footer">
        <span>{{APP_NAME}}</span>
        <span>Built as static HTML with Axonyx.</span>
      </footer>
    </SiteShell>
  }
}
