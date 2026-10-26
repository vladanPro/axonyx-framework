import { LinkButton } from "@axonyx/ui/foundry/LinkButton.asx"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.asx"

page NotFound() {

  return ASX {
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
          Add a new app folder with page.asx, or head back to the homepage.
        </Copy>
        <LinkButton slot="actions" href="/" variant="primary">Back to home</LinkButton>
        <LinkButton slot="actions" href="/contact" variant="ghost">Contact us</LinkButton>
      </PageHeader>
    </Container>
  }
}
