---
id: gtr-zjr
title: Add repository and keywords to Cargo.toml metadata
status: closed
type: task
priority: 3
tags:
- metadata
- crates-io
created: 2026-06-08
updated: 2026-06-09
closed_reason: Included
phase: ''
---

# Add repository and keywords to Cargo.toml metadata

Cargo.toml is missing repository, homepage, and keywords fields. These improve
discoverability on crates.io. Add:
  repository = "https://github.com/evensolberg/gather"
  keywords = ["gather", "files", "copy", "move", "cli"]
  categories = ["command-line-utilities", "filesystem"]
