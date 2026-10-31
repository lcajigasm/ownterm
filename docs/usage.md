# Using OwnTerm

OwnTerm is a local terminal. The shell works without an account. Agent requests go only to the OpenAI-compatible endpoint you configure. Nothing in this guide signs you into Warp or sends a usage report.

## Install or update

From a checkout of this repository:

```bash
bash script/install-ownterm.sh
```

That downloads the archive for this machine from GitHub Releases, checks it against `SHA256SUMS`, and copies `ownterm` and `ownterm-tui` to `~/.local/bin`. Run the same command later to replace those binaries with the latest release. Pin a version with `--tag v0.2.0` (use the tag you want).

Add `~/.local/bin` to `PATH` if it is not already there, then run `ownterm` or `ownterm-tui`.

The archives are not code-signed. macOS may ask you to allow the app in Privacy & Security the first time you open it. Windows SmartScreen may show a warning. Check `SHA256SUMS` on the release before you bypass either prompt.

To verify by hand:

```bash
curl -fsSL -o SHA256SUMS https://github.com/lcajigasm/ownterm/releases/latest/download/SHA256SUMS
curl -fsSL -o ownterm-linux-x86_64.tar.gz https://github.com/lcajigasm/ownterm/releases/latest/download/ownterm-linux-x86_64.tar.gz
sha256sum -c SHA256SUMS --ignore-missing
```

Use the asset name for your machine: `ownterm-macos-arm64`, `ownterm-macos-x86_64`, `ownterm-linux-x86_64`, or `ownterm-windows-x86_64`.

## First launch

Start `ownterm`. There is no signup step. With no model configured, the prompt shows **No AI provider**. Settings opens on the agent page and leaves out billing, teams, and Warp Drive. The shell still runs.

You can leave the model unset and use OwnTerm as a terminal. Set a provider when you want the agent.

## Provider

`OWNTERM_MODEL` is required for agent requests. Environment variables override `~/.ownterm/provider.env`. The file accepts `provider`, `model`, and `base_url`. It is rejected if it contains `api_key`, `token`, or `secret`. Put a cloud key in `OWNTERM_API_KEY` or type it into Settings so it stays in the OS keychain.

The installer can write the non-secret fields:

```bash
bash script/install-ownterm.sh --provider ollama --model llama3.3
```

Or edit the file yourself:

```
provider=ollama
model=llama3.3
```

`ownterm-tui configure-provider` writes that file. It does not store an API key.

```bash
ownterm-tui configure-provider --provider ollama --model llama3.3
```

`ownterm-tui models --provider ollama` prints suggested model ids and the catalog URL for that endpoint (`/models`). It does not download the catalog. Pass `--base-url` when the preset needs one, or to point at your own server.

| `provider` | Default base URL | Key |
| :--- | :--- | :--- |
| `ollama` | `http://127.0.0.1:11434/v1` | no |
| `lmstudio` | `http://127.0.0.1:1234/v1` | no |
| `vllm` | `http://127.0.0.1:8000/v1` | no |
| `localai` | `http://127.0.0.1:8080/v1` | no |
| `openai` | `https://api.openai.com/v1` | `OWNTERM_API_KEY` |
| `xai` | `https://api.x.ai/v1` | `OWNTERM_API_KEY` |
| `openrouter` | `https://openrouter.ai/api/v1` | `OWNTERM_API_KEY` |
| `groq` | `https://api.groq.com/openai/v1` | `OWNTERM_API_KEY` |
| `compatible` | set `base_url` | optional |

HTTP is accepted only for localhost and loopback. Other endpoints must be HTTPS.

## Local workflows

Workflows in `~/.ownterm/workflows` show up in command search and in the `/` menu. They are files on this machine. Warp Drive is not involved.

`~/.ownterm/workflows/git-status.yaml`:

```yaml
---
name: "Git status"
command: "git status --short"
description: "Show the working tree status"
arguments: []
---
```

Several workflows may share one YAML file, separated by `---`. Restart is not required after you save the file; the directory is watched.

## Headless TUI

`ownterm-tui` is the same terminal without a GPU window. Use it over SSH. It reads the same provider file and the same `~/.ownterm/workflows` directory.

## What stays on this machine

- The installer talks only to GitHub Releases and checks `SHA256SUMS` before it writes the binaries.
- The shipped app does not enable the built-in updater, so it does not download builds on its own.
- Analytics events are not constructed or uploaded.
- Agent traffic goes to the endpoint in your provider file or environment, and nowhere else for inference.
