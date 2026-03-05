# Claude-like Chat UI with Rust + Yew + Axum

This repository now contains a full-stack Rust starter that mirrors a Claude-style layout and supports pluggable LLM providers.

## What is implemented

- **Frontend (`frontend`)**: Yew single-page app with:
  - Claude-like dark split layout (sidebar, center composer, right settings panel)
  - Provider and model picker
  - Chat send flow to Rust backend
  - **Settings toggle** for built-in **agentic mode**
  - Agent list and "Create new agent" button
- **Backend (`backend`)**: Axum API with:
  - `/api/chat` completion endpoint
  - `/api/providers` provider registry endpoint
  - `/api/settings/agentic` toggle endpoint
  - `/api/agents` list/create custom agents endpoint
- **Provider abstraction** ready for third-party LLM integrations:
  - OpenAI
  - OpenRouter
  - NVIDIA NIM
  - Google Gemini
  - Claude/Anthropic
  - Ollama
  - LM Studio
  - CrewAI bridge provider

## Architecture notes

The backend uses a provider registry with a trait-based adapter pattern so new providers can be dropped in without changing core routes.

The built-in **CrewAI bridge** is represented as a dedicated provider (`crewai`) and an app-level `agentic_mode_enabled` setting. In production, connect it to a Python microservice that runs CrewAI crews from https://docs.crewai.com/.

The agent object schema and tools-first workflow are intentionally compatible with an OpenClaw-like orchestration style:
- explicit agent role metadata
- declared tool list per agent
- structured task prompt field (`system_prompt`)

## Run locally

### Backend

```bash
cargo run -p chat-backend
```

### Frontend

Install trunk (once):

```bash
cargo install trunk
```

Run frontend:

```bash
cd frontend
trunk serve --open
```

The frontend expects backend at `http://localhost:8080`.

## Environment variables (optional)

- `OPENAI_API_KEY`
- `OPENROUTER_API_KEY`
- `NVIDIA_API_KEY`
- `GEMINI_API_KEY`
- `ANTHROPIC_API_KEY`

If a key is missing, providers return deterministic mock responses for local development.

## Next steps for real agentic execution

1. Add a Python CrewAI worker service and call it from `CrewAiBridgeProvider`.
2. Persist agents/settings in a database.
3. Add streaming responses and multi-turn tool execution state.
4. Optionally import OpenClaw task graph conventions directly from a cloned module.
