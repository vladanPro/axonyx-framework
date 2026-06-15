import { Alert } from "@axonyx/ui/foundry/Alert.ax"
import { Badge } from "@axonyx/ui/foundry/Badge.ax"
import { Button } from "@axonyx/ui/foundry/Button.ax"
import { ButtonGroup } from "@axonyx/ui/foundry/ButtonGroup.ax"
import { Card } from "@axonyx/ui/foundry/Card.ax"
import { ContentGrid } from "@axonyx/ui/foundry/ContentGrid.ax"
import { PageHeader } from "@axonyx/ui/foundry/PageHeader.ax"
import { Progress } from "@axonyx/ui/foundry/Progress.ax"
import { Stack } from "@axonyx/ui/foundry/Stack.ax"

page ComponentsShowcase() -> ASX {

return {
<Head>
  <Title>{{APP_NAME}} | Components</Title>
</Head>

<Container max="xl">
  <PageHeader title="Component showcase">
    <Copy slot="eyebrow">Foundry primitives</Copy>
    <Copy tone="lead">
      A small set of practical UI pieces that prove the docs template can look
      like a real site without becoming a React app.
    </Copy>
    <Button slot="actions" href="/reference" variant="ghost">Reference</Button>
  </PageHeader>

  <ContentGrid cols={2} gap="lg">
    <Card title="Buttons and badges">
      <Stack gap="md" align="start">
        <ButtonGroup>
          <Button variant="primary">Primary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="soft">Soft</Button>
        </ButtonGroup>
        <div>
          <Badge tone="success">Ready</Badge>
          <Badge tone="warning">Draft</Badge>
          <Badge tone="danger">Blocked</Badge>
        </div>
      </Stack>
    </Card>

    <Card title="Feedback">
      <Stack gap="md">
        <Alert tone="success" title="Build passed">
          The docs starter generated, checked, and built successfully.
        </Alert>
        <Alert tone="warning" title="Edit me">
          Replace these examples with your own product documentation.
        </Alert>
      </Stack>
    </Card>

    <Card title="Foundry status">
      <Stack gap="md">
        <div class="ax-status-lamp" data-tone="success" data-pulse="true">
          <span class="ax-status-lamp__light"></span>
          <span class="ax-status-lamp__body">
            <span class="ax-status-lamp__label">Server path</span>
            <span class="ax-status-lamp__description">Tokio/Axum production server is active.</span>
          </span>
        </div>
        <div class="ax-status-lamp" data-tone="warning">
          <span class="ax-status-lamp__light"></span>
          <span class="ax-status-lamp__body">
            <span class="ax-status-lamp__label">Docs content</span>
            <span class="ax-status-lamp__description">Add your real product sections next.</span>
          </span>
        </div>
        <Progress value="64" label="Docs readiness" />
      </Stack>
    </Card>

    <Card title="Industrial controls">
      <Stack gap="md" align="start">
        <button class="ax-machine-switch" data-state="on" type="button" aria-pressed="true">
          <span class="ax-machine-switch__controls">
            <span class="ax-machine-switch__pad" data-tone="danger" data-active="false">OFF</span>
            <span class="ax-machine-switch__pad" data-tone="success" data-active="true">ON</span>
          </span>
          <span class="ax-machine-switch__label">Foundry mode</span>
          <span class="ax-machine-switch__label" data-state="true">ON</span>
        </button>
        <Copy>
          MachineSwitch and StatusLamp give Foundry a more industrial feel than
          generic component libraries.
        </Copy>
      </Stack>
    </Card>
  </ContentGrid>

  <Card title="Interactive patterns">
    <div class="ax-tabs" data-default-value="overview">
      <div class="ax-tab" data-value="overview">
        <div class="ax-tab__label">Overview</div>
        <div class="ax-tab__panel">
        <Copy>Tabs are wired by the Axonyx UI behavior runtime.</Copy>
        </div>
      </div>
      <div class="ax-tab" data-value="authoring">
        <div class="ax-tab__label">Authoring</div>
        <div class="ax-tab__panel">
        <Copy>Use components directly from `.ax` files with JSX-like syntax.</Copy>
        </div>
      </div>
      <div class="ax-tab" data-value="deploy">
        <div class="ax-tab__label">Deploy</div>
        <div class="ax-tab__panel">
        <Copy>Build with `cargo ax build --clean` and run with `cargo ax run start`.</Copy>
        </div>
      </div>
    </div>
  </Card>

  <div class="ax-accordion" data-single="true">
    <div class="ax-accordion__item" data-open="true">
      <div class="ax-accordion__trigger">Can I change the theme?</div>
      <div class="ax-accordion__content">
        <Copy>Yes. Use the theme switcher in the shell. The choice is stored before first paint.</Copy>
      </div>
    </div>
    <div class="ax-accordion__item">
      <div class="ax-accordion__trigger">Can I add more pages?</div>
      <div class="ax-accordion__content">
        <Copy>Yes. Add `app/my-section/page.ax` and link it from `app/layout.ax`.</Copy>
      </div>
    </div>
  </div>
</Container>
}
}
