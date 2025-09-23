# Multi-stage build for WhisperX service
FROM python:3.11-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Create virtual environment
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Copy requirements and install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -U pip wheel setuptools && \
    pip install --no-cache-dir -r requirements.txt

# Production stage
FROM python:3.11-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Copy virtual environment from builder
COPY --from=builder /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Create non-root user
RUN useradd -m -u 1000 whisperx

# Create directories
RUN mkdir -p /app /home/whisperx/.cache/whisperx /tmp/whisperx && \
    chown -R whisperx:whisperx /app /home/whisperx/.cache/whisperx /tmp/whisperx

# Set working directory
WORKDIR /app

# Copy application code
COPY --chown=whisperx:whisperx . .

# Switch to non-root user
USER whisperx

# Environment variables
ENV PYTHONUNBUFFERED=1 \
    WHISPERX_MODEL_CACHE_DIR=/home/whisperx/.cache/whisperx \
    WHISPERX_TEMP_DIR=/tmp/whisperx

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD python -c "import httpx; r = httpx.get('http://localhost:8001/health'); exit(0 if r.json()['healthy'] else 1)"

# Expose port
EXPOSE 8001

# Run service
CMD ["python", "-m", "uvicorn", "service:app", "--host", "0.0.0.0", "--port", "8001"]