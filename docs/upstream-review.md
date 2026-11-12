# Upstream review (warpdotdev/warp)

OwnTerm forked the Warp client at `1e6c3ae465d6a925a9b97ae03877aaadd159b8f1` (2026-10-22, “Project ACP MCP tool calls with their tool name, server identity, and late input”, warpdotdev/warp#16615). This note lists commits on the upstream default branch after that point, through 2026-10-30, and whether OwnTerm should take them.

## Automatic sync

`.github/workflows/upstream-sync.yml` runs every Monday and on demand. It runs `script/sync-upstream.sh`, which merges the upstream default branch into a branch named `upstream-sync` and opens a pull request. Paths in `.github/upstream-sync-keep-ours` stay as OwnTerm has them, and a file OwnTerm deleted stays deleted. The privacy audit result is in the pull request, and OwnTerm CI runs on the branch. When a file conflicts, the run fails and its summary lists the files and the commands to finish the merge from a checkout.

Merge that pull request with a merge commit. A squash drops the link to upstream, and the next run would bring the same commits again.

The tables below are the manual passes made before this workflow existed.

Telemetry, accounts, hosted keys, and paid or cloud-only behavior stay out. Two small fixes are in this tree. The rest are not.

Checked again on 2026-11-01. The upstream default branch still ends at `d57d9760dba0590416eca4c94379ce7653f347b5` (#16696, 2026-10-30). No newer commit, so this pass brings nothing further.

Checked again on 2026-11-02. The same commit is still the tip. This pass brings nothing from upstream. The agent-tool work in this tree is local.

Checked again later on 2026-11-02. The upstream default branch still ends at `d57d9760dba0590416eca4c94379ce7653f347b5` (#16696, 2026-10-30). No newer commit, so this pass brings nothing further. Saving a local workflow is OwnTerm code: it writes YAML the existing command search already loads, and it does not take Warp Drive.

Checked again on 2026-11-04. The upstream default branch now ends at `2d5398a4f10aac2efb914b71104e518e83594d16` (#16727). Seven commits landed after `d57d976`. None of them belong in this tree. The Auto rules in this pass are local.

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
| Let a configured custom inference endpoint pass the out-of-credits gate (#16493, `ffc5dc884c32`) | Warp still stops a prompt when credits are exhausted unless a legacy API-key list is filled. OwnTerm's OSS channel already allows the request when a local provider or a custom endpoint URL is set, before that credit check. |
| Send the warp-channel-config token as a URL-scoped git header (#16701, `0eca80f4c125`) | Fetches Warp's private channel config. OwnTerm does not clone that repository. |
| Read the Azure signing tenant and client IDs from repository variables (#16712, `77a4fc2223a3`) | Warp release signing. |
| Fix Linux signing passphrase handling (#16713, `3dca8f995cc1`) | Warp release signing. |
| ACP: resume the adapter session on new sandboxes, and steer mid-turn follow-ups (#16720, `1eca3e4eebc6`) | Cloud sandbox memory and mid-turn steering for the Claude Code and Codex harnesses. OwnTerm's local agent talks to an OpenAI-compatible endpoint and does not resume a Warp sandbox. |
| Authenticate changelog metadata across repositories (#16721, `eba011405547`) | Warp changelog automation. |
| Update auth configuration for the changelog generation agent action (#16727, `2d5398a4f10a`) | Warp changelog automation. The commit is the upstream tip on 2026-11-04. |
