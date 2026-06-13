---
id: gtr-bdh
title: dry-run output silenced by -q (same root cause as gtr-b6p)
status: open
type: bug
priority: 2
tags:
- ux
- cli
- logging
- dry-run
created: 2026-06-12
updated: 2026-06-12
phase: ''
---

# dry-run output silenced by -q (same root cause as gtr-b6p)

gather -n -q produces zero output — the dry-run start banner (main.rs:40) and per-file preview lines (main.rs:66-69) use log::info! which LevelFilter::Error silences. The same fix applied to the summary block (println! instead of log::info!) should be applied here. Discovered during code review of the gtr-b6p fix (PR #75).
