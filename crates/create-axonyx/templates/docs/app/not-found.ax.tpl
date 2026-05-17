import { Button } from "@axonyx/ui/foundry/Button.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"

page NotFound

<Head>
  <Title>{{APP_NAME}} Docs | Page not found</Title>
</Head>

<Container max="xl">
  <PageHeader title="Docs page not found">
    <Copy slot="eyebrow">404</Copy>
    <Copy tone="lead">
      This docs route does not exist yet.
    </Copy>
    <Copy>
      Create a new app/**/page.ax file or return to the docs index.
    </Copy>
    <Button slot="actions" href="/" variant="primary">Docs home</Button>
    <Button slot="actions" href="/getting-started" variant="ghost">Getting started</Button>
  </PageHeader>
</Container>
