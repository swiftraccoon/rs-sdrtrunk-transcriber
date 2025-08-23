-- Initialize test database for CI/CD
-- This script runs when the PostgreSQL container starts

-- Create test user if not exists (backup in case env vars don't work)
DO
$do$
BEGIN
   IF NOT EXISTS (
      SELECT FROM pg_catalog.pg_user
      WHERE usename = 'sdrtrunk_test') THEN
      CREATE USER sdrtrunk_test WITH PASSWORD 'test_password';
   END IF;
END
$do$;

-- Grant all privileges on test database
GRANT ALL PRIVILEGES ON DATABASE sdrtrunk_test TO sdrtrunk_test;

-- Create schema
CREATE SCHEMA IF NOT EXISTS public;
GRANT ALL ON SCHEMA public TO sdrtrunk_test;

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Switch to test database
\c sdrtrunk_test;

-- Grant schema permissions
GRANT CREATE ON SCHEMA public TO sdrtrunk_test;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO sdrtrunk_test;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO sdrtrunk_test;