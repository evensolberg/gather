---
id: gtr-p5l
title: Document or fix .last(true) + default_value('.') silent wrong-directory footgun
status: in_progress
type: bug
priority: 2
tags:
- ux
- cli
- clap
created: 2026-06-08
updated: 2026-06-12
phase: ''
---

# Document or fix .last(true) + default_value('.') silent wrong-directory footgun

With .last(true) on the target arg and default_value("."), a user who
runs `gather *.pdf /dest` (without --) has /dest consumed by the greedy
`read` arg (num_args 1..) and silently gathers into the CWD with no
warning. check_directory validates "." without complaint.

Options:
A) Document clearly in help text: "TARGET must appear after --,
   e.g.: gather *.pdf -- /dest"
B) Add a test asserting this behaviour so any accidental fix is noticed
C) Reconsider .last(true) — switch target to a named option
   (e.g. --target / -t) to avoid the positional ambiguity entirely

The PR that introduced default_value (gtr-9cf) made this more prominent:
before, omitting the target produced a clap required-arg error; now it
silently defaults to '.'.

[start] 2026-06-12 21:00:45
