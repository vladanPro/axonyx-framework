# Structured Async In Axonyx

Status: architecture principle
Date: 2026-05-12

## Principle

Axonyx uses an async Rust runtime internally, but the Axonyx language should remain declarative and sync-looking for application authors.

This is intentional.

The framework should not expose developers to the JavaScript-style async timing problem where every feature becomes a hand-written mix of `await`, loading flags, race conditions, and "which millisecond did this finish in?" reasoning.

Axonyx should provide **structured async**:

```text
Rust runtime: async
Axonyx syntax: declarative and stable
```

## Why This Matters

The move from the current beta `std::net` server toward Hyper/Tokio is a runtime implementation detail. It should not force users to write async orchestration everywhere.

Axonyx should make common async categories explicit:

- data before render -> `loader`
- user mutations -> `action`
- temporary UI state -> `signal`
- slow render segments -> `<Await>`
- heavy/background work -> `job`

This is the rail system. Users place work on the correct rail, and the runtime decides how to execute it.

## User-Facing Shape

Instead of JavaScript-style code:

```ts
const data = await fetch(...)
setLoading(false)
```

Axonyx should prefer:

```ax
loader posts {
  return Posts.all()
}

page Blog {
  <Await data={posts}>
    <PostList posts={posts} />
  </Await>
}
```

For normal server-rendered routes, even `<Await>` should often be unnecessary:

```ax
loader post {
  return Blog.find(params.slug)
}

<Card title={post.title}>
  <Markdown value={post.body} />
</Card>
```

The lowering/runtime can decide:

- loader becomes async Rust internally
- page render waits for the loader for normal server-rendered routes
- stream boundaries are used only where declared
- state updates return typed patches
- users do not manually orchestrate timing

## State Machine Rule

Axonyx should keep async behavior out of ad-hoc user code by assigning each kind of work to one of a few primitives.

1. Server-owned data goes through `loader` and `action`.
2. UI-owned temporary state goes through `signal`.
3. Slow or streaming data goes through `<Await>`.
4. Background and heavy work goes through `job`.
5. The user should not manually orchestrate promise timing.

Example:

```ax
state theme = signal("silver")

loader user {
  return Auth.user()
}

action saveProfile(form) {
  return Profile.update(user.id, form)
}

<If condition={user}>
  <ProfileForm user={user} action={saveProfile} />
</If>
```

Under the hood:

- `Auth.user()` may be async
- `Profile.update()` may be async
- the form has pending/success/error lifecycle
- the browser receives a patch or redirect
- the `.ax` code remains declarative

## Runtime Categories

### Loader

Loaders fetch server-owned data before render.

```ax
loader product {
  return Products.find(params.slug)
}
```

Lowering direction:

```text
loader product
-> async Rust function
-> RequestContext injection
-> result made available to route render
```

### Action

Actions handle mutations, forms, uploads, login, and other request-driven changes.

```ax
action submitComment(form) {
  return Comments.create(form)
}
```

Lowering direction:

```text
action submitComment
-> async Rust function
-> form/body parser
-> action lifecycle result
-> redirect, error, or patch
```

### Signal

Signals represent UI-owned temporary state.

```ax
state count = signal(0)

<Button on:click={count += 1}>
  Count: {count}
</Button>
```

Lowering direction:

```text
signal
-> SignalId
-> DOM marker
-> event binding table
-> typed patch
```

### Await

`<Await>` marks an explicit streaming/deferred boundary.

```ax
<Await data={posts}>
  <PostList posts={posts} />
</Await>
```

Lowering direction:

```text
<Await>
-> stream boundary
-> async segment
-> chunked response segment
```

### Job

Jobs run heavy or background work outside the request's critical path.

```ax
job OptimizeImage(input: FileRef) {
  capability: "image.process"
  run {
    Process.exec("magick", [input.path, "-resize", "1200x", input.out])
  }
}
```

Lowering direction:

```text
job
-> JobRegistry entry
-> worker runtime
-> capability check
-> structured logs/events
```

## Server-Net Implication

`axonyx-server-net` should be async internally because the runtime needs:

- streaming HTML
- upload/download streaming
- auth/session IO
- database IO
- jobs and process orchestration
- WebSockets/SSE later
- middleware and cache hooks

But the application author should mostly write:

```ax
loader
action
signal
Await
job
```

not raw async orchestration.

## Public Wording

Good wording:

> Axonyx runs on an async Rust runtime, but its language model is structured and declarative. Developers place work into loaders, actions, signals, stream boundaries, and jobs instead of hand-orchestrating promise timing.

Avoid wording:

> Axonyx makes users write async functions everywhere.

## Design Rule

If a feature requires the user to think about race conditions, loading flags, cancellation, and exact timing for a normal case, the framework has failed that abstraction.

The runtime may be async.

The Axonyx authoring model should feel stable.

