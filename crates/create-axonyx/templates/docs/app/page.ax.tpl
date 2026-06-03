import { Alert } from "@axonyx/ui/foundry/Alert.ax"
import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Card } from "@axonyx/ui/foundry/Card.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page DocsHome

<Head>
  <Title>{{APP_NAME}} | Docs</Title>
</Head>

<Container max="xl">
  <PageHeader title="{{APP_NAME}} Docs">
    <Copy slot="eyebrow">Axonyx docs showcase</Copy>
    <Copy tone="lead">
      A docs starter that looks like a real product surface from the first run:
      theme switcher, sidebar navigation, Foundry components, and route-first
      pages.
    </Copy>
    <Copy>
      Use this template for framework docs, internal platforms, product guides,
      or any site that should not need a heavy client-side app just to explain
      itself.
    </Copy>
    <Button slot="actions" href="/getting-started" variant="primary">Get started</Button>
    <Button slot="actions" href="/components" variant="ghost">View components</Button>
    <Badge slot="actions" tone="success">Low JS</Badge>
  </PageHeader>

  <Alert tone="info" title="Docker demo ready">
    This template is what the Axonyx Docker demo image runs by default.
  </Alert>

  <ContentGrid cols={3} gap="lg">
    <Card title="Theme-first">
      <Stack gap="md" align="start">
        <div class="ax-status-lamp" data-tone="success" data-pulse="true">
          <span class="ax-status-lamp__light"></span>
          <span class="ax-status-lamp__body">
            <span class="ax-status-lamp__label">Preflight theme</span>
            <span class="ax-status-lamp__description">Stored themes apply before CSS paints.</span>
          </span>
        </div>
        <Copy>
          The layout uses `Theme default="silver" storageKey="{{APP_SLUG}}-theme" preflight="true"`.
        </Copy>
      </Stack>
    </Card>
    <Card title="Route-first docs">
      <Copy>
        Each section is a normal `app/**/page.ax` route, wrapped by the shared
        docs layout and sidebar.
      </Copy>
      <Button href="/getting-started" variant="ghost" size="sm">Open guide</Button>
    </Card>
    <Card title="Foundry UI">
      <Copy>
        Buttons, cards, badges, alerts, lamps, and layout primitives are already
        wired through `use "@axonyx/ui"`.
      </Copy>
      <Button href="/components" variant="primary" size="sm">Open showcase</Button>
    </Card>
  </ContentGrid>

  <ContentGrid cols={2} gap="lg">
    <Card title="What to edit first">
      <Copy>Edit `app/page.ax` for the overview page.</Copy>
      <Copy>Edit `app/layout.ax` for the shell, nav, theme, and sidebar.</Copy>
      <Copy>Add nested folders with `page.ax` to create new routes.</Copy>
    </Card>
    <Card title="Production loop">
      <Copy>`cargo ax check` validates `.ax` sources.</Copy>
      <Copy>`cargo ax build --clean` writes static output and route artifacts.</Copy>
      <Copy>`cargo ax run start` serves through the Axonyx production path.</Copy>
    </Card>
  </ContentGrid>
</Container>
