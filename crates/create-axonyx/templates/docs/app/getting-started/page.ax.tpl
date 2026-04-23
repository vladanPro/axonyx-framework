import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { CommandList } from "@axonyx/ui/foundry/CommandList.ax"
import { DocsSection } from "@axonyx/ui/foundry/DocsSection.ax"
import { DocsCodeBlock } from "@axonyx/ui/foundry/DocsCodeBlock.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page GettingStarted

<Head>
  <Title>Getting Started | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <PageHeader title="Getting Started">
    <Copy slot="eyebrow">Quick Start</Copy>
    <Copy tone="lead">
      Start by generating the app, running cargo ax run dev, and editing
      app/page.ax or a nested route page.
    </Copy>
    <Copy>
      This docs template already includes the vendored UI package, silver theme,
      and route structure so the first edits can stay focused on content.
    </Copy>
    <a slot="actions" href="/reference">Read reference</a>
    <a slot="actions" href="/examples">See examples</a>
    <ContentGrid cols={2} gap="md">
      <SectionCard title="Scaffold">
        <Copy>
          create-axonyx already wires the runtime, UI vendor snapshot, and
          silver theme starter shell for this template.
        </Copy>
      </SectionCard>
      <SectionCard title="Preview">
        <Copy>
          Use cargo run for quick preview generation or cargo ax run dev for
          route-aware local serving.
        </Copy>
      </SectionCard>
      <SectionCard title="Edit">
        <Copy>
          Start with app/layout.ax for the shell, then move through app/page.ax
          and nested route pages as the docs grow.
        </Copy>
      </SectionCard>
      <SectionCard title="Expand">
        <Copy>
          Add routes/ or jobs/ later if your docs app needs APIs, ingestion, or
          scheduled work.
        </Copy>
      </SectionCard>
    </ContentGrid>
  </PageHeader>

  <DocsSection title="Good First Files">
    <Copy slot="eyebrow">Route Shape</Copy>
    <Copy>app/layout.ax defines the shell, metadata, and shared navigation.</Copy>
    <Copy>app/page.ax is the main homepage route.</Copy>
    <Copy>
      app/*/page.ax extends the site with route folders that stay easy to scan.
    </Copy>
    <Copy slot="aside" tone="muted">Start in layout, then move into page-level routes.</Copy>
    <Copy slot="aside" tone="muted">Keep assets in public/ and backend handlers in routes/ or jobs/ only when needed.</Copy>
    <a slot="actions" href="/reference">Open route reference</a>
  </DocsSection>

  <ContentGrid cols={2} gap="lg">
    <CommandList title="Core Commands">
      <Copy slot="eyebrow">CLI Loop</Copy>
      <ol>
        <li>
          Generate the project.
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
          Or generate a quick preview file.
          <code>cargo run</code>
        </li>
      </ol>
      <a slot="actions" href="/reference">See CLI reference</a>
    </CommandList>
    <DocsCodeBlock title="Starter Layout Shape">
      <Copy slot="eyebrow">Example</Copy>
      {"import { SiteShell } from \"@axonyx/ui/foundry/SiteShell.ax\"\n\npage SiteLayout\n\n<Head>\n  <Title>{{APP_NAME}}</Title>\n  <Theme>silver</Theme>\n</Head>\n\n<SiteShell max=\"xl\">\n  <Slot />\n</SiteShell>"}
    </DocsCodeBlock>
  </ContentGrid>
</Container>
