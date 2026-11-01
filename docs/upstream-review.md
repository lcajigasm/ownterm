# Upstream review (warpdotdev/warp)

OwnTerm forked the Warp client at `1e6c3ae465d6a925a9b97ae03877aaadd159b8f1` (2026-10-22, “Project ACP MCP tool calls with their tool name, server identity, and late input”, warpdotdev/warp#16615). This note lists commits on the upstream default branch after that point, through 2026-10-30, and whether OwnTerm should take them.

Telemetry, accounts, hosted keys, and paid or cloud-only behavior stay out. Two small fixes are in this tree. The rest are not.

## Brought

| Change | Why |
| :--- | :--- |
| Fix Tab key behavior always detected as user defined (#16529) | Settings compared `to_uppercase()` of the binding with `"Tab"`, so a real Tab binding never matched and the control always showed a custom binding. Two comparisons, no new behavior. |
| Fix premature MCP protocol-version header during initialize (#16698) | Sending `mcp-protocol-version` on `initialize` stops a local MCP server from offering another protocol version. The header stays on the discover request only. |

## Not brought

| Change | Why |
| :--- | :--- |
| Reset reconnects with no progress if there are no new events to send (#16696) | Cloud session reconnect. OwnTerm does not sync a Warp session. |
| Preview URL idle retention (#16649) | Cloud preview URLs. |
| Support managed LLM provider API keys in the harness driver (#16470) | Hosted account keys. OwnTerm reads a local environment variable and does not store the key. |
| Prefer `WARP_WORKSPACE_ROOT` in the factory-deferred-repositories skill (#16693) | Cloud factory skill, not the local terminal. |
| Fix environment checkout hanging during shell bootstrap (#16661) | Cloud environment checkout. |
| Release workflow: federate to GCP and Octo STS (#16674) | Warp release infrastructure. |
| Align agent dev image Go linter with server CI (#16672) | Warp server CI. |
| Bump `warpdotdev/oz-agent-action` (#16518) | Warp's agent GitHub Action. |
| Bump `taiki-e/install-action` (#16575) | Upstream CI tooling, not product behavior. |
| Bump `trunk-io/analytics-uploader` (#16562) | Analytics upload. OwnTerm does not send telemetry. |
| Exclude warp acp-bridge from TUI detection (#16675) | Special-cases Warp's own agent bridge binary. |
| CI: mint GitHub App tokens instead of PATs (#16655) | Warp CI credentials. |
| Update factory authoring skill for automation scope filters (#16670) | Cloud automation authoring. |
| Preserve Linux runtime commands after shell profiles reset PATH (#16650) | Restores `WARP_CLOUD_AGENT_FALLBACK_BIN` for cloud containers. |
| Fix status-bar history subscription accumulation (#16658) | Touches conversation usage updates in the agent status bar. Larger than the two fixes above, and next to usage accounting. |
| Declare `GITHUB_TOKEN` permissions for workflows that write to the repo (#16652) | Warp workflow permissions. |
| Add Octo STS trust policy `channel-versions-read` (#16656) | Warp infrastructure. |
