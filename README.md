# OwnTerm

<p align="center">
  <strong>The Open-Source, Local-First, Privacy-Focused AI Terminal.</strong><br>
  <em>GPU-accelerated, block-based terminal emulator with native agent tool-calling — zero logins, zero telemetry.</em>
</p>

<p align="center">
  <a href="https://github.com/lcajigasm/ownterm/releases/latest"><img src="https://img.shields.io/github/v/release/lcajigasm/ownterm?color=00ff9d&label=Release" alt="Release"></a>
  <a href="https://lcajigasm.github.io/ownterm/"><img src="https://img.shields.io/badge/Website-lcajigasm.github.io%2Fownterm-00d2ff" alt="Website"></a>
  <a href="https://github.com/lcajigasm/ownterm/actions/workflows/ci.yml"><img src="https://github.com/lcajigasm/ownterm/actions/workflows/ci.yml/badge.svg" alt="CI &amp; Release"></a>
  <a href="https://github.com/lcajigasm/ownterm/actions/workflows/pages.yml"><img src="https://github.com/lcajigasm/ownterm/actions/workflows/pages.yml/badge.svg" alt="GitHub Pages"></a>
  <a href="LICENSE-AGPL"><img src="https://img.shields.io/badge/License-AGPL%203.0-blue.svg" alt="License: AGPL-3.0"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/UI%20Core-MIT-green.svg" alt="License: MIT"></a>
  <a href="https://github.com/lcajigasm/ownterm/actions/workflows/audit-services.yml"><img src="https://github.com/lcajigasm/ownterm/actions/workflows/audit-services.yml/badge.svg" alt="Zero-Telemetry Audit"></a>
</p>

---

## 🌐 Official Website & Documentation

Visit the official website for interactive demos, feature overviews, and release assets:
👉 **[https://lcajigasm.github.io/ownterm/](https://lcajigasm.github.io/ownterm/)**

---

## Table of Contents

- [Why OwnTerm?](#why-ownterm)
- [Key Features](#key-features)
- [Downloads & Installation](#downloads--installation)
- [Bring Your Own Model (BYOM)](#bring-your-own-model-byom)
  - [1. Local Models with Ollama](#1-local-models-with-ollama)
  - [2. Local Models with vLLM / LM Studio / LocalAI](#2-local-models-with-vllm--lm-studio--localai)
  - [3. Cloud Providers (OpenAI, OpenRouter, Groq)](#3-cloud-providers-openai-openrouter-groq)
- [Agent Tool Calling & Rules Configuration](#agent-tool-calling--rules-configuration)
- [Headless TUI (ownterm-tui)](#headless-tui-ownterm-tui)
- [Comparison Matrix](#comparison-matrix)
- [Security & Zero-Telemetry Guarantee](#security--zero-telemetry-guarantee)
- [Building from Source](#building-from-source)
- [Releasing New Versions](#releasing-new-versions)
- [Continuous Integration & GitHub Pages](#continuous-integration--github-pages)
- [Licensing and Trademarks](#licensing-and-trademarks)

---

## Why OwnTerm?

Modern terminals need more than classic 80x24 character grids — they need block-based navigation, searchable history, rich completions, and contextual AI assistance. However, proprietary alternatives lock these features behind **mandatory accounts, telemetry tracking, cloud drives, and paid subscriptions**.

**OwnTerm** is an open-source, local-first fork of the Warp client published by Warp Services, Inc. The terminal core remains under the upstream AGPL-3.0 and MIT licenses; see [Licensing and Trademarks](#licensing-and-trademarks). OwnTerm is not affiliated with Warp. The `ownterm` binary does not require a Warp account, does not upload telemetry or crash reports, and sends agent requests only to the OpenAI-compatible endpoint you configure.

### Core Principles

- **Zero Sign-In**: Start the app and immediately use your shell. No signup, no login screens, no authentication gates.
- **Zero Telemetry**: Completely offline-capable. No analytics, tracking pixels, or background data collection.
- **Direct Model Connections**: Connect directly to your local Ollama / vLLM instance or any OpenAI-compatible provider.
- **Keychain Security**: API keys are saved exclusively in your operating system's native credential store (macOS Keychain, Linux Secret Service, Windows Credential Manager).
- **Tool Calling Freedom**: Inspect files, execute commands, and run tests locally with user permission.
- **Open Source**: The main codebase is licensed under **AGPL-3.0**, and the shared UI framework components are licensed under **MIT**.

---

## Key Features

- ⚡ **GPU-Accelerated Rendering**: Lightning-fast performance using native Metal on macOS and OpenGL/Vulkan on Linux and Windows.
- 🧱 **Block-Based Output**: Terminal commands and outputs are grouped into interactive, searchable, and copyable blocks.
- 🤖 **Native AI Agent Mode**: Prompt the agent to inspect build failures, search files, write scripts, or execute shell commands right from the terminal prompt (`Ctrl+Enter` / `Cmd+Enter`).
- 🛠️ **Autonomous Tool Execution**: The agent suggests and executes tools like `run_shell_command` and `read_files`, requiring explicit user confirmation or policy allowance.
- 🎛️ **Project & Global Rules**: Customize model behavior and system instructions using local `.ownterm/rules.md` or global `~/.ownterm/rules.md`.
- 🖥️ **Dual Interfaces**: Choose between the visual desktop GUI (`ownterm`) or the lightweight headless console interface (`ownterm-tui`) for SSH sessions and remote servers.

---

## Downloads & Installation

Precompiled archives are attached to [GitHub Releases](https://github.com/lcajigasm/ownterm/releases/latest). Each archive contains `ownterm` and `ownterm-tui`.

From a checkout, install or update the binaries for this machine into `~/.local/bin`. The script checks `SHA256SUMS` before it writes them. The app does not download updates by itself.

```bash
bash script/install-ownterm.sh
bash script/install-ownterm.sh --provider ollama --model llama3.3
```

The day-to-day guide (first launch, `provider.env`, and local workflows) is [docs/usage.md](docs/usage.md). Archives are not code-signed.

| Platform | Architecture | Package |
| :--- | :--- | :--- |
| **macOS** | Apple Silicon (M1/M2/M3/M4) | [ownterm-macos-arm64.tar.gz](https://github.com/lcajigasm/ownterm/releases/latest/download/ownterm-macos-arm64.tar.gz) |
| **macOS** | Intel (x86_64) | [ownterm-macos-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/latest/download/ownterm-macos-x86_64.tar.gz) |
| **Linux** | x86_64 (glibc 2.31+) | [ownterm-linux-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/latest/download/ownterm-linux-x86_64.tar.gz) |
| **Windows**| x86_64 (Win 10/11) | [ownterm-windows-x86_64.tar.gz](https://github.com/lcajigasm/ownterm/releases/latest/download/ownterm-windows-x86_64.tar.gz) |

Verify a download with the `SHA256SUMS` file on that release. A tag `vX.Y.Z` builds these archives; pushing to `master` does not.

---

## Bring Your Own Model (BYOM)

OwnTerm connects to any inference engine that exposes an OpenAI-compatible `/chat/completions` endpoint. Named presets live in `crates/ownterm_provider`.

| `OWNTERM_PROVIDER` | Default base URL | API key |
| :--- | :--- | :--- |
| `ollama` | `http://127.0.0.1:11434/v1` | not required |
| `lmstudio` | `http://127.0.0.1:1234/v1` | not required |
| `vllm` | `http://127.0.0.1:8000/v1` | not required |
| `localai` | `http://127.0.0.1:8080/v1` | not required |
| `openai` | `https://api.openai.com/v1` | `OWNTERM_API_KEY` |
| `xai` | `https://api.x.ai/v1` | `OWNTERM_API_KEY` |
| `openrouter` | `https://openrouter.ai/api/v1` | `OWNTERM_API_KEY` |
| `groq` | `https://api.groq.com/openai/v1` | `OWNTERM_API_KEY` |
| `compatible` | set `OWNTERM_BASE_URL` | optional |

`OWNTERM_MODEL` is always required. `OWNTERM_BASE_URL` overrides the preset. HTTP is accepted only for `localhost` and loopback addresses.

You can also put non-secret fields in `~/.ownterm/provider.env` (or `OWNTERM_PROVIDER_FILE`). Environment variables win. The file is rejected if it contains `api_key`, `token`, or `secret`. `ownterm-tui configure-provider --provider ollama --model llama3.3` writes the file. `ownterm-tui models --provider ollama` prints suggested ids and the `/models` URL without downloading it.

```
provider=xai
model=grok-3
```

When a provider is configured this way, OwnTerm does not apply Warp credit or signup gates to the agent request. Keys typed into Settings still go to the OS keychain. With no provider, the prompt shows "No AI provider" and opens AI settings instead of a signup or credit warning.

Workflows in `~/.ownterm/workflows` appear in command search and the `/` menu without Warp Drive. Shell completions and command-palette file search stay on this machine.

### 1. Local Models with Ollama

Run completely private, local models with zero internet access:

```bash
# 1. Pull and run your preferred model
ollama run llama3.3

# 2. Start OwnTerm pointing to Ollama
export OWNTERM_PROVIDER="ollama"
export OWNTERM_MODEL="llama3.3"
./script/run
```

*(No API key is needed for loopback endpoints.)*

### 2. Local Models with vLLM / LM Studio / LocalAI

- **vLLM**:
  ```bash
  vllm serve Qwen/Qwen2.5-Coder-7B-Instruct --port 8000
  export OWNTERM_BASE_URL="http://localhost:8000/v1"
  export OWNTERM_MODEL="Qwen/Qwen2.5-Coder-7B-Instruct"
  ./script/run
  ```
- **LM Studio**:
  Start the Local Server in LM Studio (default: `http://localhost:1234/v1`) and set:
  ```bash
  export OWNTERM_BASE_URL="http://localhost:1234/v1"
  export OWNTERM_MODEL="qwen2.5-coder-7b-instruct"
  ./script/run
  ```

### 3. Cloud Providers (OpenAI, OpenRouter, Groq)

For remote cloud providers, HTTPS is strictly enforced and API keys are automatically redacted from error traces:

```bash
# OpenRouter example
export OWNTERM_BASE_URL="https://openrouter.ai/api/v1"
export OWNTERM_MODEL="anthropic/claude-3.5-sonnet"
export OWNTERM_API_KEY="sk-or-v1-..."
./script/run

# OpenAI example
export OWNTERM_BASE_URL="https://api.openai.com/v1"
export OWNTERM_MODEL="gpt-4o"
export OWNTERM_API_KEY="sk-proj-..."
./script/run
```

---

## Agent Tool Calling & Rules Configuration

OwnTerm normalizes function calling so any compatible model can interact directly with your terminal and workspace.

### Available Agent Tools

- `run_shell_command`: Executes terminal commands (e.g. running builds, test suites, git commands, and CLI tools). Users can inspect the command, click **Run**, or choose **Always Allow**.
- `read_files`: Inspects file contents in the workspace to analyze errors or provide code fixes.

### Custom Rules and System Prompts

You can steer the agent's behavior and personality using rules files. OwnTerm resolves system instructions with the following order of precedence:

1. `OWNTERM_SYSTEM_PROMPT` environment variable.
2. Local project rules: `.ownterm/rules.md` (in current working directory).
3. Global user rules: `~/.ownterm/rules.md` (in your home directory).
4. Default OwnTerm terminal assistant prompt.

---

## Headless TUI (`ownterm-tui`)

OwnTerm includes a headless terminal client (`crates/warp_tui`) designed for remote servers, SSH sessions, containers, and minimal desktop environments without a window manager:

```bash
# Run headless console client
./script/run-tui
```

`ownterm-tui` uses the cell-grid `TuiElement` rendering system, requiring no GPU or graphical display server.

---

## Comparison Matrix

| Feature | OwnTerm | Upstream Warp | Alacritty | Ghostty | WezTerm | Kitty |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Account / Login Required** | **Never** ✕ | Mandatory ✓ | **Never** ✕ | **Never** ✕ | **Never** ✕ | **Never** ✕ |
| **Telemetry & Metrics** | **Zero** ✕ | Always on ✓ | **Zero** ✕ | **Zero** ✕ | **Zero** ✕ | **Zero** ✕ |
| **Local LLMs (Ollama / vLLM)** | **Native 1st-Class** ✓ | Cloud only ✕ | None ✕ | None ✕ | None ✕ | None ✕ |
| **Agent Tool Calling** | **Native** ✓ | Cloud subscription ✓ | None ✕ | None ✕ | None ✕ | None ✕ |
| **Block-Based Terminal** | **Yes** ✓ | Yes ✓ | Classic grid ✕ | Classic grid ✕ | Classic grid ✕ | Classic grid ✕ |
| **GPU Acceleration** | **Metal / OpenGL** ✓ | Metal / OpenGL ✓ | OpenGL ✓ | Metal / OpenGL ✓ | OpenGL / Vulkan ✓ | OpenGL ✓ |
| **Headless TUI Included** | **Yes (`ownterm-tui`)** ✓ | No ✕ | No ✕ | No ✕ | No ✕ | No ✕ |
| **License** | **AGPL-3.0 / MIT** | Proprietary services | Apache 2.0 | MIT | MIT | GPLv3 |

---

## Security & Zero-Telemetry Guarantee

1. **Storage Isolation**: OwnTerm stores data strictly under `~/.ownterm` (GUI) and `~/.ownterm_cli` (TUI), fully isolated from any legacy configuration.
2. **Keyring Credentials**: API keys entered in Settings are written directly to OS-level secure storage, never plain text configuration files.
3. **Loopback-Only HTTP**: Non-HTTPS connections are strictly rejected unless targeting `localhost` or `127.0.0.1`.
4. **Automated Audit Workflow**: Every push and PR is scanned by [`.github/workflows/audit-services.yml`](.github/workflows/audit-services.yml) running [`scripts/audit-warp-services.sh`](scripts/audit-warp-services.sh) to ensure zero upstream cloud service coupling.

---

## Building from Source

### Prerequisites

- **Rust**: Latest stable toolchain (`rustup update stable`).
- **Protobuf Compiler (`protoc`)**:
  - macOS: `brew install protobuf`
  - Linux: `sudo apt-get install -y protobuf-compiler pkg-config libasound2-dev libfontconfig1-dev libfreetype6-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev`
  - Windows: `choco install protoc`

### Build & Run

```bash
# Clone the repository
git clone https://github.com/lcajigasm/ownterm.git
cd ownterm

# Run the GUI desktop app locally
./script/run

# Run the headless console TUI
./script/run-tui

# Build production release binary
cargo build --release -p warp --bin ownterm
```

*(Note for macOS: If you do not have Xcode Metal Developer Tools installed, prepend `WARP_LOCAL_SKIP_METAL=1 ./script/run`)*.

### Running Unit Tests

```bash
# Run core and channel tests
cargo test -p warp_core

# Run OwnTerm inference and agent tests
WARP_LOCAL_SKIP_METAL=1 cargo test -p warp --lib ownterm
```

---

## Releasing New Versions

OwnTerm includes full automated continuous delivery for versioning, building, and publishing multi-platform releases.

### 1. Pull request CI

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pull requests and on `master`. It audits the privacy gates, tests the release installer offline, formats and lints `ownterm_provider`, tests that crate plus the channel-config unit tests, and typechecks the `ownterm` and `ownterm-tui` binaries on Linux. It does not publish a release.

### 2. Tagged release binaries

Pushing a tag `vX.Y.Z` runs [`.github/workflows/release.yml`](.github/workflows/release.yml), which builds `ownterm` and `ownterm-tui` for:

- macOS Apple Silicon (`aarch64-apple-darwin` on `macos-15`)
- macOS Intel (`x86_64-apple-darwin` on `macos-15`)
- Linux (`x86_64-unknown-linux-gnu` on `ubuntu-22.04`)
- Windows (`x86_64-pc-windows-msvc` on `windows-latest`)

The workflow attaches the `.tar.gz` archives and `SHA256SUMS` to that tag. It does not create or force-push tags.

### 3. Manual version bump

`./script/bump-version.sh` updates manifests. Land that change with a squash-merged pull request, then push the tag yourself. Pushing `master` does not publish binaries.

```bash
./script/bump-version.sh 0.2.0
git tag v0.2.0
git push origin v0.2.0
```

---

## Continuous Integration & GitHub Pages

- **GitHub Pages Landing Page**: The official landing website located in `docs/` is automatically deployed on every push to `master` via [`.github/workflows/pages.yml`](.github/workflows/pages.yml). The live site is available at [https://lcajigasm.github.io/ownterm/](https://lcajigasm.github.io/ownterm/).
- **Zero-Telemetry Audit**: Every commit and pull request is scanned by [`.github/workflows/audit-services.yml`](.github/workflows/audit-services.yml) to prevent accidental introduction of remote endpoints, telemetry tracking, or proprietary services.

---

## Licensing and Trademarks

- **Code Licensing**: The main terminal client is licensed under the **GNU Affero General Public License v3.0** ([`LICENSE-AGPL`](LICENSE-AGPL)). Shared UI framework crates (`warpui`, `warpui_core`) retain the **MIT License** ([`LICENSE-MIT`](LICENSE-MIT)).
- **Attribution**: Fork of the Warp client (© Warp Services, Inc.). Original copyright and license notices stay in the tree. OwnTerm does not claim ownership of the upstream work.
- **Trademarks & Non-Affiliation**: OwnTerm is an independent open-source project and is not affiliated with, sponsored by, or endorsed by Warp Services, Inc. "Warp" is a trademark of Warp Services, Inc.
