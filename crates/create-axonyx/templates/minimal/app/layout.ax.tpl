page RootLayout
  title "{{APP_NAME}}"
  meta name: "description", content: "{{APP_NAME}} is a fresh Axonyx app scaffold."
  link rel: "icon", href: "/favicon.svg", type: "image/svg+xml"
  Container max: "xl", recipe: "app-shell"
    Copy tone: "eyebrow" -> "{{APP_NAME}}"
    Copy tone: "muted" -> "app/layout.ax wraps app/page.ax during preview."
    Slot
