---
id: gtr-lrc
title: Set wildcards = warn in deny.toml (currently allow)
status: closed
type: task
priority: 2
tags:
- security
- cargo-deny
- dependencies
created: 2026-06-08
updated: 2026-06-09
closed_reason: Set to warn for now
phase: ''
---

# Set wildcards = warn in deny.toml (currently allow)

deny.toml has `wildcards = "allow"` in the [bans] section. Wildcard version
requirements (version = "*") are risky because they allow any version including
future breaking or vulnerable releases. Change to `wildcards = "warn"` or `"deny"`
to be alerted when a dependency uses unconstrained version specs.
