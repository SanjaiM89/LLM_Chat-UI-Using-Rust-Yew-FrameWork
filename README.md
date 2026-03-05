# Claude-like Chat UI with Rust + Yew + Axum

This repository contains a full-stack Rust app with a Claude-style Yew UI and a pluggable Axum backend for multi-provider LLM chat plus agentic execution.

## What is implemented

- **Frontend (`frontend`)**:
  - Claude-like three-pane layout
  - Dynamic provider list from backend
  - Chat composer and response panel
  - Agentic controls: objective input, selected agent, strategy selector, run trigger
  - Agent creation form (OpenClaw-inspired tool-first defaults)
  - Recent run history panel
- **Backend (`backend`)**:
  - `GET /api/health`
  - `GET /api/providers`
  - `POST /api/chat`
  - `GET /api/settings`
  - `POST /api/settings/agentic`
  - `GET/POST /api/agents`
  - `GET/POST /api/agentic/runs`
  - `GET /api/agent-templates/openclaw`

## Providers supported

- OpenAI
- OpenRouter
- NVIDIA NIM
- Google Gemini
- Anthropic Claude
- Ollama
- LM Studio
- CrewAI bridge provider key (`crewai`)

When API keys are not configured, remote providers return deterministic mock responses for local development.

## Agentic framework improvements

- Strategy-based execution:
  - `native`: local step planner + provider execution loop
  - `crew_ai_bridge`: forwards structured run payload to `CREWAI_BRIDGE_URL`
- Configurable settings persisted in-memory:
  - `agentic_mode_enabled`
  - `default_agent_id`
  - `agentic_strategy`
  - `crewai_bridge_url`
- Structured run objects with step-level outputs and run history.
- OpenClaw-inspired template endpoint for plan/execute/verify/final-report workflow scaffolding.

## Run locally

### Backend

```bash
cargo run -p chat-backend
```

### Frontend

Install trunk once:

```bash
cargo install trunk
```

Then:

```bash
cd frontend
trunk serve --open
```

Frontend expects backend at `http://localhost:8080`.

## Environment variables

- `OPENAI_API_KEY`
- `OPENROUTER_API_KEY`
- `NVIDIA_API_KEY`
- `GEMINI_API_KEY`
- `ANTHROPIC_API_KEY`
- `CREWAI_BRIDGE_URL`
