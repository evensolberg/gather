---
id: gtr-h5k
title: Detect same-basename collisions at target
status: in_progress
type: bug
priority: 3
tags:
- gather
- correctness
created: 2026-07-04
updated: 2026-07-04
phase: ''
---

# Detect same-basename collisions at target

When two source files share the same basename (e.g. /a/report.pdf and /b/report.pdf), the second silently overwrites the first at the target with no warning. Pre-existing in serial mode; a race in parallel mode. Add collision detection before writing (or at minimum a warning).

[start] 2026-07-04 09:33:28
