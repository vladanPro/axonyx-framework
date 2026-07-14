---
title: Hello from the Axonyx workbench
description: Why this journal starts with static HTML, Markdown, and a Rust-powered build.
date: 2026-07-15
category: Field note
reading_time: 3 min read
---
# Hello from the Axonyx workbench

This journal starts with a deliberately small publishing loop: write Markdown,
run one Cargo command, and ship static HTML.

## Why start static?

Static pages are easy to cache, easy to host, and easy to understand. When the
site later needs actions, a database, or live state, Axonyx can add those layers
without making the first article pay for them.

- content stays in version control
- routes stay visible in the filesystem
- the output remains portable
