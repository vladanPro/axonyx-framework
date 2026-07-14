import { Button } from "@axonyx/ui/foundry/Button.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"

page NotFound() -> ASX {

return {
<Head>
  <Title>{{APP_NAME}} | Page not found</Title>
</Head>

<Container max="xl">
  <PageHeader title="Page not found">
    <Copy slot="eyebrow">404</Copy>
    <Copy tone="lead">
      Axonyx could not find a matching route for this URL.
    </Copy>
    <Copy>
      Add a new app folder with page.ax, or head back to the homepage.
    </Copy>
    <Button slot="actions" href="/" variant="primary">Back to home</Button>
    <Button slot="actions" href="/contact" variant="ghost">Contact us</Button>
  </PageHeader>
</Container>
}
}
