---
id: gtr-1a5
title: Add integration test for --serial flag end-to-end
status: open
type: feature
priority: 3
tags:
- gather
- testing
created: 2026-07-04
updated: 2026-07-04
phase: ''
---

# Add integration test for --serial flag end-to-end

The --serial / -1 flag is unit-tested for CLI parsing but has no integration test that runs the binary and confirms files are actually copied in serial mode. Add a tests/cli.rs test using assert_cmd or similar.
