import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { CommandList } from "@axonyx/ui/foundry/CommandList.ax"
import { DocsCallout } from "@axonyx/ui/foundry/DocsCallout.ax"
import { DocsCodeBlock } from "@axonyx/ui/foundry/DocsCodeBlock.ax"
import { DocsNav } from "@axonyx/ui/foundry/DocsNav.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<Head>
  <Title>{{APP_NAME}} | Axonyx site starter</Title>
</Head>

<Container max="xl">
  <PageHeader title="{{APP_NAME}}">
    <Copy slot="eyebrow">Site Starter</Copy>
    <Copy tone="lead">
      This site starter uses real @axonyx/ui imports, silver theme styling,
      and route files that stay easy to scan.
    </Copy>
    <Copy>
      Keep structure in .ax, let Rust stay underneath, and use CSS for most of
      the presentation layer.
    </Copy>
    <a slot="actions" href="/posts">Open posts</a>
    <a slot="actions" href="/">Refresh homepage</a>
  </PageHeader>

  <ContentGrid cols={2} gap="lg">
    <SectionCard title="Write pages that stay readable">
      <Copy>
        Route pages can stay focused on content and composition while Foundry
        components carry more of the repeated visual contract.
      </Copy>
    </SectionCard>
    <SectionCard title="Ship the kinds of sites we actually want">
      <Copy>
        Use this template for product pages, framework sites, launch surfaces,
        and other content-first experiences that should not start from a heavy
        React shell.
      </Copy>
      <Copy>
        The first page already consumes Foundry components instead of only raw
        built-ins.
      </Copy>
    </SectionCard>
  </ContentGrid>

  <ContentGrid cols={3} gap="lg">
    <SectionCard title="Foundry Ready">
      <Copy>
        The scaffold vendors axonyx-ui into vendor/axonyx-ui and syncs its CSS
        into public/css/axonyx-ui so the visual layer works immediately.
      </Copy>
    </SectionCard>
    <SectionCard title="Rust-First">
      <Copy>
        The runtime, preview generation, and backend story stay in Rust while
        the page authoring layer remains framework-friendly.
      </Copy>
    </SectionCard>
    <SectionCard title="Grow From Here">
      <Copy>
        Expand app/, keep assets in public/, and use routes/ or jobs/ when this
        site needs backend behavior.
      </Copy>
    </SectionCard>
  </ContentGrid>

  <ContentGrid cols={2} gap="lg">
    <DocsCallout title="Current Best Fit">
      <Copy slot="eyebrow">Signal</Copy>
      <Copy>
        This starter is strongest for framework sites, product pages, launch
        surfaces, and other content-first experiences with minimal JS needs.
      </Copy>
    </DocsCallout>
    <DocsNav title="Where To Go Next">
      <Copy>
        After the homepage, start shaping nested routes, connect real assets,
        and then decide whether the project needs posts, APIs, or jobs.
      </Copy>
      <a slot="actions" href="/posts">Open posts route</a>
      <a slot="actions" href="/">Keep iterating</a>
    </DocsNav>
  </ContentGrid>

  <ContentGrid cols={2} gap="lg">
    <CommandList title="Core Commands">
      <Copy slot="eyebrow">Site Loop</Copy>
      <ol>
        <li>
          Start the local route-aware dev loop.
          <code>cargo ax run dev</code>
        </li>
        <li>
          Generate a static preview when needed.
          <code>cargo run</code>
        </li>
        <li>
          Inspect and edit the route source.
          <code>app/page.ax</code>
        </li>
      </ol>
      <a slot="actions" href="/posts">Open posts route</a>
    </CommandList>
    <DocsCodeBlock title="Home Route Shape">
      <Copy slot="eyebrow">Example</Copy>
      {"page Home\n\n<Container max=\"xl\">\n  <PageHeader title=\"{{APP_NAME}}\">\n    <Copy tone=\"lead\">Your first site section.</Copy>\n  </PageHeader>\n</Container>"}
    </DocsCodeBlock>
  </ContentGrid>
</Container>
