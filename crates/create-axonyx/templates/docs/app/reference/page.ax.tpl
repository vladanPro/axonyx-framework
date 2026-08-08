import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { DocsSection } from "@axonyx/ui/foundry/DocsSection.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Reference() {
  return ASX {
    <Head>
      <Title>Reference | {{APP_NAME}}</Title>
    </Head>

    <Container max="xl">
      <PageHeader title="Reference">
        <Copy slot="eyebrow">Surface Area</Copy>
        <Copy tone="lead">
          Use this page to document the stable contract your project exposes:
          routes, components, commands, and deployment expectations.
        </Copy>
        <Copy>
          The starter keeps frontend authoring in `.ax` route files while the
          compiler/runtime handle package assets, static output, checks, and the
          production server path.
        </Copy>
      </PageHeader>

      <ContentGrid cols={3} gap="md">
        <SectionCard title="Authoring">
          <Copy>Function-shaped pages with explicit `return ASX` blocks are the recommended page shape.</Copy>
          <Copy>Use native HTML tags and imported Foundry components together.</Copy>
        </SectionCard>
        <DocsSection title="Runtime">
          <Copy slot="eyebrow">Execution</Copy>
          <Copy>
            Axonyx lowers route files into generated Rust-backed runtime code,
            writes static output, and serves the same route tree in dev/start.
          </Copy>
          <Copy slot="aside" tone="muted">Static docs do not need a database, API route, or jobs folder.</Copy>
          <a slot="actions" href="/getting-started">Connect it to setup</a>
        </DocsSection>
        <SectionCard title="CLI">
          <Copy>`cargo ax run dev` starts local development.</Copy>
          <Copy>`cargo ax check` validates source files.</Copy>
          <Copy>`cargo ax build --clean` writes deployable output.</Copy>
        </SectionCard>
      </ContentGrid>
    </Container>
  }
}
