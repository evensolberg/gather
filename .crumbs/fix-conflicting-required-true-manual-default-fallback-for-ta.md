---
id: gtr-9cf
title: Fix conflicting required(true) + manual default fallback for target arg
status: closed
type: bug
priority: 1
tags:
- bug
- clap
- cli
created: 2026-06-08
updated: 2026-06-08
closed_reason: 'Implemented: clap default_value on target arg; dead fallback removed from main.rs; two unit tests added. PR on fix/gtr-9cf-target-default-value.'
phase: ''
---

# Fix conflicting required(true) + manual default fallback for target arg

In cli.rs, replace `.required(true)` with `.required(false).default_value(".")` on the target arg:

  Arg::new("target")
      .value_name("TARGET")
      .help("The target directory into which files are to be gathered. Defaults to the current directory.")
      .num_args(1)
      .required(false)
      .default_value(".")
      .last(true)
      .action(ArgAction::Set)

Then in main.rs, delete the three lines of custom fallback code entirely:

  // DELETE these three lines:
  const DEFAULT_TARGET_DIR: &str = ".";
  let binding = DEFAULT_TARGET_DIR.to_string();
  let target_dir = cli_args.get_one::<String>("target").unwrap_or(&binding);

  // REPLACE with:
  let target_dir = cli_args.get_one::<String>("target").expect("clap default_value ensures this is always set");

Clap owns the default; no custom string or borrow-checker workaround needed. The expect() will never fire — it just makes the contract explicit.
