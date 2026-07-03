---
id: gtr-4h2
title: Remove .replace('\"', "") workaround from error formatting in main()
status: blocked
type: task
priority: 2
tags:
- code-quality
- error-handling
created: 2026-06-08
updated: 2026-07-02
blocks:
- gtr-3dm
blocked_by:
- gtr-6ug
- gtr-bmr
phase: ''
---

# Remove .replace('\"', "") workaround from error formatting in main()

main() calls `err.to_string().replace('"', "")` to strip stray quote characters
from error messages. This is a code smell: Display impls should not emit literal
quotes in normal error text. Using {err} (Display) rather than {err:?} (Debug)
avoids unwanted quotes. Investigate the error constructors and fix Display
formatting there; then remove the replace() call.
