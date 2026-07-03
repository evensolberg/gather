---
id: gtr-bmr
title: Replace err.to_string().replace in main() with eprintln!("{err}")
status: closed
type: task
priority: 3
tags:
- code-quality
- main
- error-handling
created: 2026-06-13
updated: 2026-07-02
closed_reason: 'Done in PR #81. Dead replace() removed; eprintln!("{err}") used directly.'
blocks:
- gtr-4h2
phase: ''
---

# Replace err.to_string().replace in main() with eprintln!("{err}")

The .replace(", "") in main.rs:142 is a pre-existing dead strip — no current error string contains double-quotes — and would silently corrupt any future quoted error message. Fix: change eprintln!("{}", err.to_string().replace(", "")) to eprintln!("{err}"). Identified during final code review of PR #78 (gtr-ar3).

[start] 2026-07-02 21:15:30

[stop]  2026-07-02 21:42:17  26m 47s  Resolved as part of gtr-6ug: replace() was dead code, removed in the same PR #81.
