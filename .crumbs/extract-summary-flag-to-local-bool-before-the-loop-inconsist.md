---
id: gtr-ar9
title: Extract --summary flag to local bool before the loop (inconsistent with other flags)
status: closed
type: task
priority: 3
tags:
- code-quality
- clap
- idiomatic
created: 2026-06-08
updated: 2026-06-09
closed_reason: Fixed
phase: ''
---

# Extract --summary flag to local bool before the loop (inconsistent with other flags)

main.rs extracts move_files, stop_on_error, show_detail_info, and dry_run
into local booleans before the loop, but evaluates the summary flag inline
after the loop via cli_args.value_source("summary") == Some(CommandLine).

Fix: add `let print_summary = cli_args.get_flag("summary");` alongside
the other flag extractions at the top, and replace the post-loop
value_source check with `if print_summary { ... }`.

This is part of the broader gtr-llt work (replace all value_source
checks with get_flag).
