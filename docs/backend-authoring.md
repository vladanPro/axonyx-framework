# Backend Authoring

Axonyx backend authoring is moving toward a framework-native shape instead of a thin copy of another ecosystem.

## Current Top-Level Blocks

- `route`
- `loader`
- `query`
- `action`
- `job`

## Example

```ax
query loadPosts() -> Post[]
  data posts = db.posts.all()
    where status = "published"
    order created_at desc
    limit 6
  return posts
```

Pages can consume route-local query functions without a manual API fetch:

```ax
page Posts

data posts = loadPosts()

<Each items={posts} as="post">
  <Card title={post.title} />
</Each>
```

`loader PostsList` and `load PostsList` remain supported for compatibility, but new templates prefer `query loadPosts()` and `data posts = loadPosts()`.

## Current Query Clauses

- `where`
- `order`
- `limit`
- `offset`

## Raw SQL Escape Hatch

Use `db.<table>.all()` for normal reads. When Axonyx does not have the query
shape yet, use `db.query(...)` with a SQL string and variadic parameters:

```ax
loader PublishedPosts
  data posts = db.query("select * from posts where status = ?", "published")
  return posts
```

Current v0 rules:

- backend-only
- SELECT/WITH statements only
- parameters are passed separately; do not concatenate user input into SQL
- errors still pass through the Axonyx DB error translator

## Current Mutation Steps

- `insert`
- `update`
- `delete`
- `patch`
- `revalidate`
- `return`
- `send`

## Runtime Contract

Generated backend handlers lower into runtime request types such as:

- `AxQueryRequest`
- `AxRawSqlRequest`
- `AxInsertRequest`
- `AxUpdateRequest`
- `AxDeleteRequest`
- `AxSendRequest`

That separation is important:

- `.ax` authoring owns developer ergonomics
- lowering owns execution shape
- runtime owns environment and transport behavior

## Action Patch Protocol

Actions can now emit state patches for progressive interactivity:

```ax
action SetTheme
  input:
    theme: string

  patch theme = input.theme
  return ok
```

When a form/action request sends `Accept: application/ax-patch+json` or
`__ax_patch=1`, the dev server returns:

```json
{
  "ok": true,
  "redirect": "/",
  "patches": [
    { "op": "set", "signal": "root:theme:1", "value": "gold", "source": "action" }
  ]
}
```

The browser can pass each patch to `window.__axonyx.state.applyPatch(...)`.
For the current V1 contract, a simple identifier such as `theme` lowers to
`root:theme:1`. Explicit signal strings such as `patch "root:theme:2" = value`
remain available as an escape hatch until The Melt owns a full cross-file signal
binding table.

When a rendered page contains a form whose `action` points at
`/__axonyx/action`, Axonyx injects a small action runtime. It submits the form as
`application/ax-patch+json`, adds `__ax_patch=1`, applies returned patches through
`window.__axonyx.state.applyPatch(...)`, and falls back to redirect navigation
when no patches are returned.

Patch responses are validated against the route's current state manifest when
the signal is known. For example, a patch targeting `state count: Number = 0`
must return a numeric patch value instead of a string.

## Env Convention

Examples:

- `Runtime.Env.public.app_name` -> `AX_PUBLIC_APP_NAME`
- `Runtime.Env.secret.db_url` -> `AX_SECRET_DB_URL`
- `Runtime.Env.secret.db_dialect` -> `AX_SECRET_DB_DIALECT`
- `Runtime.Env.secret.db_transport` -> `AX_SECRET_DB_TRANSPORT`

For deeper draft details, see:

- [Reactivity v1](./reactivity-v1.md)
- [IR v1](./ir-v1.md)
