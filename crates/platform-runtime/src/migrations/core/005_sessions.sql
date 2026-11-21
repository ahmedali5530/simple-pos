-- Harden sessions: hashed tokens and expiry. Plain token kept temporarily for migration.

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS token_hash TEXT;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ;

UPDATE sessions
SET token_hash = token
WHERE token_hash IS NULL AND token IS NOT NULL;

UPDATE sessions
SET expires_at = created_at + interval '30 days'
WHERE expires_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS sessions_token_hash_idx
  ON sessions (tenant_id, token_hash)
  WHERE token_hash IS NOT NULL;
