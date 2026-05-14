# Getting Started

This is the practical starting point for Axonyx development today.

## Local Scaffold

From the framework repo:

```bash
git submodule update --init --recursive
cargo run -p create-axonyx -- my-app --yes
```

By default the generated app uses the shared `axonyx-runtime` Git repository, so regular users do not need the framework submodule setup after scaffolding.

Then:

```bash
cd my-app
cargo run
```

If `cargo-axonyx` is installed, the first framework-shaped local loop is:

```bash
cargo ax check
cargo ax doctor
cargo ax schema pull ./sample-posts.json --name Post
cargo ax content
cargo ax build
cargo ax run dev
```

`cargo ax build` regenerates `src/generated/backend.rs` from:

- `app/**/loader.ax`
- `app/**/actions.ax`
- `routes/**/*.ax`
- `jobs/**/*.ax`

`cargo ax run dev` now runs that backend sync once before starting the local route-aware dev server with live reload polling.

`cargo ax doctor` checks the app shape, runtime dependency, UI package wiring, package CSS, and `.ax` diagnostics before you start chasing browser issues.

`cargo ax content` indexes configured content collections, which is the first filesystem/content layer for future docs, blog, and CMS flows.
`cargo ax build` writes that manifest to `dist/_ax/content/manifest.json` when collections are configured.
Route loaders can now read configured content collections:

```ax
loader DocsList
  data docs = Content.Collection("docs")
    order slug asc
  return docs
```

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

Axonyx now has an early typed data path for JSX-like `.ax` files. Define a record shape, bind loader data to a typed list, and `cargo ax check` can catch wrong field access before render:

```ax
page Blog

type Post {
  title: String
  slug: String
  excerpt: String
  summary?: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.title}>
    <Copy>{post.excerpt}</Copy>
  </Card>
</Each>
```

If the page uses `post.summary` instead of a declared field, `cargo ax check` reports an `axonyx-type` diagnostic. This is the first bridge between `.ax` primitives like `String` / `List<Post>` and Rust-side Axonyx types.

For intentionally optional data, use safe member access:

```ax
<Copy>{post?.summary}</Copy>
```

If `summary` is missing at runtime, it lowers to an empty string instead of failing the render.
If a field is optional in the type itself, regular access is allowed and resolves to `Optional<T>`:

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

The default scaffold flow now uses `--runtime-source git`.

- `git`
  - best default for current public use
- `path`
  - best for Axonyx contributors working inside the framework repo
- `registry`
  - best once the runtime crates are published

```bash
cargo run -p create-axonyx -- my-app --yes
```

## First Useful Variants

Minimal starter:

```bash
cargo run -p create-axonyx -- my-app --yes --template minimal
```

Site starter:

```bash
cargo run -p create-axonyx -- my-site --yes --template site
```

Docs starter:

```bash
cargo run -p create-axonyx -- my-docs --yes --template docs
```

## What You Get

Generated apps currently include:

- `app/` for `.ax` UI authoring
- `routes/` for route-style backend authoring
- `jobs/` for scheduled or background-style backend authoring
- `src/generated/` for generated backend Rust output
- `src/db/` and `src/domain/` as early integration seams

## Next Step To Close Core

After the first run loop works, use the proof checklist to verify the full framework story:

- [Proof App Checklist](./proof-app-checklist.md)

That checklist is the fastest path to confirm that your project is not only scaffolded, but
also aligned with the current AX v2 route, import, loader/action, and dev-server flow.
