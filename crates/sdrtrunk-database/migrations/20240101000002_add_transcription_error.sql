-- Add transcription_error column for tracking transcription failures
ALTER TABLE radio_calls 
ADD COLUMN IF NOT EXISTS transcription_error TEXT;