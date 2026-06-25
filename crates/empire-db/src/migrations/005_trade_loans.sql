-- Migration 005: commodity market and peer-to-peer loan tables.
-- Ported from: include/commodity.h (struct comstr), include/loan.h (struct lonstr)

CREATE TABLE IF NOT EXISTS trade_items (
    uid         INTEGER PRIMARY KEY,
    seller      INTEGER NOT NULL,
    item        INTEGER NOT NULL,    -- Item enum discriminant (0-13)
    amount      INTEGER NOT NULL,
    price       REAL    NOT NULL,
    from_x      INTEGER NOT NULL DEFAULT 0,
    from_y      INTEGER NOT NULL DEFAULT 0,
    created     INTEGER NOT NULL DEFAULT 0,
    bought      INTEGER NOT NULL DEFAULT 0,   -- boolean: 0=listed, 1=sold
    buyer       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS loans (
    uid           INTEGER PRIMARY KEY,
    loaner        INTEGER NOT NULL,
    loanee        INTEGER NOT NULL,
    amount        REAL    NOT NULL,
    paid          REAL    NOT NULL DEFAULT 0,
    interest_rate REAL    NOT NULL DEFAULT 0.05,
    status        INTEGER NOT NULL DEFAULT 0,  -- LoanStatus enum (0=Offered..3=Defaulted)
    created       INTEGER NOT NULL DEFAULT 0,
    due           INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO schema_version(version) VALUES (5);
