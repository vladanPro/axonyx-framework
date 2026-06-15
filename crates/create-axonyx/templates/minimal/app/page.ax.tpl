page Home() -> ASX {
return {
  <Container max="xl" recipe="hello-shell">
    <Card title="Hello Axonyx" recipe="hero-card">
      <Copy tone="eyebrow">{{APP_NAME}}</Copy>
      <Copy tone="lead">
        Start with one clean .ax page, keep JavaScript small, and let Rust stay
        underneath the framework.
      </Copy>
      <Button tone="primary">Edit app/page.ax</Button>
    </Card>
    <Grid cols={3} gap="md">
      <Card title="Write in .ax">
        <Copy>
          Keep pages readable with JSX-like authoring instead of framework
          boilerplate.
        </Copy>
      </Card>
      <Card title="Lower into Rust">
        <Copy>
          Axonyx stays framework-shaped on top while Rust remains the execution
          engine below.
        </Copy>
      </Card>
      <Card title="Grow from here">
        <Copy>
          Use app/posts, routes/api, and jobs as the next slice once this hello
          page feels right.
        </Copy>
      </Card>
    </Grid>
  </Container>
}
}
