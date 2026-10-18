# Private Binary Routes - 0.7.0 Release Candidates

| Package | Candidate |
| --- | --- |
| axonyx-core | 0.7.0 |
| axonyx-runtime | 0.7.0 |
| cargo-axonyx | 0.7.0 |
| create-axonyx | 0.7.0 |
| axonyx-lsp | 0.7.0 |

Candidate versions are not publication confirmation. Macros, UI, editor and the
internal state WASM package are unchanged. There is no new WASM/browser ABI.

## Why A New Compatibility Line

Core lowering/codegen and runtime API-validation public enums have new private
Binary cases. Publishing this as a compatible 0.6.x patch would break old
tooling's exhaustive Rust matches. Existing 0.6.x consumers retain their old
dependency line; native Rust consumers moving to 0.7 must handle the new cases.
Backend Binary is a reserved HTTP response marker, not a user record/data type.

Explicit GET -> Binary routes use Native.privateResponse and a separately bounded
session-verified Rust callback. Compiled response policy always enforces private
headers and redacted errors, with binary validation by reference when enabled.
JSON/native callback budgets stay separate. Preview refuses compiled-only binary
routes. See ../private-binary-cli-v0.md for authoring and supported MIME/methods.

CLI check, session discovery, API/Melt reports, hashes and OpenAPI recognize this
contract. Empty/nested native object/array payloads also compile correctly.
Runtime recommendations in upgrade/Doctor and scaffolder defaults move together
with workspace dependency pins to 0.7.0. LSP is rebuilt against the same new core.

## Promotion And Registry Proof

Runtime PRs 264/265/266, framework PR 367 and Legura PR 64 passed their required
source gates. The latter proves actual private gallery image decoding, auth/CSRF,
current authority, scoped uploads, JS/no-JS and archive/recovery on exact source
pins. These results do not substitute for new registry-package verification.

Candidate feature PRs target dev. Merge only after required CI, then use separate
dev -> main release PRs and require green main CI. Publish core first; after
crates.io propagation verify/package/publish runtime. Only then verify/package
CLI, scaffolder and LSP against registry 0.7.0 and publish them. Do not bypass
verification with --no-verify to work around an unpublished dependency.

Fresh isolated installs must scaffold and run representative templates with
registry runtime 0.7.0 and the installed CLI. Legura must replace its temporary Git
pins in root/nested manifests and CI, regenerate both lockfiles without unrelated
upgrades and pass full isolated Media/ScopedMedia/MediaBrowser acceptance again,
without RuntimeSource or ToolManifest overrides. Only actual publication plus
consumer proof justifies package-scoped tags and GitHub releases.

No global CLI, operator CMS/DB/configuration, deployed site, Docker image, npm
package or UI/editor release is updated automatically. Public post images,
attachments, repair UI, thumbnails, streaming/ranges and aggregate process-memory
protection remain separate tasks.
