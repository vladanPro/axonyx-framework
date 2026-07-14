import { Button } from "@axonyx/ui/foundry/Button.ax"
import { Copy } from "@axonyx/ui/foundry/Copy.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"

page Error() -> ASX {
  return {
    <Container max="md">
      <PageHeader title="The press stopped">
        <Copy slot="eyebrow">Build error</Copy>
        <Copy tone="lead">Run `cargo ax check` to inspect the content or route diagnostic.</Copy>
        <Button slot="actions" href="/" variant="primary">Back to the journal</Button>
      </PageHeader>
    </Container>
  }
}
