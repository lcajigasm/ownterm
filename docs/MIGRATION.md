# Migration plan

This is a staged removal plan, not a blind text replacement. Deleting account
types before their dependencies are understood is likely to break terminal
startup and preferences.

## Phase 0 — reproducible baseline

- Check out the public fork into `upstream/`.
- Read the repository `AGENTS.md` before editing.
- Build the OSS binary unchanged with the documented scripts.
- Record platform, toolchain, commit, build command, and startup behavior.
- Add a CI job that builds the unmodified baseline.

Exit criterion: a clean build and a smoke-tested shell session.

## Phase 1 — inventory and denylist

Run `scripts/audit-warp-services.sh`. Classify each result as terminal core,
optional local feature, commercial UI, or remote dependency. Create tests that
fail when new Warp endpoints or product modules enter OwnTerm.

Initial areas to inspect include:

- authentication, account, team, workspace, entitlements, billing, credits;
- Warp Drive and cloud object sync;
- Oz/cloud-agent orchestration and hosted inference;
- analytics, experiments, remote feature flags, Sentry/crash upload;
- update/release services, referral, marketing, admin, and enterprise policy;
- settings views for plans, usage, organizations, and hosted API keys.

Exit criterion: every outbound endpoint and business-facing module has an owner
and disposition.

## Phase 2 — offline terminal shell

- Make authentication optional, then remove it from the startup state machine.
- Replace remote feature gates with compile-time/local defaults.
- Disable cloud initialization and network retries.
- Keep terminal, PTY, renderer, input, tabs/panes, blocks, themes, search,
  completions, keybindings, command palette, and local settings.
- Remove business navigation and unreachable assets.

Exit criterion: a new profile can launch and use a shell with networking denied.

## Phase 3 — direct BYOK inference

- Extract the provider-neutral request and event types.
- Implement an OpenAI-compatible adapter, including SSE streaming and tools.
- Add native Anthropic and Google adapters only where their APIs cannot be
  represented cleanly through the compatibility layer.
- Add secure credential storage and a provider settings screen.
- Ensure custom base URLs and model selections persist locally.
- Remove the client-to-Warp GraphQL inference path.

Exit criterion: the agent works against a selected provider while Warp domains
are blocked, and logs contain no credentials.

## Phase 4 — debranding and packaging

- Rename bundle/application IDs, binaries, channels, config paths, protocol
  handlers, desktop files, icons, and user-facing copy.
- Replace product links and support contacts.
- Keep copyright, AGPL, MIT, third-party notices, and fork attribution.
- Add a migration note for users with existing Warp configuration; never read
  or upload it silently.

Exit criterion: no product identity suggests endorsement by Warp.

## Phase 5 — hardening and release

- Add provider contract tests with recorded, secret-free fixtures.
- Test cancellation, retry boundaries, proxy settings, invalid certificates,
  oversized context, malformed SSE, and tool-call validation.
- Run secret scanning and dependency/license checks.
- Publish source and reproducible build instructions with every binary release.

Exit criterion: signed artifacts correspond to a public tagged source commit.
