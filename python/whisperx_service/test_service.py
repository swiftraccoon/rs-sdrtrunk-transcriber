"""Test script for WhisperX service."""

import asyncio
import json
from datetime import datetime
from pathlib import Path
from uuid import uuid4

import httpx


async def test_service():
    """Test the WhisperX service."""
    base_url = "http://localhost:8001"

    async with httpx.AsyncClient(timeout=300) as client:
        # 1. Check health
        print("1. Checking service health...")
        response = await client.get(f"{base_url}/health")
        health = response.json()
        print(f"Health: {json.dumps(health, indent=2)}")

        if not health["healthy"]:
            print("Service is not healthy!")
            return

        # 2. Get stats
        print("\n2. Getting service stats...")
        response = await client.get(f"{base_url}/stats")
        stats = response.json()
        print(f"Stats: {json.dumps(stats, indent=2)}")

        # 3. Test transcription
        print("\n3. Testing transcription...")

        # Create test request
        test_audio_path = Path("/tmp/test_audio.mp3")  # Replace with actual test file
        if not test_audio_path.exists():
            print(f"Test audio file not found: {test_audio_path}")
            print("Please provide a test audio file")
            return

        request = {
            "id": str(uuid4()),
            "call_id": str(uuid4()),
            "audio_path": str(test_audio_path),
            "requested_at": datetime.utcnow().isoformat(),
            "options": {
                "language": None,  # Auto-detect
                "diarize": True,
                "word_timestamps": True,
                "vad": True,
            },
            "retry_count": 0,
            "priority": 0,
        }

        print(f"Request: {json.dumps(request, indent=2)}")

        # Send transcription request
        print("\nSending transcription request...")
        response = await client.post(
            f"{base_url}/transcribe",
            json=request,
        )

        if response.status_code != 200:
            print(f"Error: {response.status_code} - {response.text}")
            return

        result = response.json()

        # Print results
        print(f"\n4. Transcription Results:")
        print(f"Status: {result['status']}")
        print(f"Language: {result.get('language', 'N/A')}")
        print(f"Processing time: {result.get('processing_time_ms', 'N/A')}ms")
        print(f"Confidence: {result.get('confidence', 'N/A')}")
        print(f"Speaker count: {result.get('speaker_count', 'N/A')}")

        if result.get("text"):
            print(f"\nTranscription:\n{result['text']}")

        if result.get("segments"):
            print(f"\nSegments ({len(result['segments'])}):")
            for seg in result["segments"][:3]:  # First 3 segments
                print(f"  [{seg['start']:.2f}s - {seg['end']:.2f}s] "
                      f"Speaker {seg.get('speaker', '?')}: {seg['text']}")

        if result.get("speaker_segments"):
            print(f"\nSpeaker segments ({len(result['speaker_segments'])}):")
            for seg in result["speaker_segments"][:5]:  # First 5 segments
                print(f"  {seg['speaker']}: {seg['start']:.2f}s - {seg['end']:.2f}s")

        # 5. Check request status
        print(f"\n5. Checking request status...")
        response = await client.get(f"{base_url}/status/{request['id']}")
        status = response.json()
        print(f"Status: {json.dumps(status, indent=2)}")


if __name__ == "__main__":
    asyncio.run(test_service())