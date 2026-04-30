# Proof App Checklist

Use this checklist to confirm that an Axonyx app follows the current core framework path.

## 1) Scaffold

Create a fresh app:

```bash
cargo run -p create-axonyx -- proof-app --yes --template site
```

Expected:

- app is generated without manual file fixes
- `app/layout.ax` and `app/page.ax` exist
- template README mentions AX v2 direction

## 2) Run Loop

From app root:

```bash
cargo run
cargo ax run dev
```

Expected:

- preview build succeeds
- dev server starts on `http://127.0.0.1:3000`
- page renders through layout composition

## 3) AX v2 Page Edit

Edit `app/page.ax` and change visible copy.

Expected:

- no parser downgrade required
- route render updates after refresh
- diagnostics stay actionable if an intentional syntax error is introduced

## 4) Nested Route

Add `app/docs/page.ax` and open `/docs`.

Expected:

- nested route resolves
- nearest layouts apply in expected order

## 5) Dynamic Route Params

Add `app/posts/[slug]/page.ax` and open `/posts/hello-axonyx`.

Expected:

- route resolves
- `params.slug` is visible in rendered output

## 6) Query Context

Open `/posts/hello-axonyx?status=draft`.

Expected:

- page/backend flow can read query context where used

## 7) Local Component Import

Create `app/components/HeroCard.ax`, import it in `app/page.ax`.

Expected:

- import resolves
- rendered output shows component body

## 8) UI Package Import

Import one component from `@axonyx/ui/...`.

Expected:

- package import resolves
- component renders without exposing import internals in output

## 9) Backend Generation

Run:

```bash
cargo ax build
```

Expected:

- `src/generated/backend.rs` updates
- loader/actions/routes/jobs sources are discovered correctly

## 10) Optional Override Check

Add a temporary component or package override in `Axonyx.toml`.

Expected:

- override path resolves
- invalid target emits clear diagnostics with source context

## Done Criteria

Treat the app as a successful core proof when all checks pass without custom patches to
framework internals.
