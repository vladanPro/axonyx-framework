import { LinkButton } from "@axonyx/ui/foundry/LinkButton.asx"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.asx"

page Error() {

  return ASX {
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
        <LinkButton slot="actions" href="/" variant="primary">Docs home</LinkButton>
      </PageHeader>
    </Container>
  }
}
