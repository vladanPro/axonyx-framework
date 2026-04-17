page Posts
  data posts = load PostsList

  Container max: "xl"
    Grid cols: 3, gap: "md", recipe: "content-grid"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
          Button tone: "primary" -> "Read more"
