-- Migration 007: news table
-- NOTE: Only the CREATE TABLE statement is here; the CREATE INDEX
-- is executed separately in lib.rs to avoid multi-statement raw_sql issues.
-- Stores news events: who did what to whom, how many times, when.
-- Corresponds to struct nwsstr / EF_NEWS in 4.4.1.

CREATE TABLE IF NOT EXISTS news (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    actor    INTEGER NOT NULL,
    verb     INTEGER NOT NULL,
    victim   INTEGER NOT NULL DEFAULT 0,
    times    INTEGER NOT NULL DEFAULT 1,
    when_ts  INTEGER NOT NULL
);
