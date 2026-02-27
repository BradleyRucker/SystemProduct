# SystemProduct

A desktop systems engineering tool built with [Tauri](https://tauri.app/) (Rust + SvelteKit). Supports requirements management, traceability, architecture diagrams, simulation, and AI-assisted analysis.

## Features

- **Requirements management** — import from DOCX/text, NLP-assisted extraction, traceability matrix
- **Architecture canvas** — SysML-style block diagrams with ELK layout
- **Subsystem management** — per-subsystem requirements, knowledge, and artifacts
- **Simulation** — discrete-event simulation with scenario scripting
- **AI assistance** — Anthropic Claude or Ollama for requirement extraction and quality review
- **Authentication** — email/password and Google OAuth via Nhost
- **Baselines & reviews** — snapshot and review workflow

## Prerequisites

| Tool | Version |
|------|---------|
| [Rust](https://rustup.rs/) | stable (1.78+) |
| [Node.js](https://nodejs.org/) | 18+ |
| [Python](https://www.python.org/) | 3.9+ (for sidecar) |

## Getting Started

### 1. Clone and install dependencies

```bash
git clone <your-repo-url>
cd SystemProduct
npm install
```

### 2. Configure environment

```bash
cp .env.example .env
```

Edit `.env` and fill in your [Nhost](https://nhost.io/) project credentials:

```
VITE_NHOST_SUBDOMAIN=your-project-subdomain
VITE_NHOST_REGION=us-east-1
VITE_NHOST_REDIRECT_TO=http://localhost:1420/
```

See `AUTH_SETUP.md` for full authentication configuration including Google OAuth setup.

### 3. Set up the Python sidecar

The requirements parser sidecar is a Python script bundled as a standalone executable for releases. In dev mode the script is called directly.

**Install spaCy (optional but improves extraction quality):**
```bash
pip install spacy
python -m spacy download en_core_web_sm
```

**Build the sidecar binary (required for release builds):**
```bash
cd sidecar
build_sidecar.bat   # Windows
```

### 4. Download LLaMA binaries (optional — for local AI)

The local LLM feature uses [llama.cpp](https://github.com/ggerganov/llama.cpp). Download the Windows CPU build and extract it to `src-tauri/resources/llama/`. Place a compatible GGUF model in `src-tauri/resources/models/`.

These files are not tracked in git because of their size (~400 MB+).

### 5. Run in development

```bash
npm run tauri dev
```

This starts the Vite dev server on port 1420 and launches the Tauri window.

## AI Configuration

AI features work with either:

- **Anthropic Claude** — set your API key in the app's Settings page, or set the `ANTHROPIC_API_KEY` environment variable before launching.
- **Ollama** — install [Ollama](https://ollama.com/), pull a model (e.g. `ollama pull qwen2.5:7b`), and configure the model in Settings.
- **Local LLM** — place a GGUF model in `src-tauri/resources/models/` (see step 4 above).

AI is optional; all core features work without it.

## Building for Release

```bash
npm run tauri build
```

> **Note:** the sidecar binary (`sidecar/build_sidecar.bat`) must be built and copied to `src-tauri/binaries/` before releasing.

## Project Structure

```
src/                  # SvelteKit frontend
  lib/                # Shared components, stores, types
  routes/             # Pages (projects, canvas, requirements, simulation, …)
src-tauri/            # Tauri + Rust backend
  src/
    ai/               # AI providers (Anthropic, Ollama, local LLM, GraphRAG)
    commands/         # Tauri commands exposed to the frontend
    core/             # Database store (SQLite via SQLx)
    diagrams/         # Diagram model and layout
  resources/
    llama/            # llama.cpp binaries (not in git)
    models/           # GGUF model files (not in git)
  migrations/         # SQLite migrations
sidecar/              # Python requirement-parser sidecar
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `VITE_NHOST_SUBDOMAIN` | Nhost project subdomain |
| `VITE_NHOST_REGION` | Nhost project region (e.g. `us-east-1`) |
| `VITE_NHOST_REDIRECT_TO` | OAuth redirect URL (e.g. `http://localhost:1420/`) |
| `ANTHROPIC_API_KEY` | Optional — overrides the key stored in the app database |
