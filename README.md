# (W.I.P.) Rust SDRTrunk Transcriber

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.89.0%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2024-red.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)

Transcribes SDRTrunk P25 radio recordings using Whisper Large v3. API server receives uploads from SDRTrunk, standalone workers transcribe via a PostgreSQL job queue. Scales horizontally in Kubernetes.

## Architecture

```
SDRTrunk → POST /api/call-upload → API Server → PostgreSQL job queue
                                                        ↓
                                              Worker pods (N replicas)
                                                        ↓
                                              Whisper Large v3 (CPU)
                                                        ↓
                                              Results → radio_calls table → Web UI
```

**Crates:**
- `sdrtrunk-types` — Validated newtypes (SystemId, TalkgroupId, etc.)
- `sdrtrunk-protocol` — Configuration types, business logic
- `sdrtrunk-storage` — PostgreSQL models, queries, job queue
- `sdrtrunk-transcriber` — Whisper service trait + implementations
- `sdrtrunk-api` — REST API server (Axum)
- `sdrtrunk-worker` — Standalone transcription worker binary
- `sdrtrunk-monitor` — File system monitoring
- `sdrtrunk-web` — Web UI (Leptos)

## Prerequisites

- Rust 1.89.0+
- PostgreSQL 15+
- FFmpeg (for audio conversion in worker)
- Podman (for K8s deployment)

## Quick Start (Local Development)

```bash
# Install tools
cargo install just cargo-nextest

# Database
just db-setup      # PostgreSQL in Podman on port 5433
just db-migrate    # Create tables

# API server (receives uploads from SDRTrunk)
cargo run -p sdrtrunk-api

# Transcription worker (processes jobs from DB queue)
# Requires Whisper model: set WHISPER_MODEL_PATH=/path/to/ggml-large-v3.bin
cargo run -p sdrtrunk-worker

# Web UI (optional)
cargo run -p sdrtrunk-web
```

## Configuration

```bash
cp config.example.toml config.toml
```

Required: set `database.url` to your PostgreSQL connection string.

See `config.example.toml` for all options.

## K8s Deployment

```bash
# Create storage (NFS PVCs for model + audio)
kubectl apply -f k8s/storage.yaml

# Download Whisper Large v3 model (~3GB)
kubectl apply -f k8s/download-model.yaml

# Deploy worker(s)
kubectl apply -f k8s/worker.yaml

# Scale transcription
kubectl scale deployment sdrtrunk-worker --replicas=10
```

Workers claim jobs from PostgreSQL using `SELECT ... FOR UPDATE SKIP LOCKED`. Each loads the 3GB Whisper model into RAM (~4GB per worker). No shared filesystem needed — audio bytes are stored in the job queue.

### Environment Variables (K8s)

```yaml
SDRTRUNK__DATABASE__URL: "postgresql://user:pass@host:5432/db"
SDRTRUNK__TRANSCRIPTION__ENABLED: "true"
WHISPER_MODEL_PATH: "/models/ggml-large-v3.bin"
```

Uses `__` as nested config separator.

## API Endpoints

- `POST /api/call-upload` — Rdio Scanner compatible upload
- `GET /api/calls` — List calls with filtering
- `GET /api/calls/{id}` — Call detail with transcription
- `GET /api/queue/stats` — Job queue statistics
- `POST /api/v1/transcription/callback` — Webhook (legacy)

## Development

```bash
just fmt          # Format code
just lint         # Run clippy
just test         # Run all tests
just lint-strict  # Run full lint.sh (clippy + docs + fmt + audit + deny + machete)
```

## License

GPL-3.0
