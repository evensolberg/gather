---
id: gtr-3dm
title: log_build() should return () — Builder is consumed internally by .init()
status: closed
type: task
priority: 2
tags:
- code-quality
- api-design
created: 2026-06-08
updated: 2026-07-02
blocks:
- gtr-3aa
phase: ''
---

# log_build() should return () — Builder is consumed internally by .init()

utils::log_build() returns a Builder assigned to _logbuilder (underscore = intentional
discard). The Builder is already consumed via .init() inside the function, so returning
it is misleading. Callers cannot meaningfully use the returned value.
Fix: change return type to () and remove the return statement; update the call site
in main.rs to discard the assignment entirely.

[2026-06-08] Also fix the builder chain pattern inside log_build: the current match arms call logbuilder.filter_level(...) and discard the returned &mut Builder via semicolons, which reads as though the match produces a value when it does not. Extract the level first, then chain fluently:

  let level = if cli_args.get_flag("quiet") { LevelFilter::Off } else {
      match cli_args.get_count("debug") {
          0 => LevelFilter::Info, 1 => LevelFilter::Debug, _ => LevelFilter::Trace,
      }
  };
  Builder::new().filter_level(level).target(Target::Stdout).init();

This also folds the quiet check into the same expression rather than a separate if-block.

[start] 2026-07-02 22:04:04

[stop]  2026-07-02 22:13:25  9m 21s
