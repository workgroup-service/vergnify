// @author Cursor

package main

import (
	"context"
	"database/sql"
)

func migrate(ctx context.Context, db *sql.DB) error {
	statements := []string{
		`CREATE TABLE IF NOT EXISTS hosts (
			id BIGSERIAL PRIMARY KEY,
			hostname TEXT NOT NULL,
			created_at TIMESTAMPTZ NOT NULL DEFAULT now()
		)`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS os TEXT NOT NULL DEFAULT ''`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS inventory JSONB NOT NULL DEFAULT '[]'::jsonb`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS reported_at TIMESTAMPTZ NOT NULL DEFAULT now()`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS org_id BIGINT`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS user_id BIGINT`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS token_hash TEXT`,
		`ALTER TABLE hosts ADD COLUMN IF NOT EXISTS revoked BOOLEAN NOT NULL DEFAULT false`,
		`DROP INDEX IF EXISTS hosts_hostname_key`,
		`CREATE UNIQUE INDEX IF NOT EXISTS hosts_token_hash_key ON hosts (token_hash) WHERE token_hash IS NOT NULL`,
		`CREATE TABLE IF NOT EXISTS organizations (
			id BIGSERIAL PRIMARY KEY,
			name TEXT NOT NULL,
			created_at TIMESTAMPTZ NOT NULL DEFAULT now()
		)`,
		`CREATE TABLE IF NOT EXISTS users (
			id BIGSERIAL PRIMARY KEY,
			org_id BIGINT NOT NULL REFERENCES organizations (id),
			email TEXT NOT NULL,
			password_hash TEXT,
			role TEXT NOT NULL,
			created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
			UNIQUE (org_id, email)
		)`,
		`CREATE TABLE IF NOT EXISTS sessions (
			id BIGSERIAL PRIMARY KEY,
			user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
			token_hash TEXT NOT NULL UNIQUE,
			expires_at TIMESTAMPTZ NOT NULL
		)`,
		`CREATE TABLE IF NOT EXISTS invitations (
			id BIGSERIAL PRIMARY KEY,
			org_id BIGINT NOT NULL REFERENCES organizations (id),
			email TEXT NOT NULL,
			token_hash TEXT NOT NULL UNIQUE,
			expires_at TIMESTAMPTZ NOT NULL,
			accepted_at TIMESTAMPTZ
		)`,
		`CREATE TABLE IF NOT EXISTS device_codes (
			id BIGSERIAL PRIMARY KEY,
			org_id BIGINT NOT NULL REFERENCES organizations (id),
			user_code TEXT NOT NULL,
			device_code_hash TEXT NOT NULL UNIQUE,
			hostname TEXT NOT NULL DEFAULT '',
			os TEXT NOT NULL DEFAULT '',
			user_id BIGINT REFERENCES users (id) ON DELETE CASCADE,
			host_id BIGINT,
			token TEXT,
			expires_at TIMESTAMPTZ NOT NULL
		)`,
		`ALTER TABLE device_codes DROP CONSTRAINT IF EXISTS device_codes_user_id_fkey`,
		`ALTER TABLE device_codes ADD CONSTRAINT device_codes_user_id_fkey FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE`,
	}
	for _, statement := range statements {
		if _, err := db.ExecContext(ctx, statement); err != nil {
			return err
		}
	}
	return nil
}
