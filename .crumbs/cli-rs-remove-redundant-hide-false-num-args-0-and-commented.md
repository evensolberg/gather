---
id: gtr-voo
title: 'cli.rs: remove redundant .hide(false), .num_args(0), and commented-out .author()'
status: open
type: task
priority: 2
tags:
- idiomatic
- clap
- cleanup
- cli
created: 2026-06-08
updated: 2026-06-08
phase: ''
---

# cli.rs: remove redundant .hide(false), .num_args(0), and commented-out .author()

Every arg in cli.rs carries .hide(false) and .num_args(0) even though both are
the default values for SetTrue/Count actions — they add visual noise without meaning.
Also remove the commented-out `.author(clap::crate_authors!("\n"))` line; dead code
belongs in version history, not source files.
Concretely, remove from every Arg:
  .hide(false)
  .num_args(0)
And delete the commented author line entirely.
