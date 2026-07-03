---
id: gtr-jz8
title: Replace Box<dyn Error> with anyhow::Error for richer error context
status: closed
type: feature
priority: 2
tags:
- error-handling
- anyhow
- code-quality
created: 2026-06-08
updated: 2026-07-03
phase: ''
---

# Replace Box<dyn Error> with anyhow::Error for richer error context

run() uses Box<dyn Error> as its return type. The anyhow crate provides anyhow::Error
which adds .context() / .with_context() for attaching human-readable context to errors
at each call site, making CLI error messages far more actionable.
Add `anyhow = "1"` to Cargo.toml and replace Box<dyn Error> with anyhow::Error;
swap .into() for .context(...).
