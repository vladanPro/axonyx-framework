page GettingStarted
  Container max: "xl"
    Card title: "Getting Started"
      Copy tone: "lead" -> "Start by generating an app, running `cargo ax run dev`, and editing `app/page.ax` or a nested route page."
      Grid cols: 2, gap: "md"
        Card title: "Scaffold"
          Copy -> "Use `create-axonyx` to generate a docs-first starter with the shared runtime already wired in."
        Card title: "Preview"
          Copy -> "Use `cargo run` for quick previews or `cargo ax run dev` for route-aware local serving."
