---
id: gtr-du6
title: Remove empty [features] section from Cargo.toml
status: closed
type: task
priority: 4
tags:
- cleanup
- cargo
created: 2026-06-08
updated: 2026-06-09
closed_reason: Fixed
phase: ''
---

# Remove empty [features] section from Cargo.toml

Cargo.toml contains an empty [features] section which adds noise without value.
Remove it unless features are actually planned in the near future.
