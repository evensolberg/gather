---
id: gtr-6ug
title: Modernize fn main() to return Result<()> and fix String::as_str path
status: open
type: task
priority: 2
tags:
- idiomatic
- main
- error-handling
created: 2026-06-08
updated: 2026-06-08
phase: ''
---

# Modernize fn main() to return Result<()> and fix String::as_str path

main() currently uses the old std::process::exit(match run() {...}) pattern.
Modern idiomatic Rust CLIs declare `fn main() -> anyhow::Result<()>` (or
`Result<(), Box<dyn Error>>`) and let the runtime handle the non-zero exit:

  fn main() -> anyhow::Result<()> {
      run()
  }

This eliminates the manual exit-code mapping and the log::error! call in main
(run() already returns a descriptive Err).

Also in the same pass: the sources iterator uses `std::string::String::as_str`
(fully qualified path, unnecessary). Replace with just `String::as_str`:
  .map(std::string::String::as_str)  // current
  .map(String::as_str)               // idiomatic
