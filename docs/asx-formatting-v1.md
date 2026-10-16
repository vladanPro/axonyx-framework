# Readable ASX formatting V1

The compiler-owned formatter expands complete, compact element-only containers:

```asx
<Card><Copy>Ready</Copy><Button>Open</Button></Card>
```

becomes:

```asx
<Card>
  <Copy>Ready</Copy>
  <Button>Open</Button>
</Card>
```

Leaf content stays compact. Mixed prose, explicit sibling spaces, pre/code/raw
content and embedded JS/style blocks are not rewritten. Expansion is accepted
only if the original and reformatted markup parse to identical ASX ASTs.
Formatting is idempotent and does not change expressions or attribute values.

Both `cargo ax fmt` and LSP `textDocument/formatting` call this same core function.
VS Code already routes Shift+Alt+F and optional format-on-save through the LSP,
falling back to the CLI if unavailable. No separate extension formatter engine
or Prettier dependency is introduced. The extension's final offline fallback is
still indentation-only.

This improvement requires the updated LSP/CLI binaries; publishing a VSIX alone
cannot update an installed Rust formatter. Reload the editor after updating the
toolchain. To enable saving-time formatting:

```json
{
  "[ax]": {
    "editor.defaultFormatter": "vladanpro.axonyx-vscode",
    "editor.formatOnSave": true
  }
}
```

V1 does not automatically wrap long attribute lists or complete inline function
bodies. Existing multiline attributes are indented by the existing formatter.
These are future layout rules, not claims of full Prettier parity.
