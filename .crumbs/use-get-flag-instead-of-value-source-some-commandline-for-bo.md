---
id: gtr-llt
title: Use get_flag() instead of value_source() == Some(CommandLine) for boolean flags
status: closed
type: task
priority: 1
tags:
- code-quality
- clap
- idiomatic
created: 2026-06-08
updated: 2026-06-09
phase: ''
---

# Use get_flag() instead of value_source() == Some(CommandLine) for boolean flags

main.rs repeats `cli_args.value_source("flag") == Some(ValueSource::CommandLine)` six
times for SetTrue flags. Clap 4's idiomatic API is `cli_args.get_flag("name")`, which
is cleaner, shorter, and does not require importing ValueSource.

Affected flags: move, stop, dry-run, summary, detail-off, quiet.

Fixed manually on 2026-06-09.
