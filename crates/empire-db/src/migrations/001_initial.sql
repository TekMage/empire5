-- Empire 5 — initial database schema
-- Replaces empire4.x flat binary files with SQLite tables.
-- One row per game object; JSON columns for variable-length arrays (items, etc.)
-- Migration 001: initial schema

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- ============================================================
-- Nations (ref: struct natstr / nat.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS nations (
    uid         INTEGER PRIMARY KEY,   -- nat_uid = nat_cnum
    cnum        INTEGER NOT NULL UNIQUE CHECK(cnum >= 0 AND cnum < 255),
    status      INTEGER NOT NULL DEFAULT 0,  -- NatStatus enum
    flags       INTEGER NOT NULL DEFAULT 0,  -- NatFlags bitfield
    name        TEXT    NOT NULL DEFAULT '',
    representative TEXT NOT NULL DEFAULT '',
    host_addr   TEXT    NOT NULL DEFAULT '',
    user_id     TEXT    NOT NULL DEFAULT '',
    xcap        INTEGER NOT NULL DEFAULT 0,
    ycap        INTEGER NOT NULL DEFAULT 0,
    xorg        INTEGER NOT NULL DEFAULT 0,
    yorg        INTEGER NOT NULL DEFAULT 0,
    money       INTEGER NOT NULL DEFAULT 0,
    reserve     INTEGER NOT NULL DEFAULT 0,
    tech        REAL    NOT NULL DEFAULT 0.0,
    research    REAL    NOT NULL DEFAULT 0.0,
    education   REAL    NOT NULL DEFAULT 0.0,
    happiness   REAL    NOT NULL DEFAULT 0.0,
    login_count INTEGER NOT NULL DEFAULT 0,
    tele_cnt    INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- Realms (ref: struct realmstr / nat.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS realms (
    uid         INTEGER PRIMARY KEY,
    cnum        INTEGER NOT NULL REFERENCES nations(cnum),
    realm       INTEGER NOT NULL CHECK(realm >= 0 AND realm < 50),
    xl          INTEGER NOT NULL DEFAULT 0,
    xh          INTEGER NOT NULL DEFAULT 0,
    yl          INTEGER NOT NULL DEFAULT 0,
    yh          INTEGER NOT NULL DEFAULT 0,
    UNIQUE(cnum, realm)
);

-- ============================================================
-- Sectors (ref: struct sctstr / sect.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS sectors (
    uid         INTEGER PRIMARY KEY,   -- XYOFFSET(x, y)
    own         INTEGER NOT NULL DEFAULT 0,
    x           INTEGER NOT NULL,
    y           INTEGER NOT NULL,
    sector_type INTEGER NOT NULL DEFAULT 0,
    effic       INTEGER NOT NULL DEFAULT 0,
    mobil       INTEGER NOT NULL DEFAULT 0,
    off         INTEGER NOT NULL DEFAULT 0,
    loyal       INTEGER NOT NULL DEFAULT 0,
    terr0       INTEGER NOT NULL DEFAULT 0,
    terr1       INTEGER NOT NULL DEFAULT 0,
    terr2       INTEGER NOT NULL DEFAULT 0,
    terr3       INTEGER NOT NULL DEFAULT 0,
    dterr       INTEGER NOT NULL DEFAULT 0,
    dist_x      INTEGER NOT NULL DEFAULT 0,
    dist_y      INTEGER NOT NULL DEFAULT 0,
    avail       INTEGER NOT NULL DEFAULT 0,
    flags       INTEGER NOT NULL DEFAULT 0,
    elev        INTEGER NOT NULL DEFAULT 0,
    work        INTEGER NOT NULL DEFAULT 100,
    coastal     INTEGER NOT NULL DEFAULT 0,
    new_type    INTEGER NOT NULL DEFAULT 0,
    min_ore     INTEGER NOT NULL DEFAULT 0,
    gmin        INTEGER NOT NULL DEFAULT 0,
    fertil      INTEGER NOT NULL DEFAULT 0,
    oil         INTEGER NOT NULL DEFAULT 0,
    uran        INTEGER NOT NULL DEFAULT 0,
    old_own     INTEGER NOT NULL DEFAULT 0,
    items       TEXT    NOT NULL DEFAULT '[]',  -- JSON [i16; 14]
    mines       INTEGER NOT NULL DEFAULT 0,
    pstage      INTEGER NOT NULL DEFAULT 0,
    ptime       INTEGER NOT NULL DEFAULT 0,
    fallout     INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(x, y)
);

-- ============================================================
-- Ships (ref: struct shpstr / ship.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS ships (
    uid             INTEGER PRIMARY KEY,
    own             INTEGER NOT NULL DEFAULT 0,
    x               INTEGER NOT NULL DEFAULT 0,
    y               INTEGER NOT NULL DEFAULT 0,
    ship_type       INTEGER NOT NULL DEFAULT 0,
    effic           INTEGER NOT NULL DEFAULT 0,
    mobil           INTEGER NOT NULL DEFAULT 0,
    off             INTEGER NOT NULL DEFAULT 0,
    tech            INTEGER NOT NULL DEFAULT 0,
    fleet           TEXT    NOT NULL DEFAULT ' ',
    opx             INTEGER NOT NULL DEFAULT 0,
    opy             INTEGER NOT NULL DEFAULT 0,
    mission         INTEGER NOT NULL DEFAULT 0,
    mission_radius  INTEGER NOT NULL DEFAULT 0,
    items           TEXT    NOT NULL DEFAULT '[]',
    pstage          INTEGER NOT NULL DEFAULT 0,
    ptime           INTEGER NOT NULL DEFAULT 0,
    access          INTEGER NOT NULL DEFAULT 0,
    name            TEXT    NOT NULL DEFAULT '',
    orig_x          INTEGER NOT NULL DEFAULT 0,
    orig_y          INTEGER NOT NULL DEFAULT 0,
    orig_own        INTEGER NOT NULL DEFAULT 0,
    retreat_flags   INTEGER NOT NULL DEFAULT 0,
    retreat_path    TEXT    NOT NULL DEFAULT '',
    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- Planes (ref: struct plnstr / plane.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS planes (
    uid             INTEGER PRIMARY KEY,
    own             INTEGER NOT NULL DEFAULT 0,
    x               INTEGER NOT NULL DEFAULT 0,
    y               INTEGER NOT NULL DEFAULT 0,
    plane_type      INTEGER NOT NULL DEFAULT 0,
    effic           INTEGER NOT NULL DEFAULT 0,
    mobil           INTEGER NOT NULL DEFAULT 0,
    off             INTEGER NOT NULL DEFAULT 0,
    tech            INTEGER NOT NULL DEFAULT 0,
    wing            TEXT    NOT NULL DEFAULT ' ',
    opx             INTEGER NOT NULL DEFAULT 0,
    opy             INTEGER NOT NULL DEFAULT 0,
    mission         INTEGER NOT NULL DEFAULT 0,
    mission_radius  INTEGER NOT NULL DEFAULT 0,
    range           INTEGER NOT NULL DEFAULT 0,
    harden          INTEGER NOT NULL DEFAULT 0,
    ship            INTEGER NOT NULL DEFAULT -1,
    land            INTEGER NOT NULL DEFAULT -1,
    flags           INTEGER NOT NULL DEFAULT 0,
    access          INTEGER NOT NULL DEFAULT 0,
    theta           REAL    NOT NULL DEFAULT 0.0,
    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- Land Units (ref: struct lndstr / land.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS land_units (
    uid             INTEGER PRIMARY KEY,
    own             INTEGER NOT NULL DEFAULT 0,
    x               INTEGER NOT NULL DEFAULT 0,
    y               INTEGER NOT NULL DEFAULT 0,
    land_type       INTEGER NOT NULL DEFAULT 0,
    effic           INTEGER NOT NULL DEFAULT 0,
    mobil           INTEGER NOT NULL DEFAULT 0,
    off             INTEGER NOT NULL DEFAULT 0,
    tech            INTEGER NOT NULL DEFAULT 0,
    army            TEXT    NOT NULL DEFAULT ' ',
    opx             INTEGER NOT NULL DEFAULT 0,
    opy             INTEGER NOT NULL DEFAULT 0,
    mission         INTEGER NOT NULL DEFAULT 0,
    mission_radius  INTEGER NOT NULL DEFAULT 0,
    ship            INTEGER NOT NULL DEFAULT -1,
    harden          INTEGER NOT NULL DEFAULT 0,
    retreat         INTEGER NOT NULL DEFAULT 0,
    retreat_flags   INTEGER NOT NULL DEFAULT 0,
    retreat_path    TEXT    NOT NULL DEFAULT '',
    scar            INTEGER NOT NULL DEFAULT 0,
    items           TEXT    NOT NULL DEFAULT '[]',
    pstage          INTEGER NOT NULL DEFAULT 0,
    ptime           INTEGER NOT NULL DEFAULT 0,
    carried_by_land INTEGER NOT NULL DEFAULT -1,
    access          INTEGER NOT NULL DEFAULT 0,
    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- Nukes (ref: struct nukstr / nuke.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS nukes (
    uid         INTEGER PRIMARY KEY,
    own         INTEGER NOT NULL DEFAULT 0,
    x           INTEGER NOT NULL DEFAULT 0,
    y           INTEGER NOT NULL DEFAULT 0,
    nuke_type   INTEGER NOT NULL DEFAULT 0,
    effic       INTEGER NOT NULL DEFAULT 100,
    tech        INTEGER NOT NULL DEFAULT 0,
    stockpile   TEXT    NOT NULL DEFAULT ' ',
    plane       INTEGER NOT NULL DEFAULT -1,
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- News (ref: struct nwsstr / news.h)
-- ============================================================
CREATE TABLE IF NOT EXISTS news (
    uid         INTEGER PRIMARY KEY AUTOINCREMENT,
    item        INTEGER NOT NULL DEFAULT 0,
    actor       INTEGER NOT NULL DEFAULT 0,
    victim      INTEGER NOT NULL DEFAULT 0,
    times       INTEGER NOT NULL DEFAULT 1,
    happened_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- ============================================================
-- Telegrams / Messages
-- ============================================================
CREATE TABLE IF NOT EXISTS telegrams (
    uid         INTEGER PRIMARY KEY AUTOINCREMENT,
    to_cnum     INTEGER NOT NULL REFERENCES nations(cnum),
    from_cnum   INTEGER NOT NULL DEFAULT 0,
    sent_at     INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    body        TEXT    NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_telegrams_to ON telegrams(to_cnum);

-- ============================================================
-- Schema version tracking
-- ============================================================
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
INSERT OR IGNORE INTO schema_version(version) VALUES (1);
