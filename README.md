# ContextX Intelligence

**By Santhakumar K • Alpha X Solutions**

Local-first Claude usage monitor + context token saver for AI tools.

`contextx` is the command. **ContextX Intelligence** is the product: it combines Claude Usage Monitor-style live token tracking with ContextX-style reversible compression, MCP tools, a localhost proxy, CLI wrapping, auto-configuration helpers, and doctor checks in one Rust binary.

## What This Project Does

ContextX shows two numbers together:

- **Usage:** what Claude or another AI provider/tool is consuming.
- **Savings:** what ContextX saved before the request was sent.

Example:

```text
Claude / Provider Usage
Input sent:        18,500 tokens
Output received:   4,200 tokens
Burn rate:           820 tokens/min

ContextX Saver
Original input:   120,000 tokens
Compressed sent:   18,500 tokens
Saved:              84.6%
CCR originals:         12 retrievable
```

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│               CONTEXTX INTELLIGENCE                                       │
│               Santhakumar K • Alpha X Solutions                             │
│                                                                              │
│  Single local Rust binary: contextx                                          │
│  No Docker • No telemetry • Memory-only by default • Localhost-only API       │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ contextx daemon                                                              │
│ Shared local brain running on 127.0.0.1:8787                                 │
│                                                                              │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐     │
│  │ Compression Engine │  │ Usage Monitor      │  │ CCR Memory Cache   │     │
│  │ JSON / code / text │  │ session / weekly   │  │ originals in RAM   │     │
│  │ savings gate       │  │ burn rate / agents │  │ retrieve by key    │     │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘     │
│                                                                              │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐     │
│  │ Event Ring Buffer  │  │ Doctor / Installer │  │ Learning Observe   │     │
│  │ recent local stats │  │ safe config edits  │  │ suggestions only   │     │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────────┘
          ▲                    ▲                    ▲                    ▲
          │                    │                    │                    │
          │                    │                    │                    │
┌─────────┴─────────┐ ┌────────┴────────┐ ┌────────┴────────┐ ┌────────┴────────┐
│ MCP Mode          │ │ Proxy Mode      │ │ Wrapper Mode    │ │ CLI / TUI       │
│ contextx mcp      │ │ contextx proxy  │ │ contextx wrap   │ │ stats, doctor   │
│                   │ │                 │ │                 │ │ tui, install    │
│ Claude Desktop    │ │ OpenAI SDK      │ │ Claude Code     │ │ user dashboard  │
│ Cursor            │ │ Anthropic API   │ │ Codex CLI       │ │ setup checks    │
│ Cline             │ │ LangChain       │ │ Aider           │ │ safe auto-config│
│ Continue          │ │ Vercel AI SDK   │ │ other CLIs      │ │                 │
│ Zed               │ │ custom apps     │ │                 │ │                 │
└───────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘
```

## How The Combined Flow Works

```text
AI tool request
   │
   ├─ through MCP, proxy, or wrapper
   ▼
ContextX daemon
   │
   ├─ counts original input tokens
   ├─ compresses useful large context
   ├─ stores full original in CCR RAM cache
   ├─ sends smaller request onward
   ├─ records provider output usage when available
   └─ updates live stats / TUI
```

This means ContextX is both:

- a **Claude usage monitor** where usage data is visible or estimable
- a **token saver** where ContextX can compress the context before sending

## Quick Start

```bash
cargo run -- doctor
cargo run -- doctor --json
cargo run -- doctor --fix --dry-run
cargo run -- print-config --client claude-desktop
cargo run -- verify-client --client claude-desktop
cargo run -- daemon
cargo run -- mcp
cargo run -- proxy --port 8787
cargo run -- tui
```

## Commands

- `contextx mcp` - MCP stdio server exposing `contextx_compress`, `contextx_retrieve`, `contextx_stats`, and `contextx_learn`.
- `contextx daemon` - shared localhost brain for MCP, proxy, stats, and TUI.
- `contextx proxy` - localhost OpenAI/Anthropic-compatible proxy that compresses request messages and tracks usage.
- `contextx wrap <command>` - run a local AI CLI through ContextX's session tracker.
- `contextx tui` - lightweight live terminal dashboard.
- `contextx stats` - print memory-only usage snapshot.
- `contextx doctor` - inspect local setup and recommended next steps.
- `contextx doctor --json` - print machine-readable setup status.
- `contextx doctor --fix` - apply safe auto-configuration.
- `contextx print-config --client claude-desktop` - print config without editing files.
- `contextx verify-client --client claude-desktop` - verify an installed client config.
- `contextx install --all` - write safe MCP config snippets for supported local clients, backing up existing files first.
- `contextx install --client cursor` - configure one client only.

## Supported Tools

Connection paths:

- **MCP:** Claude Desktop, Cursor, Cline, Continue, Zed, custom MCP clients.
- **Proxy:** OpenAI SDK, Anthropic-style message clients, LangChain, Vercel AI SDK, custom Python/Node apps, curl.
- **Wrapper:** Claude Code, Codex CLI, Aider, and other terminal AI commands.
- **CLI:** local scripts, dashboards, setup verification, and automation.

Important: ContextX can only measure or save tokens for traffic it can see through MCP, proxy, wrapper, or direct CLI calls. Exact Claude account quota depends on what Claude exposes.

## Recommended Local Workflow

Run the shared daemon in one terminal:

```bash
cargo run -- daemon
```

Then connect tools to it:

```bash
cargo run -- mcp
cargo run -- tui
cargo run -- stats --watch
```

The daemon exposes local-only ContextX APIs:

- `GET /health`
- `GET /v1/contextx/stats`
- `POST /v1/contextx/compress`
- `POST /v1/contextx/retrieve`
- `POST /v1/contextx/learn`

## Privacy Defaults

- No cloud service.
- No telemetry.
- No prompt history database.
- CCR originals live only in process memory.
- Stats store counts, hashes, timing, model, provider, agent, and project metadata.
- HTTP services refuse non-localhost binds unless explicitly started with `--allow-non-localhost`.

Prompts still go to the AI provider selected by your client or proxy upstream.

## Identity

Project: **ContextX Intelligence**

Creator: **Santhakumar K**

Company: **Alpha X Solutions**

Command name: `contextx`

# ContextX-Intelligence
