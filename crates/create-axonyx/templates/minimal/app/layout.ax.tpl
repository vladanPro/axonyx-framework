page RootLayout() -> ASX {
return {
  <Head>
    <Title>{{APP_NAME}}</Title>
    <Meta name="description" content="{{APP_NAME}} is a fresh Axonyx app scaffold." />
    <Link rel="icon" href="/favicon.svg" type="image/svg+xml" />
  </Head>

  <Container max="xl" recipe="app-shell">
    <Copy tone="eyebrow">{{APP_NAME}}</Copy>
    <Copy tone="muted">app/layout.ax wraps app/page.ax during preview.</Copy>
    <Slot />
  </Container>
}
}
