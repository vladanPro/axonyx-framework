# Compound Request Guards And Rust 1.82 - 0.7.1 Candidates

Core, runtime, CLI, scaffolder and LSP candidates are 0.7.1. The version table,
CLI upgrade/Doctor recommendation and scaffold registry runtime default move
together. Preparation does not mean these packages are already published.
Macros, UI/editor and internal state WASM remain unchanged.

This patch delivers runtime PRs 269/270 and framework PR 371: compound request
members and Auth.subject are resolved through server context, membership permits
fallible operands, literal text and logical short-circuiting are preserved.
The HTTP regression verifies denial before session creation and real session,
query/parameter/header guards without an intermediate binding workaround.

Fresh Rust 1.82 resolution is protected by the runtime's Tokio <1.54 bound,
including default consumers, and a new fresh-lockfile default/Tokio/Axum check.
It does not raise the declared Rust minimum or add a new public enum/browser ABI.
Missing request fields keep their existing error contract.

Candidate PRs target dev and require their mandatory CI. Promote using separate
dev-to-main release PRs, then require green main CI before publishing verified
core -> runtime -> tooling packages. Runtime packaging must use the actual
published core, not an unpublished dependency workaround or no-verify bypass.

Verify fresh registry tool installs, all four scaffold build/HTTP paths and a
compiled request-guard regression against registry dependencies before creating
package-scoped tags/releases. No global CLI, operator CMS/DB/configuration,
production site, Docker image, npm package or Marketplace extension is changed
automatically. This is a scoped patch, not a production-readiness claim.
