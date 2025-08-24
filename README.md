# (W.I.P.) Rust SDRTrunk Transcriber

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.89.0%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2024-red.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)

A Rust application for transcribing SDRTrunk P25 radio recordings with advanced features including speaker diarization, live streaming, and multi-interface support.

## Features

- **Dual Input Methods**: Process MP3 files from directories or receive via Rdio-compatible API
- **Advanced Transcription**: WhisperX integration with speaker diarization
- **Live Streaming**: Icecast integration for real-time audio streaming
- **Multi-Interface**: Web UI (Leptos), Native Desktop (Tauri), REST API
- **High Performance**: Async Rust with Tokio, sub-100ms API response times
- **Production Ready**: Comprehensive monitoring, health checks, and horizontal scaling

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for complete system design and technical specifications.

## Development Standards

This project maintains strict quality standards:

- Rust 1.89.0 with Edition 2024
- 90% minimum test coverage
- Zero unsafe code policy
- Comprehensive CI/CD pipeline
- Professional documentation requirements

## Quick Start

### Prerequisites

- Rust 1.89.0
- PostgreSQL 15+
- FFmpeg
- Docker (optional, for containerized deployment)

### Installation

```bash
# Clone the repository
git clone https://github.com/swiftraccoon/rs-sdrtrunk-transcriber.git
cd rs-sdrtrunk-transcriber

# Install development tools
cargo install just
cargo install cargo-nextest
cargo install cargo-llvm-cov

# Run initial setup
just setup

# Run tests
just test

# Start development server
just dev
```

### Configuration

Copy the example configuration and adjust for your environment:

```bash
cp config.example.toml config.toml
```

Key configuration options:

- Database connection settings
- WhisperX service URL
- Icecast streaming configuration
- API authentication keys

## Project Structure

```
rs-sdrtrunk-transcriber/
├── crates/
│   ├── sdrtrunk-core/     # Core functionality
│   ├── sdrtrunk-api/      # REST API server
│   ├── sdrtrunk-cli/      # CLI interface
│   ├── sdrtrunk-web/      # Web UI (Leptos)
│   └── sdrtrunk-desktop/  # Desktop app (Tauri)
├── tests/                 # Integration tests
├── benches/              # Performance benchmarks
└── docs/                 # Documentation
```

## Development

### Common Commands

```bash
just fmt          # Format code
just lint         # Run clippy
just test         # Run all tests
just coverage     # Generate coverage report
just bench        # Run benchmarks
just doc          # Build documentation
just ci           # Run full CI pipeline locally
```

### Contributing

Please read our contributing guidelines and ensure:

- All tests pass with 90%+ coverage
- Code passes all linting checks
- Documentation is complete
- Changes follow conventional commits

## Performance Targets

| Metric | Target | Maximum |
|--------|--------|---------|
| API Response | 50ms | 100ms |
| File Processing | 2s | 5s |
| Transcription | 0.3x real-time | 0.5x real-time |
| Memory Usage | 2GB baseline | 4GB peak |
| Concurrent Uploads | 100+ | - |

## Deployment

### Docker Compose

```bash
docker-compose up -d
```

### Kubernetes

```bash
kubectl apply -f k8s/
```

See [deployment documentation](docs/deployment.md) for production deployment guidelines.

## API Documentation

The API follows the Rdio Scanner protocol for compatibility with SDRTrunk.

Key endpoints:

- `POST /api/call-upload` - Receive audio files from SDRTrunk
- `GET /api/calls` - List calls with filtering
- `GET /api/calls/{id}/transcription` - Get transcription with speaker diarization
- `WebSocket /api/stream/{system_id}` - Live audio streaming

Full API documentation available at `/api/docs` when running.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- SDRTrunk for radio recording capabilities
- WhisperX for advanced transcription with speaker diarization
- The Rust community for excellent libraries and tools

## Support

- [Issues](https://github.com/swiftraccoon/rs-sdrtrunk-transcriber/issues)
- [Discussions](https://github.com/swiftraccoon/rs-sdrtrunk-transcriber/discussions)
- [Documentation](https://github.com/swiftraccoon/rs-sdrtrunk-transcriber/wiki)
