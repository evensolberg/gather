---
id: gtr-bmr
title: Replace err.to_string().replace in main() with eprintln!("{err}")
status: open
type: task
priority: 3
tags:
- code-quality
- main
- error-handling
created: 2026-06-13
updated: 2026-06-13
phase: ''
---

# Replace err.to_string().replace in main() with eprintln!("{err}")

The .replace(", "") in main.rs:142 is a pre-existing dead strip — no current error string contains double-quotes — and would silently corrupt any future quoted error message. Fix: change eprintln!("{}", err.to_string().replace(", "")) to eprintln!("{err}"). Identified during final code review of PR #78 (gtr-ar3).
