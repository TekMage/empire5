-- Empire 5 — Migration 002: password hashes and login timestamps
-- Adds per-nation authentication fields absent from the initial schema.
-- ref: struct natstr / nat.h  (nat_pnam, nat_last_login, nat_last_logout)
--
-- passwd_hash: bcrypt hash of the nation password (empty = no password set)
-- last_login:  Unix timestamp of most recent successful login
-- last_logout: Unix timestamp of most recent logout

ALTER TABLE nations ADD COLUMN passwd_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE nations ADD COLUMN last_login  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE nations ADD COLUMN last_logout INTEGER NOT NULL DEFAULT 0;

INSERT OR IGNORE INTO schema_version(version) VALUES (2);
