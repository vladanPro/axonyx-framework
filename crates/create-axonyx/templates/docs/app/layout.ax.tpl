page DocsLayout
  title "{{APP_NAME}}"
  meta name: "description", content: "{{APP_NAME}} is an Axonyx-powered documentation site."
  link rel: "icon", href: "/favicon.svg", type: "image/svg+xml"
  Container max: "xl", recipe: "app-shell"
    Card title: "{{APP_NAME}} Docs", recipe: "hero-card"
      img src: "/brand-mark.svg", alt: "{{APP_NAME}} brand mark", width: 80, height: 80
      Copy tone: "lead" -> "A docs-first Axonyx starter with semantic routes, minimal browser-side JavaScript, and room for examples and references."
      nav class: "docs-nav"
        a href: "/" -> "Home"
        a href: "/getting-started" -> "Getting Started"
        a href: "/reference" -> "Reference"
        a href: "/examples" -> "Examples"
    Slot
