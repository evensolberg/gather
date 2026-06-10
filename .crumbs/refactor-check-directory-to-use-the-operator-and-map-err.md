---
id: gtr-4sh
title: Refactor check_directory to use the ? operator and map_err
status: closed
type: task
priority: 2
tags:
- idiomatic
- error-handling
- refactoring
- utils
created: 2026-06-08
updated: 2026-06-09
closed_reason: Implemented
phase: ''
---

# Refactor check_directory to use the ? operator and map_err

check_directory in utils.rs uses a manual match + early return pattern that predates
the ? operator. The idiomatic rewrite is:

  let metadata = std::fs::metadata(target)
      .map_err(|e| format!("Target: {e}"))?;
  if !metadata.is_dir() {
      return Err("Specified target is not a directory. Unable to proceed.".into());
  }
  log::debug!("Specified target is a directory. Proceeding.");
  Ok(())

This reduces ~14 lines to ~6 with identical semantics. The map_err transforms the
IO error into the same formatted string the current code produces.
