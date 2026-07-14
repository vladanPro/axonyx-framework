import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"

page NotFound() -> ASX {
  return {
    <Container max="md">
      <PageHeader title="This note is not on the workbench">
        <Copy slot="eyebrow">404</Copy>
        <Copy tone="lead">The link may be old, or the article may still be a draft.</Copy>
        <Button slot="actions" href="/" variant="primary">Browse all notes</Button>
      </PageHeader>
    </Container>
  }
}
