import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Examples

<Head>
  <Title>Examples | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <SectionCard title="Examples">
    <Copy tone="lead">
      Use this section for focused examples that teach one concept at a time.
    </Copy>
  </SectionCard>

  <ContentGrid cols={2} gap="md">
    <SectionCard title="Landing Page">
      <Copy>
        Show how a marketing-style page can be expressed with semantic HTML,
        Foundry components, and minimal JavaScript.
      </Copy>
    </SectionCard>
    <SectionCard title="Docs Module">
      <Copy>
        Show how section pages, nested layouts, and static assets fit together
        in a real Axonyx site.
      </Copy>
    </SectionCard>
  </ContentGrid>
</Container>
