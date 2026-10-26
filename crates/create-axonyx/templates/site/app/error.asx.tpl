import { LinkButton } from "@axonyx/ui/foundry/LinkButton.asx"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.asx"

page Error() {

  return ASX {
    <Head>
      <Title>{{APP_NAME}} | Application error</Title>
    </Head>

    <Container max="xl">
      <PageHeader title="Application error">
        <Copy slot="eyebrow">500</Copy>
        <Copy tone="lead">
          Axonyx could not render this route.
        </Copy>
        <Copy>
          Run cargo ax check to inspect parser, import, loader, and route diagnostics.
        </Copy>
        <LinkButton slot="actions" href="/" variant="primary">Back to home</LinkButton>
      </PageHeader>
    </Container>
  }
}
