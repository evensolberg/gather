---
id: gtr-6ug
title: Modernize fn main() to return Result<()> and fix String::as_str path
status: closed
type: task
priority: 2
tags:
- idiomatic
- main
- error-handling
created: 2026-06-08
updated: 2026-07-02
closed_reason: 'Done in PR #81 (refactor/modernise-main). main() returns ExitCode::FAILURE/SUCCESS; String::as_str path simplified.'
blocks:
- gtr-4h2
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

[start] 2026-07-02 21:15:30

[stop]  2026-07-02 21:42:17  26m 47s  Implemented: modernised main() to return ExitCode, removed dead replace() call, fixed String::as_str qualification. PR #81.
