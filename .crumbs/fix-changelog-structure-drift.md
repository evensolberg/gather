---
id: gtr-886
title: Fix CHANGELOG structure drift
status: closed
type: task
priority: 3
tags:
- cli
- docs
- chore
created: 2026-06-13
updated: 2026-07-03
phase: ''
---

# Fix CHANGELOG structure drift

The [unreleased] section has wrong group headings ("Chore"/"Fix" instead of "Miscellaneous Tasks") and contains already-released items. Regenerate with git-cliff or manually align to cliff.toml groups. Noted during PR #77 Copilot review.

[start] 2026-07-03 07:56:52

[stop]  2026-07-03 07:57:01  9s
