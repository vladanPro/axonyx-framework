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
query loadPosts() -> Post[] {
  data posts = db.posts.all()
    where status = "published"
    order created_at desc
    limit 6
  return posts
}
```

Pages can consume route-local query functions without a manual API fetch:

```ax
page Posts() -> ASX {

data posts = loadPosts()

return {
<Each items={posts} as="post">
  <Card title={post.title} />
</Each>
}
}
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
query loadPublishedPosts() -> Post[] {
  data posts = db.query("select * from posts where status = ?", "published")
  return posts
}
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
- `transaction`
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
- `AxTransactionRequest`
- `AxSendRequest`

That separation is important:

- `.ax` authoring owns developer ergonomics
- lowering owns execution shape
- runtime owns environment and transport behavior

## Atomic Transactions

Use `transaction {}` when several database writes must either all succeed or
all roll back:

```ax
action publishPost(id: String, title: String) {
  transaction {
    db.posts.where({ id: input.id }).update({
      title: input.title,
      status: "published"
    })
    db.audit.insert({
      post_id: input.id,
      event: "post.published"
    })
    db.drafts.where({ post_id: input.id }).delete()
  }

  return ok()
}
```

Transaction V1 rules:

- the block must contain at least one operation
- only `insert`, `update`, and `delete` are allowed inside the block
- expressions are evaluated before preview data is changed
- SQLite uses one immediate transaction and Postgres uses one pooled client transaction
- every touched resource participates in normal action invalidation
- direct SQLite and Postgres transports are supported
- API transport, MySQL, and the memory adapter fail explicitly instead of pretending to be atomic

Do not place redirects, state patches, messages, or external side effects inside
the transaction. Perform those after the block succeeds. Nested transactions
and raw SQL transaction steps are intentionally outside the V1 contract.

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

## Database Check

Use `cargo ax db check` to verify the active database contract from `.env`,
`.env.local`, and the shell environment.

SQLite introspection is available first:

```env
AX_SECRET_DB_URL=sqlite://data/app.db
AX_SECRET_DB_DIALECT=sqlite
AX_SECRET_DB_TRANSPORT=direct
```

For Supabase/Postgres, use the database connection string, not the Supabase
dashboard URL:

```env
AX_SECRET_DB_URL=postgresql://postgres:<password>@db.<project-ref>.supabase.co:5432/postgres
AX_SECRET_DB_DIALECT=postgres
AX_SECRET_DB_TRANSPORT=direct
DB_POOL_MAX_SIZE=10
DB_POOL_TIMEOUT_MS=5000
```

Current Postgres behavior:

- Direct runtime queries use a lazy, process-shared connection pool.
- Compiled Axum requests execute synchronous rendering and database work on
  Tokio's blocking pool instead of occupying async network workers.
- `DB_POOL_MAX_SIZE` controls the maximum open connections; the default is `10`.
- `DB_POOL_TIMEOUT_MS` controls how long checkout waits; the default is `5000`.
- `cargo ax db check` validates the connection and lists visible tables.
- The printed URL is redacted before display.
- The same contract works with local PostgreSQL and hosted providers such as Supabase.

## Database Schema Pull

Use `cargo ax db pull` to write the current database schema snapshot to:

```text
.axonyx/db/schema.json
```

Current behavior:

- SQLite and Postgres/Supabase table, view, and column introspection is supported.
- The JSON manifest includes a deterministic schema hash, raw SQL types, mapped
  Axonyx types, and generated row contract names.
- `app/generated/db.ax` is generated beside the manifest by default.
- `cargo ax check` rejects unknown `db.*` resources, unknown fields in
  `where`/`order`/`insert`/`update`, and writes through read-only views.
- `cargo ax db check` compares the live schema hash with the pulled manifest and
  fails when they differ.
- Existing schema output is overwritten, so rerun the command after changing the database.

Example:

```bash
cargo ax db pull
cargo ax db pull --out .axonyx/db/schema.json
```

Example generated contract:

```ax
export type PostsRow {
  id: Int
  title: String
  summary: Optional<String>
  published_at: Optional<DateTime>
}

export type PostsCreateInput {
  id: Optional<Int>
  title: String
  summary: Optional<String>
  published_at: Optional<DateTime>
}

export type PostsUpdateInput {
  id: Optional<Int>
  title: Optional<String>
  summary: Optional<String>
  published_at: Optional<DateTime>
}
```

Use the generated row type in loader/action return contracts rather than
duplicating the database shape by hand:

```ax
export query loadPosts() -> List<PostsRow> {
  return db.posts.where({ status: "published" }).all()
}
```

The JSON manifest is the authoritative resource catalog. The generated `.ax`
file makes those rows available to the existing Axonyx type and API contract
pipeline. Commit both generated artifacts so CI and production builds stay
deterministic without live database credentials. Raw `db.query()` remains the
explicit escape hatch for database names that cannot be represented by the
Axonyx DSL.

For mutations, `cargo ax check` also verifies literal and direct `input.*`
values against pulled column types. Inserts must provide every non-null column
without a database default. Nullable/defaulted columns and SQLite integer
primary keys remain optional, while every generated update field is optional.
Complex runtime expressions stay allowed until their type can be proven by the
compiler; Axonyx does not guess and emit false-positive errors.

## Database Migrations

Axonyx V1 migrations are ordered SQL pairs owned by the application. Configure
their directory in `Axonyx.toml`:

```toml
[db]
migrations = "db/migrations"
```

Create a migration:

```bash
cargo ax db migration create create_posts
```

This creates one immutable migration directory:

```text
db/migrations/
  20260901123456_001_create_posts/
    up.sql
    down.sql
```

Both files must contain executable SQL. Axonyx owns the transaction boundary,
so migration files must not contain `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`,
or `RELEASE` statements.

Use the safe inspection and execution flow:

```bash
cargo ax db status
cargo ax db status --format json
cargo ax db migrate --dry-run
cargo ax db migrate
cargo ax db rollback --dry-run
cargo ax db rollback
```

`status` and both dry-run commands are read-only. The first successful apply
creates `_axonyx_migrations`; applying SQL and recording its version/checksum
happen in the same database transaction. Rollback can only remove the latest
applied migration and also runs atomically.

The default profile loads `.env` followed by `.env.local`. Named profiles load
`.env` followed by `.env.<name>`, before shell values are applied:

```bash
cargo ax db status --env staging
cargo ax db migrate --env prod --dry-run
cargo ax db migrate --env prod --confirm
```

Production apply and rollback commands refuse to run without `--confirm`.
Axonyx prints the target driver and redacted URL, rejects missing/out-of-order
migration files, and fails if an already-applied checksum differs from disk.

V1 supports persistent direct SQLite files and Postgres transports. Ephemeral
SQLite `:memory:` databases are rejected because migration history would vanish
with the connection. API transport migrations, schema diff generation, seeds,
and multi-step rollback are deliberately not claimed yet.

For deeper draft details, see:

- [Reactivity v1](./reactivity-v1.md)
- [IR v1](./ir-v1.md)
