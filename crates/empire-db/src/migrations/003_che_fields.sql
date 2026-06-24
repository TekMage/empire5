-- Empire 5 — Migration 003: add CHE (guerrilla) fields to sectors
--
-- Ported from: include/sect.h  sct_che / sct_che_target
--   che        — number of guerrilla fighters in this sector (0..255)
--   che_target — nation that the CHE are targeting (0 = none)

ALTER TABLE sectors ADD COLUMN che         INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sectors ADD COLUMN che_target  INTEGER NOT NULL DEFAULT 0;

INSERT OR IGNORE INTO schema_version(version) VALUES (3);
