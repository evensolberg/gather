---
id: gtr-wek
title: Add file existence pre-validation before the processing loop
status: closed
type: feature
priority: 3
tags:
- ux
- validation
created: 2026-06-08
updated: 2026-07-03
phase: ''
---

# Add file existence pre-validation before the processing loop

Missing source files are discovered one-by-one inside the processing loop, meaning
errors surface mid-run after some files may already have been moved/copied.
Add a pre-flight pass: check all source paths with Path::exists() upfront and either
abort early (if stop_on_error) or print all missing paths before doing any work.
This gives users a complete error picture before any destructive moves begin.

[start] 2026-07-03 16:16:54
