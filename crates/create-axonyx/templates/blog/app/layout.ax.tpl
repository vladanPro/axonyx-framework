use "@axonyx/ui"
import { Header } from "@axonyx/ui/foundry/Header.ax"
import { Navbar } from "@axonyx/ui/foundry/Navbar.ax"
import { SiteShell } from "@axonyx/ui/foundry/SiteShell.ax"
import { TextLink } from "@axonyx/ui/foundry/TextLink.ax"
import { ThemeSwitcher } from "@axonyx/ui/foundry/ThemeSwitcher.ax"

page BlogLayout() {
  return ASX {
    <Head>
      <Title>{{APP_NAME}} | Field Notes</Title>
      <Theme storageKey="{{APP_SLUG}}-theme" default="bronze" preflight="true" />
      <Meta name="description" content="Essays, field notes, and ideas from {{APP_NAME}}." />
      <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
    </Head>

    <SiteShell max="lg">
      <Header sticky="top">
        <Navbar brandHref="/">
          <span slot="brand">
            <img src="/brand-mark.svg" alt="" width={34} height={34} />
            {{APP_NAME}} / Notes
          </span>
          <TextLink href="/">Writing</TextLink>
          <TextLink href="/about">About</TextLink>
          <ThemeSwitcher label="Theme" size="sm" surface="forged" storageKey="{{APP_SLUG}}-theme" ariaLabel="Choose reading theme" />
        </Navbar>
      </Header>
      <main><Slot /></main>
      <footer class="ax-site-footer">
        <span>{{APP_NAME}} Field Notes</span>
        <span>Markdown in. Static HTML out.</span>
      </footer>
    </SiteShell>
  }
}
