page Posts
  data posts = load PostsList

  Container max: "xl"
    Card title: "Publish from Axonyx"
      Copy tone: "muted" -> "This route can load and mutate without a client-side framework shell."
      form method: "post", action: action CreatePost, class: "ax-form"
        input type: "text", name: "title", placeholder: "New story title", class: "ax-input"
        textarea name: "excerpt", placeholder: "Short story excerpt", class: "ax-textarea"
        Button type: "submit", tone: "primary" -> "Add story"
    Copy tone: "muted" -> "Featured writing"
    Grid cols: 2, gap: "lg"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
