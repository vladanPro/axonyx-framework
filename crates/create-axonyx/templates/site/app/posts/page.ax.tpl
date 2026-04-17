page Posts
  Container max: "xl"
    Copy tone: "muted" -> "Featured writing"
    Grid cols: 2, gap: "lg"
      each post in posts
        Card title: post.title
          Copy -> post.excerpt
