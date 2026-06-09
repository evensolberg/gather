---
id: gtr-0du
title: Change dry-run short flag from -r to -n (conventional)
status: open
type: task
priority: 3
tags:
- ux
- convention
- cli
created: 2026-06-08
updated: 2026-06-08
phase: ''
---

# Change dry-run short flag from -r to -n (conventional)

cli.rs uses -r as the short flag for --dry-run. The universal Unix convention is
-n (rsync -n, make -n, etc.). -r is widely associated with --recursive, which can
confuse users. Change `.short('r')` to `.short('n')`.
Note: this is a semver-breaking change for scripts using -r, so bump to 0.3.0.
