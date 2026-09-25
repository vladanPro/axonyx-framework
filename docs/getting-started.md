# Getting Started

This is the practical starting point for Axonyx development today.

## Local Scaffold

For a new project, install the published CLI tools:

```bash
cargo install create-axonyx
cargo install cargo-axonyx
create-axonyx my-app --yes --template site
```

The default starter uses published crates.io packages. You do not need to clone
the framework repository or initialize its runtime submodule.

Then:

```bash
cd my-app
cargo ax run dev
```

From another terminal in the app root, verify the project:

```bash
cargo ax check
cargo ax doctor
cargo ax build --clean
```

The `site` starter is static and needs no database. Use `--template blog` for a
content collection or `--template docs` for a docs shell. `--template minimal`
is the full-stack playground for loaders, actions, API routes, and jobs.

`cargo ax build` regenerates `src/generated/backend.rs` from:

- `app/**/loader.ax`
- `app/**/actions.ax`
- `routes/**/*.ax`
- `jobs/**/*.ax`

`cargo ax run dev` runs backend sync before starting the route-aware dev server.

`cargo ax doctor` checks the app shape, runtime dependency, UI package wiring, package CSS, and `.asx`/`.ax` diagnostics before you start chasing browser issues.

`cargo ax content` indexes configured content collections, which is the first filesystem/content layer for future docs, blog, and CMS flows.
`cargo ax build` writes that manifest to `dist/_ax/content/manifest.json` when collections are configured.
Route loaders can now read configured content collections:

```ax
query loadDocs() -> Doc[] {
  data docs = Content.Collection("docs")
    order slug asc
  return docs
}
```

Each markdown entry exposes manifest fields plus content fields: `path`, `slug`,
`extension`, `bytes`, `body`, and simple frontmatter keys such as `title`.

`cargo ax schema pull` is the first "fast Swagger" command. It can inspect JSON from a file, inline JSON, or a local `http://` endpoint and print a draft `.ax type`:

```bash
cargo ax schema pull ./sample-posts.json --name Post
```

For real loaders and API endpoints, prefer a typed envelope. That lets the backend
send the exact DTO contract, while `data` can still contain `null` or missing
optional values:

```json
{
  "type": "List<Post>",
  "schemaHash": "sha256:abc123",
  "schema": {
    "Post": {
      "slug": "String",
      "summary": "Optional<String>",
      "title": "String"
    }
  },
  "data": []
}
```

Example output:

```ax
type Post {
  slug: String
  summary?: String
  title: String
}

// root: List<Post>
```

## Typed Data And Each

Axonyx now has an early typed data path for JSX-like `.asx` files. Define a record shape, bind query data to a typed list, and `cargo ax check` can catch wrong field access before render. For example, in `app/posts/page.asx`:

```ax
import { Card } from "@axonyx/ui/foundry/Card.asx"
import { Copy } from "@axonyx/ui/foundry/Copy.asx"

page Blog() -> ASX {

type Post {
  title: String
  slug: String
  excerpt: String
  summary?: String
}

data posts: List<Post> = loadPosts()

return {
<Each items={posts} as="post">
  <Card title={post.title}>
    <Copy>{post.excerpt}</Copy>
  </Card>
</Each>
}
}
```

Route-local data can live next to the page in `app/posts/loader.ax`:

```ax
query loadPosts() -> Post[] {
  data posts = db.posts.all()
  return posts
}
```

If the page uses an undeclared field such as `post.subtitle`, `cargo ax check`
reports an `axonyx-type` diagnostic. This bridges Axonyx primitives like
`String` and `List<Post>` to Rust-side Axonyx types.

When the record itself may be absent, use safe member access:

```ax
<Copy>{post?.summary}</Copy>
```

If `post` is absent, the expression renders as an empty string instead of
failing. If only a field is optional in the type, regular access is allowed and
resolves to `Optional<T>`:

```ax
type Post {
  summary?: String
}

<Copy>{post.summary}</Copy>
```

For a production-style local run, use:

```bash
cargo ax build
cargo ax run start --host 0.0.0.0 --port 3000
```

`cargo ax run start` serves the same Axonyx app routes and public assets without injecting the dev live-reload client. On a host such as Render, use the platform `PORT` value in the start command.

## Runtime Source Defaults

The default scaffold flow uses `--runtime-source registry`.

- `registry`
  - published crates.io packages; best default for current public use
- `path`
  - best for Axonyx contributors working inside the framework repo
- `git`
  - test an unreleased runtime branch

```bash
create-axonyx my-app --yes
```

## First Useful Variants

Minimal starter:

```bash
create-axonyx my-app --yes --template minimal
```

Site starter:

```bash
create-axonyx my-site --yes --template site
```

Docs starter:

```bash
create-axonyx my-docs --yes --template docs
```

## What You Get

Generated apps currently include:

- `app/` for `.asx` pages, layouts, and components, with route-local `.ax` backend modules
- `routes/` for route-style backend authoring
- `jobs/` for scheduled or background-style backend authoring
- `src/generated/` for generated backend Rust output
- `src/db/` and `src/domain/` as early integration seams

## Route Boundaries

Generated apps include optional framework-native boundary pages:

- `app/not-found.asx` renders with status `404` when no `app/**/page.asx` route matches.
- `app/error.asx` renders with status `500` when a matched route fails during rendering.

Both files are normal `.asx` pages and are wrapped by `app/layout.asx`, so the site
keeps the same shell even when a route is missing or a render error happens.

## Next Step To Close Core

After the first run loop works, use the proof checklist to verify the full framework story:

- [Proof App Checklist](./proof-app-checklist.md)

That checklist is the fastest path to confirm that your project is not only scaffolded, but
also aligned with the current AX v2 route, import, loader/action, and dev-server flow.
