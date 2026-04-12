# Backend Authoring

Axonix backend authoring is moving toward a framework-native shape instead of a thin copy of another ecosystem.

## Current Top-Level Blocks

- `route`
- `loader`
- `action`
- `job`

## Example

```ax
loader PostsList
  data posts = Db.Stream("posts")
    where status = "published"
    order created_at desc
    limit 6
  return posts
```

## Current Query Clauses

- `where`
- `order`
- `limit`
- `offset`

## Current Mutation Steps

- `insert`
- `update`
- `delete`
- `revalidate`
- `return`
- `send`

## Runtime Contract

Generated backend handlers lower into runtime request types such as:

- `AxQueryRequest`
- `AxInsertRequest`
- `AxUpdateRequest`
- `AxDeleteRequest`
- `AxSendRequest`

That separation is important:

- `.ax` authoring owns developer ergonomics
- lowering owns execution shape
- runtime owns environment and transport behavior

## Env Convention

Examples:

- `Runtime.Env.public.app_name` -> `AX_PUBLIC_APP_NAME`
- `Runtime.Env.secret.db_url` -> `AX_SECRET_DB_URL`
- `Runtime.Env.secret.db_dialect` -> `AX_SECRET_DB_DIALECT`
- `Runtime.Env.secret.db_transport` -> `AX_SECRET_DB_TRANSPORT`

For deeper draft details, see:

- [Reactivity v1](./reactivity-v1.md)
- [IR v1](./ir-v1.md)
