import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { DocsCodeBlock } from "@axonyx/ui/foundry/DocsCodeBlock.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Feedback() -> ASX {

page state feedbackStatus: String = "ready"

return {
<Head>
  <Title>Feedback | {{APP_NAME}}</Title>
</Head>

<Container max="xl">
  <PageHeader title="Feedback">
    <Copy slot="eyebrow">Action Demo</Copy>
    <Copy tone="lead">
      This route shows how docs can collect structured feedback with a
      route-local Axonyx action and no React form shell.
    </Copy>
    <a slot="actions" href="/getting-started">Getting started</a>
    <a slot="actions" href="/reference">Reference</a>
  </PageHeader>

  <ContentGrid cols={2} gap="lg">
    <SectionCard title="Send feedback">
      <ActionForm name="SendFeedback">
        <input type="text" name="name" placeholder="Your name" class="ax-input" />
        <textarea name="message" placeholder="What should these docs explain better?" class="ax-textarea">
        </textarea>
        <select name="tone" class="ax-select">
          <option value="idea">idea</option>
          <option value="bug">bug</option>
          <option value="question">question</option>
        </select>
        <Button type="submit" tone="primary">Send feedback</Button>
        <ActionStatus state="pending">Sending feedback...</ActionStatus>
        <ActionStatus state="complete">Feedback recorded.</ActionStatus>
        <ActionStatus state="error">Feedback could not be recorded.</ActionStatus>
      </ActionForm>
      <Copy tone="muted">Last feedback type:</Copy>
      <strong bind:text={feedbackStatus}>{feedbackStatus}</strong>
    </SectionCard>

    <DocsCodeBlock title="Inspect the action">
      <Copy slot="eyebrow">CLI</Copy>
      {"cargo ax actions\ncargo ax actions --format json"}
    </DocsCodeBlock>
  </ContentGrid>
</Container>
}
}
