import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<Head>
  <Title>{{APP_NAME}} | Axonyx site starter</Title>
</Head>

<Container max="xl">
  <ContentGrid cols={2} gap="lg">
    <SectionCard title="Write pages that stay readable">
      <Copy tone="lead">
        This site starter uses real @axonyx/ui imports, silver theme styling,
        and route files that stay easy to scan.
      </Copy>
      <Copy>
        Keep structure in .ax, let Rust stay underneath, and use CSS for most
        of the presentation layer.
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
</Container>
