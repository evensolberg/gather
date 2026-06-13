---
id: gtr-b6p
title: quiet mode silently defeats --print-summary/-p flag
status: in_progress
type: bug
priority: 3
tags:
- ux
- cli
- logging
created: 2026-06-12
updated: 2026-06-12
phase: ''
---

# quiet mode silently defeats --print-summary/-p flag

When both -q (quiet) and -p (print-summary) are passed, the summary is silently suppressed because LevelFilter::Error filters log::info! calls. The flags are not mutually exclusive at the clap level. Consider either: (A) making -p override quiet for summary output specifically, or (B) writing summary directly to stdout bypassing the logger, or (C) adding a conflict_with("quiet") on the summary arg.

[start] 2026-06-12 21:37:14  investigating quiet vs print-summary conflict
