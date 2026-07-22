# {{APP_NAME}}

Generated with `create-axonyx` using the static `site` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is intentionally small: a product/company homepage, an about page,
and a contact page. It has no database, API routes, background jobs, or action
runtime. Foundry UI and theme preflight are already configured.

## Routes

- `/` - landing page
- `/about` - company story and values
- `/contact` - static contact call to action

## Develop

Start the local server:

```bash
cargo ax run dev
```

Before sharing or deploying:

```bash
cargo ax check
cargo ax doctor
cargo ax test
cargo ax build --clean
```

Deploy the generated `dist/` directory to any static host, or run the Axonyx
server with `cargo ax run start --host 0.0.0.0 --port 3000`.

## First edits

1. Replace the copy in `app/page.ax`.
2. Update `public/brand-mark.svg` and `public/favicon.svg`.
3. Replace `hello@example.com` in `app/contact/page.ax`.
4. Choose the default theme in `app/layout.ax`.
