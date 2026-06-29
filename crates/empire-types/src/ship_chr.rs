// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// Empire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/global/ship.config, include/ship.h (struct mchrstr)
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998
//    Markus Armbruster, 2006-2016

// Ship characteristic table.
// Rust equivalent of the C `mchr[]` array (struct mchrstr), compiled in from
// the values in ship.config rather than loaded at runtime.
//
// Use `ShipChr::for_type(idx)` or `ShipChr::all()` to access entries.

bitflags::bitflags! {
    /// Ship capability flags.  Correspond to M_* constants in include/ship.h.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShipChrFlags: u32 {
        /// Fishing: can catch fish (M_FOOD).
        const FISH        = 1 << 0;
        /// Torpedo: can fire torpedoes (M_TORP).
        const TORP        = 1 << 1;
        /// Depth charge: can attack submarines (M_DCH).
        const DCH         = 1 << 2;
        /// Carrier: can launch and recover planes (M_FLY).
        const CARRIER     = 1 << 3;
        /// Missile: can launch missiles (M_MSL).
        const MISSILE     = 1 << 4;
        /// Oil: can drill for oil (M_OIL).
        const OIL         = 1 << 5;
        /// Sonar: can detect submarines (M_SONAR).
        const SONAR       = 1 << 6;
        /// Mine layer: can drop sea mines (M_MINE).
        const MINE_LAYER  = 1 << 7;
        /// Mine sweep: can sweep mines (M_SWEEP).
        const MINE_SWEEP  = 1 << 8;
        /// Submarine: this ship is a submarine (M_SUB).
        const SUBMARINE   = 1 << 9;
        /// Landing ship: allows full landing ability (M_LAND).
        const LAND        = 1 << 11;
        /// Sub-torp: can torpedo other submarines (M_SUBT).
        const SUB_TORP    = 1 << 12;
        /// Trade ship (M_TRADE).
        const TRADE       = 1 << 13;
        /// Semi-land: can land 1/4 load (M_SEMILAND).
        const SEMI_LAND   = 1 << 14;
        /// Supply: can supply other units/sectors/ships (M_SUPPLY).
        const SUPPLY      = 1 << 18;
        /// Canal: can navigate a canal (big city) (M_CANAL).
        const CANAL       = 1 << 19;
        /// Anti-missile: can shoot down missiles (M_ANTIMISSILE).
        const ANTI_MISSILE = 1 << 20;
    }
}

/// Per-ship-type descriptor.  ref: struct mchrstr in include/ship.h.
///
/// Values are compiled in from `src/lib/global/ship.config`.
#[derive(Debug, Clone, Copy)]
pub struct ShipChr {
    /// Full name (e.g. "fishing boat").
    pub name: &'static str,
    /// Short type abbreviation (e.g. "fb").
    pub sname: &'static str,
    /// LCM required to build to 100% efficiency.
    pub lcm: i32,
    /// HCM required to build to 100% efficiency.
    pub hcm: i32,
    /// Work units required to build to 100% efficiency.
    pub bwork: i32,
    /// Minimum tech level required to build.
    pub tech: i32,
    /// Build cost (dollars).
    pub cost: i32,
    /// Armor rating.
    pub armor: i32,
    /// Speed (sectors per mobility point).
    pub speed: i32,
    /// Visibility (how easily spotted).
    pub visib: i32,
    /// Visibility range (how far the ship can see).
    pub vrnge: i32,
    /// Firing range (sectors).
    pub frnge: i32,
    /// Gun limit (maximum guns that can fire).
    pub glim: i32,
    /// Maximum extra-light planes carried.
    pub nxlight: u8,
    /// Maximum choppers carried.
    pub nchoppers: u8,
    /// Maximum planes carried.
    pub nplanes: u8,
    /// Maximum land units carried.
    pub nland: u8,
    /// Capability flags.
    pub flags: ShipChrFlags,
}

// Flag shorthand constants used in the table below.
const FISH:         ShipChrFlags = ShipChrFlags::FISH;
const TORP:         ShipChrFlags = ShipChrFlags::TORP;
const DCH:          ShipChrFlags = ShipChrFlags::DCH;
const CARRIER:      ShipChrFlags = ShipChrFlags::CARRIER;
const MSL:          ShipChrFlags = ShipChrFlags::MISSILE;
const OIL:          ShipChrFlags = ShipChrFlags::OIL;
const SONAR:        ShipChrFlags = ShipChrFlags::SONAR;
const MINE:         ShipChrFlags = ShipChrFlags::MINE_LAYER;
const SWEEP:        ShipChrFlags = ShipChrFlags::MINE_SWEEP;
const SUB:          ShipChrFlags = ShipChrFlags::SUBMARINE;
const LAND:         ShipChrFlags = ShipChrFlags::LAND;
const SUBT:         ShipChrFlags = ShipChrFlags::SUB_TORP;
const SEMI:         ShipChrFlags = ShipChrFlags::SEMI_LAND;
const SUPPLY:       ShipChrFlags = ShipChrFlags::SUPPLY;
const CANAL:        ShipChrFlags = ShipChrFlags::CANAL;
const ANTIMSL:      ShipChrFlags = ShipChrFlags::ANTI_MISSILE;

/// Static ship characteristic table.  Indices match the type numbers in
/// ship.config.  Index 5 (trade ship) is commented out in ship.config but
/// we include a placeholder so that indices remain stable.
static MCHR: &[ShipChr] = &[
    // 0: fishing boat (fb)
    ShipChr {
        name: "fishing boat", sname: "fb",
        lcm: 25, hcm: 15, bwork: 75, tech: 0, cost: 180,
        armor: 10, speed: 10, visib: 15, vrnge: 2, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(FISH.bits() | CANAL.bits()),
    },
    // 1: fishing trawler (ft)
    ShipChr {
        name: "fishing trawler", sname: "ft",
        lcm: 25, hcm: 15, bwork: 75, tech: 35, cost: 300,
        armor: 10, speed: 25, visib: 15, vrnge: 2, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(FISH.bits() | CANAL.bits()),
    },
    // 2: cargo ship (cs)
    ShipChr {
        name: "cargo ship", sname: "cs",
        lcm: 60, hcm: 40, bwork: 160, tech: 20, cost: 500,
        armor: 20, speed: 25, visib: 35, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: SUPPLY,
    },
    // 3: ore ship (os)
    ShipChr {
        name: "ore ship", sname: "os",
        lcm: 60, hcm: 40, bwork: 160, tech: 20, cost: 500,
        armor: 20, speed: 25, visib: 35, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::empty(),
    },
    // 4: slave ship (ss)
    ShipChr {
        name: "slave ship", sname: "ss",
        lcm: 60, hcm: 40, bwork: 160, tech: 0, cost: 300,
        armor: 20, speed: 10, visib: 35, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::empty(),
    },
    // 5: trade ship (ts) — disabled in ship.config; placeholder
    ShipChr {
        name: "trade ship", sname: "ts",
        lcm: 200, hcm: 100, bwork: 420, tech: 30, cost: 1750,
        armor: 20, speed: 25, visib: 35, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::empty(),
    },
    // 6: frigate (frg)
    ShipChr {
        name: "frigate", sname: "frg",
        lcm: 30, hcm: 30, bwork: 110, tech: 0, cost: 600,
        armor: 50, speed: 25, visib: 25, vrnge: 3, frnge: 1, glim: 1,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: SEMI,
    },
    // 7: oil exploration boat (oe)
    ShipChr {
        name: "oil exploration boat", sname: "oe",
        lcm: 25, hcm: 15, bwork: 75, tech: 40, cost: 800,
        armor: 10, speed: 25, visib: 15, vrnge: 2, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(OIL.bits() | CANAL.bits()),
    },
    // 8: oil derrick (od)
    ShipChr {
        name: "oil derrick", sname: "od",
        lcm: 60, hcm: 60, bwork: 200, tech: 50, cost: 1500,
        armor: 30, speed: 15, visib: 65, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 2, nchoppers: 0, nplanes: 0, nland: 0,
        flags: OIL,
    },
    // 9: patrol boat (pt)
    ShipChr {
        name: "patrol boat", sname: "pt",
        lcm: 20, hcm: 10, bwork: 60, tech: 40, cost: 300,
        armor: 10, speed: 38, visib: 10, vrnge: 2, frnge: 1, glim: 1,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | CANAL.bits()),
    },
    // 10: light cruiser (lc)
    ShipChr {
        name: "light cruiser", sname: "lc",
        lcm: 30, hcm: 40, bwork: 130, tech: 45, cost: 800,
        armor: 50, speed: 30, visib: 30, vrnge: 5, frnge: 6, glim: 3,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: MINE,
    },
    // 11: heavy cruiser (hc)
    ShipChr {
        name: "heavy cruiser", sname: "hc",
        lcm: 40, hcm: 50, bwork: 160, tech: 50, cost: 1200,
        armor: 70, speed: 30, visib: 30, vrnge: 5, frnge: 8, glim: 4,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 4,
        flags: ShipChrFlags::empty(),
    },
    // 12: troop transport (tt)
    ShipChr {
        name: "troop transport", sname: "tt",
        lcm: 50, hcm: 50, bwork: 170, tech: 10, cost: 800,
        armor: 60, speed: 20, visib: 35, vrnge: 3, frnge: 1, glim: 2,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: SEMI,
    },
    // 13: battleship (bb)
    ShipChr {
        name: "battleship", sname: "bb",
        lcm: 50, hcm: 70, bwork: 210, tech: 45, cost: 1800,
        armor: 95, speed: 25, visib: 35, vrnge: 6, frnge: 10, glim: 7,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: ShipChrFlags::empty(),
    },
    // 14: battlecruiser (bbc)
    ShipChr {
        name: "battlecruiser", sname: "bbc",
        lcm: 50, hcm: 60, bwork: 190, tech: 75, cost: 1500,
        armor: 55, speed: 30, visib: 35, vrnge: 6, frnge: 10, glim: 6,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
        flags: ShipChrFlags::empty(),
    },
    // 15: tanker (tk)
    ShipChr {
        name: "tanker", sname: "tk",
        lcm: 60, hcm: 40, bwork: 160, tech: 35, cost: 600,
        armor: 75, speed: 25, visib: 45, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 0,
        flags: SUPPLY,
    },
    // 16: minesweeper (ms)
    ShipChr {
        name: "minesweeper", sname: "ms",
        lcm: 25, hcm: 15, bwork: 75, tech: 40, cost: 400,
        armor: 10, speed: 25, visib: 15, vrnge: 2, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MINE.bits() | SWEEP.bits() | CANAL.bits()),
    },
    // 17: destroyer (dd)
    ShipChr {
        name: "destroyer", sname: "dd",
        lcm: 30, hcm: 30, bwork: 110, tech: 70, cost: 600,
        armor: 45, speed: 35, visib: 20, vrnge: 4, frnge: 6, glim: 3,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 1,
        flags: ShipChrFlags::from_bits_truncate(DCH.bits() | SONAR.bits() | MINE.bits()),
    },
    // 18: submarine (sb)
    ShipChr {
        name: "submarine", sname: "sb",
        lcm: 30, hcm: 30, bwork: 110, tech: 60, cost: 650,
        armor: 25, speed: 20, visib: 5, vrnge: 4, frnge: 3, glim: 3,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | SONAR.bits() | MINE.bits() | SUB.bits()),
    },
    // 19: cargo submarine (sbc)
    ShipChr {
        name: "cargo submarine", sname: "sbc",
        lcm: 40, hcm: 40, bwork: 140, tech: 150, cost: 1200,
        armor: 50, speed: 30, visib: 2, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(SONAR.bits() | SUB.bits() | SUPPLY.bits()),
    },
    // 20: light carrier (cal)
    ShipChr {
        name: "light carrier", sname: "cal",
        lcm: 50, hcm: 60, bwork: 190, tech: 80, cost: 2700,
        armor: 60, speed: 30, visib: 40, vrnge: 5, frnge: 2, glim: 2,
        nxlight: 4, nchoppers: 20, nplanes: 20, nland: 0,
        flags: CARRIER,
    },
    // 21: aircraft carrier (car)
    ShipChr {
        name: "aircraft carrier", sname: "car",
        lcm: 60, hcm: 70, bwork: 220, tech: 160, cost: 4500,
        armor: 80, speed: 35, visib: 40, vrnge: 7, frnge: 2, glim: 2,
        nxlight: 10, nchoppers: 40, nplanes: 40, nland: 0,
        flags: CARRIER,
    },
    // 22: nuclear carrier (can)
    ShipChr {
        name: "nuc carrier", sname: "can",
        lcm: 70, hcm: 80, bwork: 250, tech: 305, cost: 8000,
        armor: 100, speed: 45, visib: 40, vrnge: 9, frnge: 2, glim: 2,
        nxlight: 20, nchoppers: 4, nplanes: 60, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(CARRIER.bits() | SUPPLY.bits()),
    },
    // 23: landing ship (ls)
    ShipChr {
        name: "landing ship", sname: "ls",
        lcm: 60, hcm: 40, bwork: 160, tech: 145, cost: 1000,
        armor: 40, speed: 30, visib: 30, vrnge: 2, frnge: 0, glim: 0,
        nxlight: 2, nchoppers: 0, nplanes: 0, nland: 6,
        flags: LAND,
    },
    // 24: asw frigate (af)
    ShipChr {
        name: "asw frigate", sname: "af",
        lcm: 40, hcm: 30, bwork: 120, tech: 220, cost: 800,
        armor: 50, speed: 35, visib: 30, vrnge: 5, frnge: 2, glim: 2,
        nxlight: 4, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | DCH.bits() | SONAR.bits() | SUBT.bits()),
    },
    // 25: nuclear attack sub (na)
    ShipChr {
        name: "nuc attack sub", sname: "na",
        lcm: 30, hcm: 40, bwork: 130, tech: 260, cost: 1200,
        armor: 45, speed: 40, visib: 3, vrnge: 6, frnge: 5, glim: 3,
        nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | SONAR.bits() | MINE.bits() | SUB.bits() | SUBT.bits()),
    },
    // 26: asw destroyer (ad)
    ShipChr {
        name: "asw destroyer", sname: "ad",
        lcm: 40, hcm: 40, bwork: 140, tech: 240, cost: 1500,
        armor: 60, speed: 40, visib: 35, vrnge: 6, frnge: 8, glim: 3,
        nxlight: 10, nchoppers: 2, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | DCH.bits() | SONAR.bits() | SUBT.bits()),
    },
    // 27: nuclear missile sub (nm)
    ShipChr {
        name: "nuc miss sub", sname: "nm",
        lcm: 30, hcm: 40, bwork: 130, tech: 270, cost: 1500,
        armor: 55, speed: 35, visib: 2, vrnge: 6, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 20, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MSL.bits() | SONAR.bits() | SUB.bits()),
    },
    // 28: missile sub (msb)
    ShipChr {
        name: "missile sub", sname: "msb",
        lcm: 30, hcm: 30, bwork: 110, tech: 230, cost: 1200,
        armor: 35, speed: 30, visib: 3, vrnge: 3, frnge: 0, glim: 0,
        nxlight: 0, nchoppers: 0, nplanes: 10, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MSL.bits() | SONAR.bits() | SUB.bits()),
    },
    // 29: missile boat (mb)
    ShipChr {
        name: "missile boat", sname: "mb",
        lcm: 20, hcm: 20, bwork: 80, tech: 180, cost: 500,
        armor: 15, speed: 40, visib: 15, vrnge: 3, frnge: 2, glim: 2,
        nxlight: 0, nchoppers: 0, nplanes: 10, nland: 0,
        flags: MSL,
    },
    // 30: missile frigate (mf)
    ShipChr {
        name: "missile frigate", sname: "mf",
        lcm: 40, hcm: 30, bwork: 120, tech: 280, cost: 1000,
        armor: 50, speed: 35, visib: 30, vrnge: 5, frnge: 2, glim: 2,
        nxlight: 2, nchoppers: 0, nplanes: 20, nland: 0,
        flags: MSL,
    },
    // 31: missile cruiser (mc)
    ShipChr {
        name: "missile cruiser", sname: "mc",
        lcm: 50, hcm: 50, bwork: 170, tech: 290, cost: 1500,
        armor: 70, speed: 35, visib: 35, vrnge: 8, frnge: 8, glim: 6,
        nxlight: 8, nchoppers: 8, nplanes: 40, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MSL.bits() | ANTIMSL.bits()),
    },
    // 32: aa cruiser (aac)
    ShipChr {
        name: "aa cruiser", sname: "aac",
        lcm: 50, hcm: 60, bwork: 190, tech: 130, cost: 1500,
        armor: 80, speed: 35, visib: 30, vrnge: 6, frnge: 1, glim: 8,
        nxlight: 1, nchoppers: 0, nplanes: 0, nland: 4,
        flags: ANTIMSL,
    },
    // 33: aegis cruiser (agc)
    ShipChr {
        name: "aegis cruiser", sname: "agc",
        lcm: 50, hcm: 60, bwork: 190, tech: 265, cost: 4000,
        armor: 80, speed: 35, visib: 30, vrnge: 6, frnge: 1, glim: 16,
        nxlight: 30, nchoppers: 2, nplanes: 32, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MSL.bits() | ANTIMSL.bits()),
    },
    // 34: nuclear cruiser (ncr)
    ShipChr {
        name: "nuc cruiser", sname: "ncr",
        lcm: 50, hcm: 50, bwork: 170, tech: 325, cost: 1800,
        armor: 100, speed: 45, visib: 35, vrnge: 6, frnge: 14, glim: 7,
        nxlight: 10, nchoppers: 2, nplanes: 20, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(MSL.bits() | ANTIMSL.bits()),
    },
    // 35: nuclear asw cruiser (nas)
    ShipChr {
        name: "nuc asw cruiser", sname: "nas",
        lcm: 50, hcm: 50, bwork: 170, tech: 330, cost: 1800,
        armor: 80, speed: 45, visib: 35, vrnge: 9, frnge: 10, glim: 4,
        nxlight: 25, nchoppers: 8, nplanes: 0, nland: 0,
        flags: ShipChrFlags::from_bits_truncate(TORP.bits() | DCH.bits() | SONAR.bits() | SUBT.bits()),
    },
    // 36: nuclear supply ship (nsp)
    ShipChr {
        name: "nuc supply ship", sname: "nsp",
        lcm: 60, hcm: 40, bwork: 160, tech: 360, cost: 1500,
        armor: 40, speed: 45, visib: 35, vrnge: 6, frnge: 0, glim: 0,
        nxlight: 10, nchoppers: 2, nplanes: 0, nland: 2,
        flags: SUPPLY,
    },
];

// Per-commodity cargo capacity table.
// Each row corresponds to ship type index (same order as MCHR).
// Columns: [civ, mil, she, gun, pet, iro, dus, bar, foo, oil, lcm, hcm, uw, rad]
// Source: ship.config (third config ship-chr block).
static MCHR_ITEMS: &[[i16; 14]] = &[
    //  civ  mil  she  gun  pet  iro  dus  bar  foo  oil  lcm  hcm   uw  rad
    [ 300,  10,   0,   0,   0,   0,   0,   0, 900,   0,   0,   0,  15,   0], //  0 fb
    [ 300,  10,   0,   0,   0,   0,   0,   0, 900,   0,   0,   0,  15,   0], //  1 ft
    [ 600,  50, 300,  50,   0,   0,   0,   0, 900,   0,1400, 900, 250,   0], //  2 cs
    [  30,   5,   0,   0,   0, 990, 990,   0, 200,   0,   0,   0,  45, 990], //  3 os
    [  20,  80,   0,   0,   0,   0,   0,   0, 200,   0,   0,   0,1200,   0], //  4 ss
    [  50,  50,   0,   0,   0,   0,   0,   0, 100,   0,   0,   0,   0,   0], //  5 ts (disabled)
    [   0,  60,  10,   2,   0,   0,   0,   0,  60,   0,   0,   0,   0,   0], //  6 frg
    [  10,   5,   0,   0,   0,   0,   0,   0, 100,   1,   0,   0,   0,   0], //  7 oe
    [ 990,  80,   0,   0,   0,   0,   0,   0, 990, 990,   0,   0, 990,   0], //  8 od
    [   0,   2,  12,   2,   0,   0,   0,   0,   5,   0,   0,   0,   0,   0], //  9 pt
    [   0, 100,  40,   5,   0,   0,   0,   0, 100,   0,   0,   0,   0,   0], // 10 lc
    [   0, 120, 100,   8,   0,   0,   0,   0, 200,   0,   0,   0,   0,   0], // 11 hc
    [   0, 120,  20,   4,   0,   0,   0,   0, 120,   0,   0,   0,   0,   0], // 12 tt
    [   0, 200, 200,  10,   0,   0,   0,   0, 900,   0,   0,   0,   0,   0], // 13 bb
    [   0, 180, 100,  10,   0,   0,   0,   0, 400,   0,   0,   0,   0,   0], // 14 bbc
    [  30,   5,   0,   0, 990,   0,   0,   0, 200, 990,   0,   0,  25,   0], // 15 tk
    [   0,  10, 100,   1,   0,   0,   0,   0,  90,   0,   0,   0,   0,   0], // 16 ms
    [   0,  60,  40,   4,   0,   0,   0,   0,  80,   0,   0,   0,   0,   0], // 17 dd
    [   0,  25,  36,   5,   0,   0,   0,   0,  80,   0,   0,   0,   0,   0], // 18 sb
    [   5,  10, 104,  20, 100,   0,   0,   0, 900,   0, 500, 300,   0,   0], // 19 sbc
    [   0, 175, 250,   4, 300,   0,   0,   0, 180,   0,   0,   0,   0,   0], // 20 cal
    [   0, 350, 500,   4, 500,   0,   0,   0, 900,   0,   0,   0,   0,   0], // 21 car
    [   0, 350, 999,   4, 999,   0,   0,   0, 900,   0,   0,   0,   0,   0], // 22 can
    [   0, 400,  10,   1,   0,   0,   0,   0, 300,   0,   0,   0,   0,   0], // 23 ls
    [   0,  60,  60,   4,   0,   0,   0,   0, 120,   0,   0,   0,   0,   0], // 24 af
    [   0,  25,  60,   6,   0,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 25 na
    [   0, 100,  80,   6,  40,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 26 ad
    [   0,  25, 200,   1,   0,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 27 nm
    [   0,  25, 100,   1,   0,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 28 msb
    [   0,   5, 100,   3,   0,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 29 mb
    [   0,  60, 220,   4,   0,   0,   0,   0, 120,   0,   0,   0,   0,   0], // 30 mf
    [   0, 120, 500,   6, 160,   0,   0,   0, 200,   0,   0,   0,   0,   0], // 31 mc
    [   0, 100, 100,  15,   0,   0,   0,   0, 200,   0,   0,   0,   0,   0], // 32 aac
    [   0, 200, 400,  25,  40,   0,   0,   0, 900,   0,   0,   0,   0,   0], // 33 agc
    [   0, 200, 400,   8,  40,   0,   0,   0, 900,   0,   0,   0,   0,   0], // 34 ncr
    [   0, 200, 120,   6, 160,   0,   0,   0, 500,   0,   0,   0,   0,   0], // 35 nas
    [  50,  50, 600,  50, 999,   0,   0,   0, 999,   0,1500, 900,   0,   0], // 36 nsp
];

impl ShipChr {
    /// Return the characteristic record for ship type index `idx`.
    ///
    /// Returns `None` if `idx` is out of range.
    pub fn for_type(idx: usize) -> Option<&'static ShipChr> {
        MCHR.get(idx)
    }

    /// Return a slice of all ship type descriptors in index order.
    pub fn all() -> &'static [ShipChr] {
        MCHR
    }

    /// Return the number of defined ship types.
    pub fn count() -> usize {
        MCHR.len()
    }

    /// Return per-commodity cargo capacity for this ship type.
    /// Returns 0 if the ship type has no capacity for that item.
    pub fn cargo_cap(&self, item: crate::commodity::Item) -> i16 {
        let idx = MCHR.iter().position(|c| std::ptr::eq(c, self)).unwrap_or(usize::MAX);
        MCHR_ITEMS.get(idx).map(|row| row[item as usize]).unwrap_or(0)
    }
}
