---
id: gtr-ar3
title: Add unit tests for check_directory and log_build
status: open
type: task
priority: 1
tags:
- testing
- quality
created: 2026-06-08
updated: 2026-06-08
phase: ''
---

# Add unit tests for check_directory and log_build

There are zero tests in the project; `cargo test` passes trivially.
Minimum coverage:
- check_directory: test with a real temp dir (pass), a file path (fail),
  and a nonexistent path (fail).
- log_build: test that quiet produces LevelFilter::Off, debug=0 -> Info,
  debug=1 -> Debug, debug=2+ -> Trace.
Use tempfile crate or std::env::temp_dir() for filesystem tests.
