---
id: gtr-3aa
title: Extract per-file processing logic into a dedicated function
status: blocked
type: task
priority: 2
tags:
- refactoring
- readability
- testability
created: 2026-06-08
updated: 2026-07-02
blocked_by:
- gtr-3dm
phase: ''
---

# Extract per-file processing logic into a dedicated function

The main loop in run() contains a deeply nested if/else chain
(dry_run / move_files / copy). Extract into a function:
  fn process_file(source: &str, target: &Path, opts: &ProcessOptions) -> Result<bool>
where ProcessOptions holds dry_run, move_files, stop_on_error, show_detail_info.
Benefits: shorter run(), independently testable, clearer control flow.
