import { LinkButton } from "@axonyx/ui/foundry/LinkButton.asx"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.asx"

page NotFound() {

  return ASX {
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
          Create a new app/**/page.asx file or return to the docs index.
        </Copy>
        <LinkButton slot="actions" href="/" variant="primary">Docs home</LinkButton>
        <LinkButton slot="actions" href="/getting-started" variant="ghost">Getting started</LinkButton>
      </PageHeader>
    </Container>
  }
}
