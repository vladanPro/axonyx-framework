import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { CommandList } from "@axonyx/ui/foundry/CommandList.ax"
import { DocsSection } from "@axonyx/ui/foundry/DocsSection.ax"
import { DocsCodeBlock } from "@axonyx/ui/foundry/DocsCodeBlock.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page GettingStarted() {
  return ASX {
    <Head>
      <Title>Getting Started | {{APP_NAME}}</Title>
    </Head>

    <Container max="xl">
      <PageHeader title="Getting Started">
        <Copy slot="eyebrow">Quick Start</Copy>
        <Copy tone="lead">
          Generate the app, run the Axonyx dev server, edit a route file, then
          check and build the static output.
        </Copy>
        <Copy>
          This template already includes Foundry UI, theme preflight, sidebar
          navigation, route checks, and a docs-shaped folder structure.
        </Copy>
        <a slot="actions" href="/reference">Read reference</a>
        <a slot="actions" href="/examples">See examples</a>
      </PageHeader>

      <ContentGrid cols={2} gap="lg">
        <CommandList title="First five commands">
          <Copy slot="eyebrow">CLI loop</Copy>
          <ol>
            <li>
              Create a docs app.
              <code>create-axonyx {{APP_SLUG}} --template docs</code>
            </li>
            <li>
              Move into the app directory.
              <code>cd {{APP_SLUG}}</code>
            </li>
            <li>
              Start local development.
              <code>cargo ax run dev</code>
            </li>
            <li>
              Check the project before sharing it.
              <code>cargo ax check</code>
            </li>
            <li>
              Build deployable output.
              <code>cargo ax build --clean</code>
            </li>
          </ol>
          <a slot="actions" href="/reference">See CLI reference</a>
        </CommandList>

        <DocsCodeBlock title="Starter page shape">
          <Copy slot="eyebrow">ASX</Copy>
          {"import { PageHeader } from \"@axonyx/ui/foundry/PageHeader.ax\"\n\npage Guide() {\n  return ASX {\n    <Container max=\"xl\">\n      <PageHeader title=\"Guide\">\n        <Copy tone=\"lead\">Write docs in .ax.</Copy>\n      </PageHeader>\n    </Container>\n  }\n}"}
        </DocsCodeBlock>
      </ContentGrid>

      <DocsSection title="Good first files">
        <Copy slot="eyebrow">Route shape</Copy>
        <Copy>`app/layout.ax` defines the shell, metadata, theme, nav, and sidebar.</Copy>
        <Copy>`app/page.ax` is the overview route at `/`.</Copy>
        <Copy>`app/getting-started/page.ax` is this page.</Copy>
        <Copy slot="aside" tone="muted">Add `app/guides/page.ax` to create `/guides`.</Copy>
        <Copy slot="aside" tone="muted">Keep `routes/` and `jobs/` out until your docs need server behavior.</Copy>
        <a slot="actions" href="/reference">Open route reference</a>
      </DocsSection>

      <ContentGrid cols={2} gap="md">
        <SectionCard title="Edit">
          <Copy>
            Change one title in `app/page.ax`, reload the browser, then move
            into nested pages as your docs grow.
          </Copy>
        </SectionCard>
        <SectionCard title="Validate">
          <Copy>
            Use `cargo ax check`, `cargo ax doctor`, and `cargo ax test` before
            deploy. The starter already includes route QA in `aegis.toml`.
          </Copy>
        </SectionCard>
        <SectionCard title="Style">
          <Copy>
            Foundry UI is activated by `use "@axonyx/ui"` in `app/layout.ax`.
            Change the default theme from silver to bronze or gold there.
          </Copy>
        </SectionCard>
        <SectionCard title="Deploy">
          <Copy>
            `cargo ax build --clean` writes `dist/`. Serve it statically, or run
            `cargo ax run start` for the Axonyx production server path.
          </Copy>
        </SectionCard>
      </ContentGrid>
    </Container>
  }
}
