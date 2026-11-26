# Data types and execution boundaries

A type describes which values a page, loader, action, or API accepts. It does
not decide where an expression runs. A page can render a typed loader result
on the server without sending it to the browser as reactive state.

## Authoring types

| Group | Types |
| --- | --- |
| Scalars | `String`, `Bool`, `Number`, `Int`, `Float`, `Decimal` |
| Date and identity | `Date`, `Time`, `DateTime`, `Uuid` |
| Collections | `List<T>` (`T[]`), `Map<K, V>`, `Set<T>` |
| Optional and result | `Optional<T>` (`T?`), `Result<T, E>` |
| Structured data and files | Named records, string-literal unions, `Json`, `Bytes`, `File`, `FileRef` |
| Special contracts | `Void`, `Never`, `Unknown` |
| Context-specific wrappers | `Secret<T>`, `Public<T>`, `Signal<T>`, `Resource<T, E>` |

String-literal unions such as `type Status = "draft" | "published"` are
supported. General unions such as `String | Number` are not. For a shared record
contract, export the type in the loader module and use it in the page binding.
See [Typed Data and Each](getting-started.md#typed-data-and-each).

## What crosses each boundary?

| Boundary | What to expect |
| --- | --- |
| Compiled Rust contracts | Records lower to Rust structs. Lists become `Vec<T>`, optional values become `Option<T>`, and results become `Result<T, E>`. Compiled maps and sets require supported ordered keys or items. `Decimal` preserves exact decimal text through `AxDecimal`; it is not a floating-point type. |
| API response | When response validation is enabled, it checks the declared shape, including record fields, optional values, and string unions. `File` is a server-only multipart input; `FileRef` is serializable metadata, not the file contents. Secrets do not cross this boundary. |
| Browser state | The browser host validates supported scalar and structured values against state types. Use browser-safe integers for `Int`. Do not treat a server-side `Decimal`, a file, or a secret as a supported browser state value just because the declaration parser recognizes the type. |
| WASM state executor | Supported local operations include numeric set/add/subtract, boolean set/toggle, and string set. Structured values such as records, lists, maps, sets, optionals, results, JSON and bytes can use the binary bridge for set. A separate evaluator handles supported reactive expressions, including member reads. The browser host still handles DOM/events and can handle supported fallback paths. |

## Why setting a value is not the same as operating on it

Passing a `List<Post>` through the WASM bridge lets the browser host validate
its declared shape and the executor validate and read its binary frame. It
does not automatically provide a client-side sorting API. To add one, Axonyx
must define its ASX semantics, check its types, lower it to a client operation,
and implement and test that operation.

More client operations can be added without changing an existing `Post`
contract. For example, sorting public posts already in browser state could run
locally; fetching another page from the database or authorizing a write still
belongs on the server. Adding WASM support is not a reason to move credentials,
storage handles, or server-only domain logic to the browser.
