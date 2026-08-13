page RootLayout() {
return ASX {
  <Head>
    <Title>{{APP_NAME}}</Title>
    <Meta name="description" content="{{APP_NAME}} is a fresh Axonyx app scaffold." />
    <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  </Head>

  <Container max="xl" recipe="app-shell">
    <Copy tone="eyebrow">{{APP_NAME}}</Copy>
    <Copy tone="muted">app/layout.asx wraps app/page.asx during preview.</Copy>
    <Slot />
  </Container>
}
}
