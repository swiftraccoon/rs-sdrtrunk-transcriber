# WhisperX Transcription Service

## Installation Options

### Option 1: Native Installation (Recommended for GPU)

This is the preferred method if you want to use GPU acceleration.

```bash
# Create virtual environment
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Run service
python service.py
```

### Option 2: Docker (CPU-only recommended)

Docker is convenient for CPU-only deployments but NOT recommended for GPU due to:
- GPU memory conflicts with host applications
- Complex nvidia-docker setup
- Potential driver compatibility issues

```bash
# CPU-only container
docker-compose up whisperx

# GPU container (NOT RECOMMENDED - will monopolize GPU)
# docker-compose up whisperx-gpu
```

### Option 3: Systemd Service (Production)

For production deployments, use systemd to manage the Python service:

```bash
sudo cp whisperx.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable whisperx
sudo systemctl start whisperx
```

## Configuration

### Environment Variables

```bash
# Model settings
export WHISPERX_MODEL_SIZE=large-v3  # or base, small, medium, large-v2
export WHISPERX_DEVICE=cuda          # or cpu, mps (Apple Silicon)
export WHISPERX_COMPUTE_TYPE=float16 # or float32, int8

# Service settings
export WHISPERX_PORT=8001
export WHISPERX_WORKERS=1
export WHISPERX_LOG_LEVEL=INFO

# Optional: Hugging Face token for speaker diarization models
export WHISPERX_HF_TOKEN=hf_xxxxxxxxxxxxx
```

### GPU Memory Requirements

| Model    | VRAM Required | Speed (RTX 3090) |
|----------|--------------|------------------|
| tiny     | ~1 GB        | 39x realtime     |
| base     | ~1 GB        | 16x realtime     |
| small    | ~2 GB        | 6x realtime      |
| medium   | ~5 GB        | 3x realtime      |
| large-v2 | ~10 GB       | 1.5x realtime    |
| large-v3 | ~10 GB       | 1.5x realtime    |

## Running Without Docker (Recommended)

### Development Mode

```bash
# Activate virtual environment
source venv/bin/activate

# Run with default settings (CPU)
python service.py

# Run with GPU
WHISPERX_DEVICE=cuda python service.py

# Run with specific model
WHISPERX_MODEL_SIZE=medium WHISPERX_DEVICE=cuda python service.py
```

### Production Mode with PM2

```bash
# Install PM2
npm install -g pm2

# Start service
pm2 start ecosystem.config.js

# Monitor
pm2 monit

# Logs
pm2 logs whisperx
```

### Production Mode with Supervisor

```bash
# Install supervisor
sudo apt-get install supervisor

# Copy config
sudo cp whisperx.supervisor.conf /etc/supervisor/conf.d/

# Start
sudo supervisorctl reread
sudo supervisorctl update
sudo supervisorctl start whisperx
```

## Testing

```bash
# Test the service
python test_service.py

# Or use curl
curl http://localhost:8001/health
```

## Troubleshooting

### GPU Not Detected

1. Check CUDA installation:
```bash
nvidia-smi
python -c "import torch; print(torch.cuda.is_available())"
```

2. Install correct PyTorch version:
```bash
# For CUDA 11.8
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu118

# For CUDA 12.1
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
```

### Out of Memory Errors

1. Use smaller model: `WHISPERX_MODEL_SIZE=small`
2. Reduce batch size: `WHISPERX_BATCH_SIZE=8`
3. Use int8 quantization: `WHISPERX_COMPUTE_TYPE=int8`

### Port Already in Use

Change the port:
```bash
WHISPERX_PORT=8002 python service.py
```