# Interactive State V1

Axonyx Pages can perform small local interactions through compiler-owned event
metadata. Authors do not need inline JavaScript, a virtual DOM, or a server
round trip for these state changes.

```asx
page Counter() {
  state count: Number = 0
  state open: Bool = false

  return ASX {
    <>
      <Button on:click={count += 1}>Increase</Button>
      <Copy>{count}</Copy>

      <If when={count > 5}>
        <Badge>High value</Badge>
      </If>

      <Button on:click={open = !open}>Toggle</Button>
    </>
  }
}
```

Supported local event forms in V1:

```asx
on:click={count += 1}
on:click={count -= 1}
on:click={open = !open}
on:click={mode = "compact"}
on:input={query = event.value}
on:change={enabled = event.checked}
```

The event target must be declared `state`. Numeric operations require
`Number`, toggle requires `Bool`, and literal assignment must match the state
type. Arbitrary browser expressions are rejected instead of being emitted as
`eval` or inline handlers.

Simple state reads such as `<Copy>{count}</Copy>` become DOM bindings. An
`<If>` whose condition directly reads one state signal keeps stable then/else
DOM branches and updates only their visibility when that signal changes.

Server actions remain the correct boundary for database writes, authorization,
validation, and trusted business logic. The local event runtime is intended for
UI state such as counters, disclosure controls, tabs, filters, and form input.

The browser receives declarative `data-ax-on-*` metadata. The Axonyx state
bridge executes the small operation set through `SignalId` handles and typed
values; it never exposes Rust pointers and does not execute source strings.
