-- Migration 006: Telegram type + announcement counters
-- Adds tel_type to telegrams, ann_cnt + last_ann_read to nations.

ALTER TABLE telegrams ADD COLUMN tel_type INTEGER NOT NULL DEFAULT 0;
ALTER TABLE nations   ADD COLUMN ann_cnt       INTEGER NOT NULL DEFAULT 0;
ALTER TABLE nations   ADD COLUMN last_ann_read INTEGER NOT NULL DEFAULT 0;

INSERT INTO schema_version(version) VALUES (6);
