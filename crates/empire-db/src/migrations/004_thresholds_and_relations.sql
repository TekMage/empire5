-- Migration 004: per-sector distribution thresholds + diplomatic relations table.
-- Ported from: include/sect.h (sct_dist[]), include/nat.h (struct relatstr)

-- Distribution thresholds: JSON array of 14 i16 values (one per Item enum index).
ALTER TABLE sectors ADD COLUMN thresholds_json TEXT NOT NULL DEFAULT '[]';

-- Diplomatic relations between nations.
-- relate: 0=AT_WAR, 1=HOSTILE, 2=NEUTRAL, 3=FRIENDLY, 4=ALLIED
-- Default is NEUTRAL (2) for pairs not in this table.
CREATE TABLE IF NOT EXISTS relations (
    cnum    INTEGER NOT NULL,
    target  INTEGER NOT NULL,
    relate  INTEGER NOT NULL DEFAULT 2,
    PRIMARY KEY (cnum, target)
);

INSERT OR IGNORE INTO schema_version(version) VALUES (4);
