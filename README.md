# Zavora OS (spatial-os)

Agentic operating system prototype — spatial field UI driven by server-sent events.

## Quick start

```bash
cp .env.example .env
cargo run
```

Open [http://localhost:8080](http://localhost:8080). Use `?demo=1` for offline simulation (no server SSE).

## Layout

| Path | Purpose |
|------|---------|
| `web/` | Field UI (`index.html`, `static/field-client.js`) |
| `src/` | Axum server, SSE orchestration, AWP routes |
| `audio/` | Prerecorded voice clips |
| `scripts/` | Demo capture (`capture.js`) and TTS generation (`gen_audio.py`) |
| `docs/` | Specification and implementation plan |
| `demo/` | Generated demo assets (gitignored) |
| `deploy/` | Deployment configs (M11) |

## API

- `POST /api/sessions` → `{ session_id, user_id }`
- `POST /api/sessions/{id}/intent` → SSE stream (`scenario`, `card_*`, `suzy_summary`, `done`)
- `GET /artifacts/{session_id}/{file}` — deck artifacts (.xlsx, .docx, .pptx)
- `GET /health`
- `GET /.well-known/awp.json`, `GET /awp/manifest` — AWP discovery

## Real deck workflow (M1)

Set `GOOGLE_API_KEY` in `.env` and build MCP servers:

```bash
(cd ../mcp-servers/worksheet-mcp && cargo build --release)
(cd ../mcp-servers/docx-mcp && cargo build --release)
(cd ../mcp-servers/mcp_slides && cargo build --release)
```

Without the API key, deck intents use mock SSE (M0 behavior).

## Docs

- [Specification](docs/SPECIFICATION.md)
- [Implementation plan](docs/IMPLEMENTATION_PLAN.md)