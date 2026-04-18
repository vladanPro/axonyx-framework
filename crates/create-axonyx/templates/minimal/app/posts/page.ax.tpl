page Posts
  data posts = load PostsList

  Container max: "xl"
    Card title: "Create post", recipe: "hero-card"
      Copy tone: "muted" -> "This form posts into route-local actions.ax during dev preview."
      form method: "post", action: action CreatePost, class: "ax-form"
        input type: "text", name: "title", placeholder: "Post title", class: "ax-input"
        textarea name: "excerpt", placeholder: "Short excerpt", class: "ax-textarea"
        Button type: "submit", tone: "primary" -> "Create post"
    Grid cols: 3, gap: "md", recipe: "content-grid"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
          Button tone: "primary" -> "Read more"
