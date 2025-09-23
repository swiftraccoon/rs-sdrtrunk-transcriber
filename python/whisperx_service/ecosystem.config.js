module.exports = {
  apps: [{
    name: 'whisperx',
    script: 'service.py',
    interpreter: './venv/bin/python',
    instances: 1,
    autorestart: true,
    watch: false,
    max_memory_restart: '18G',
    env: {
      WHISPERX_MODEL_SIZE: 'large-v3',
      WHISPERX_DEVICE: 'cuda',
      WHISPERX_COMPUTE_TYPE: 'float16',
      WHISPERX_PORT: 8081,
      WHISPERX_LOG_LEVEL: 'INFO',
      WHISPERX_WORKERS: 1,
      PYTHONUNBUFFERED: 1
    },
    env_production: {
      WHISPERX_LOG_LEVEL: 'WARNING',
      WHISPERX_WORKERS: 2
    },
    error_file: 'logs/whisperx-error.log',
    out_file: 'logs/whisperx-out.log',
    log_file: 'logs/whisperx-combined.log',
    time: true
  }]
};