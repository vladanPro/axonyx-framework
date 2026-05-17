import { Button } from "@axonyx/ui/foundry/Button.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"

page Error

<Head>
  <Title>{{APP_NAME}} Docs | Application error</Title>
</Head>

<Container max="xl">
  <PageHeader title="Docs render error">
    <Copy slot="eyebrow">500</Copy>
    <Copy tone="lead">
      Axonyx could not render this docs route.
    </Copy>
    <Copy>
      Run cargo ax check to inspect the page, layout, imports, and loaders.
    </Copy>
    <Button slot="actions" href="/" variant="primary">Docs home</Button>
  </PageHeader>
</Container>
