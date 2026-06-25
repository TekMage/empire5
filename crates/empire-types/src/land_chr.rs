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
// Ported from: src/lib/global/land.config, include/land.h (struct lchrstr)
// Known contributors to the original:
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000
//    Markus Armbruster, 2006-2016

// Land unit characteristic table.
// Rust equivalent of the C `lchr[]` array (struct lchrstr), compiled in from
// the values in land.config rather than loaded at runtime.
//
// Use `LandChr::for_type(idx)` or `LandChr::all()` to access entries.

bitflags::bitflags! {
    /// Land unit capability flags.  Correspond to L_* constants in include/land.h.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LandChrFlags: u32 {
        /// Engineer: can build/repair bridges and sectors (L_ENGINEER).
        const ENGINEER  = 1 << 1;
        /// Supply: can supply other units and sectors (L_SUPPLY).
        const SUPPLY    = 1 << 2;
        /// Security: anti-CHE (guerrilla) troops (L_SECURITY).
        const SECURITY  = 1 << 3;
        /// Light: can be loaded onto ships (L_LIGHT).
        const LIGHT     = 1 << 4;
        /// Marine: marine unit, good at ship assaults (L_MARINE).
        const MARINE    = 1 << 5;
        /// Recon: good at spying / reconnaissance (L_RECON).
        const RECON     = 1 << 6;
        /// Radar: radar unit (L_RADAR).
        const RADAR     = 1 << 7;
        /// Assault: can perform assault landings (L_ASSAULT).
        const ASSAULT   = 1 << 8;
        /// Flak: anti-aircraft fire unit (L_FLAK).
        const FLAK      = 1 << 9;
        /// Spy: spy unit (L_SPY).
        const SPY       = 1 << 10;
        /// Train: train unit, follows rails (L_TRAIN).
        const TRAIN     = 1 << 11;
        /// Heavy: heavy unit, cannot be loaded on trains (L_HEAVY).
        const HEAVY     = 1 << 12;
    }
}

/// Per-land-unit-type descriptor.  ref: struct lchrstr in include/land.h.
///
/// Values are compiled in from `src/lib/global/land.config`.
#[derive(Debug, Clone, Copy)]
pub struct LandChr {
    /// Full name (e.g. "cavalry").
    pub name: &'static str,
    /// Short type abbreviation (e.g. "cav").
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
    /// Attack multiplier.
    pub att: f32,
    /// Defense multiplier.
    pub def: f32,
    /// Vulnerability (0–100; higher = more vulnerable).
    pub vul: i32,
    /// Speed (sectors per mobility point).
    pub spd: i32,
    /// Visibility.
    pub vis: i32,
    /// Spy radius (how far unit can see during intelligence gathering).
    pub spy: i32,
    /// Reaction radius.
    pub rad: i32,
    /// Firing range (sectors).
    pub frg: i32,
    /// Firing accuracy (percentage).
    pub acc: i32,
    /// Damage per shot.
    pub dam: i32,
    /// Ammunition used per shot.
    pub ammo: i32,
    /// Anti-aircraft fire rating.
    pub aaf: i32,
    /// Maximum extra-light planes carried.
    pub nxlight: u8,
    /// Maximum land units carried.
    pub nland: u8,
    /// Capability flags.
    pub flags: LandChrFlags,
}

// Flag shorthand for the table below.
const ENG:      LandChrFlags = LandChrFlags::ENGINEER;
const SUPPLY:   LandChrFlags = LandChrFlags::SUPPLY;
const SEC:      LandChrFlags = LandChrFlags::SECURITY;
const LIGHT:    LandChrFlags = LandChrFlags::LIGHT;
const MARINE:   LandChrFlags = LandChrFlags::MARINE;
const RECON:    LandChrFlags = LandChrFlags::RECON;
const RADAR:    LandChrFlags = LandChrFlags::RADAR;
const ASSAULT:  LandChrFlags = LandChrFlags::ASSAULT;
const FLAK:     LandChrFlags = LandChrFlags::FLAK;
const SPY:      LandChrFlags = LandChrFlags::SPY;
const TRAIN:    LandChrFlags = LandChrFlags::TRAIN;
const HEAVY:    LandChrFlags = LandChrFlags::HEAVY;

/// Static land unit characteristic table.  Indices match type numbers in
/// land.config.
static LCHR: &[LandChr] = &[
    // 0: cavalry (cav)
    LandChr {
        name: "cavalry", sname: "cav",
        lcm: 10, hcm: 5, bwork: 40, tech: 30, cost: 500,
        att: 1.2, def: 0.5, vul: 80, spd: 32, vis: 18, spy: 4, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | RECON.bits()),
    },
    // 1: light infantry (linf)
    LandChr {
        name: "light infantry", sname: "linf",
        lcm: 8, hcm: 4, bwork: 36, tech: 40, cost: 300,
        att: 1.0, def: 1.5, vul: 60, spd: 28, vis: 15, spy: 2, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | ASSAULT.bits()),
    },
    // 2: infantry (inf)
    LandChr {
        name: "infantry", sname: "inf",
        lcm: 10, hcm: 5, bwork: 40, tech: 50, cost: 500,
        att: 1.0, def: 1.5, vul: 60, spd: 25, vis: 15, spy: 2, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | ASSAULT.bits()),
    },
    // 3: motor infantry (mtif)
    LandChr {
        name: "motor inf", sname: "mtif",
        lcm: 15, hcm: 10, bwork: 55, tech: 190, cost: 400,
        att: 1.2, def: 2.2, vul: 60, spd: 33, vis: 17, spy: 1, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 2, aaf: 3,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 4: mechanized infantry (mif)
    LandChr {
        name: "mech inf", sname: "mif",
        lcm: 15, hcm: 10, bwork: 55, tech: 190, cost: 800,
        att: 1.5, def: 2.5, vul: 50, spd: 33, vis: 17, spy: 1, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 2, aaf: 3,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 5: marines (mar)
    LandChr {
        name: "marines", sname: "mar",
        lcm: 10, hcm: 5, bwork: 40, tech: 140, cost: 1000,
        att: 1.4, def: 2.4, vul: 60, spd: 25, vis: 14, spy: 2, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 2,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | MARINE.bits() | ASSAULT.bits()),
    },
    // 6: supply unit (sup)
    LandChr {
        name: "supply", sname: "sup",
        lcm: 10, hcm: 5, bwork: 40, tech: 50, cost: 500,
        att: 0.1, def: 0.2, vul: 80, spd: 25, vis: 20, spy: 1, rad: 0,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(SUPPLY.bits() | LIGHT.bits()),
    },
    // 7: train (tra)
    LandChr {
        name: "train", sname: "tra",
        lcm: 100, hcm: 50, bwork: 220, tech: 40, cost: 3500,
        att: 0.0, def: 0.0, vul: 120, spd: 10, vis: 25, spy: 3, rad: 0,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 5, nland: 12,
        flags: LandChrFlags::from_bits_truncate(SUPPLY.bits() | TRAIN.bits() | HEAVY.bits()),
    },
    // 8: infiltrator / spy (spy)
    LandChr {
        name: "infiltrator", sname: "spy",
        lcm: 10, hcm: 5, bwork: 40, tech: 40, cost: 750,
        att: 0.0, def: 0.0, vul: 80, spd: 32, vis: 18, spy: 4, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(RECON.bits() | LIGHT.bits() | ASSAULT.bits() | SPY.bits()),
    },
    // 9: commando (com)
    LandChr {
        name: "commando", sname: "com",
        lcm: 10, hcm: 5, bwork: 40, tech: 55, cost: 1500,
        att: 0.0, def: 0.0, vul: 80, spd: 32, vis: 18, spy: 4, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 0,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | RECON.bits() | ASSAULT.bits() | SPY.bits()),
    },
    // 10: anti-air unit (aau)
    LandChr {
        name: "aa unit", sname: "aau",
        lcm: 20, hcm: 10, bwork: 60, tech: 70, cost: 500,
        att: 0.5, def: 1.0, vul: 60, spd: 18, vis: 20, spy: 1, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 2,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | FLAK.bits()),
    },
    // 11: artillery (art)
    LandChr {
        name: "artillery", sname: "art",
        lcm: 20, hcm: 10, bwork: 60, tech: 35, cost: 800,
        att: 0.1, def: 0.4, vul: 70, spd: 18, vis: 20, spy: 1, rad: 0,
        frg: 8, acc: 50, dam: 5, ammo: 2, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 12: light artillery (lat)
    LandChr {
        name: "lt artillery", sname: "lat",
        lcm: 20, hcm: 10, bwork: 60, tech: 70, cost: 500,
        att: 0.2, def: 0.6, vul: 60, spd: 30, vis: 18, spy: 1, rad: 1,
        frg: 5, acc: 10, dam: 3, ammo: 1, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 13: heavy artillery (hat)
    LandChr {
        name: "hvy artillery", sname: "hat",
        lcm: 40, hcm: 20, bwork: 100, tech: 100, cost: 800,
        att: 0.0, def: 0.2, vul: 60, spd: 12, vis: 20, spy: 1, rad: 0,
        frg: 11, acc: 99, dam: 8, ammo: 4, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::empty(),
    },
    // 14: mechanized artillery (mat)
    LandChr {
        name: "mech artillery", sname: "mat",
        lcm: 20, hcm: 10, bwork: 60, tech: 200, cost: 1000,
        att: 0.2, def: 0.6, vul: 50, spd: 35, vis: 17, spy: 1, rad: 1,
        frg: 8, acc: 35, dam: 6, ammo: 3, aaf: 3,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 15: engineer (eng)
    LandChr {
        name: "engineer", sname: "eng",
        lcm: 10, hcm: 5, bwork: 40, tech: 130, cost: 3000,
        att: 1.2, def: 2.4, vul: 50, spd: 25, vis: 14, spy: 2, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(ENG.bits() | LIGHT.bits() | ASSAULT.bits()),
    },
    // 16: mechanized engineer (meng)
    LandChr {
        name: "mech engineer", sname: "meng",
        lcm: 10, hcm: 5, bwork: 40, tech: 260, cost: 4500,
        att: 1.8, def: 3.5, vul: 45, spd: 33, vis: 15, spy: 3, rad: 3,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 5,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(ENG.bits() | LIGHT.bits() | ASSAULT.bits()),
    },
    // 17: light armor (lar)
    LandChr {
        name: "lt armor", sname: "lar",
        lcm: 10, hcm: 5, bwork: 40, tech: 150, cost: 600,
        att: 2.0, def: 1.0, vul: 50, spd: 42, vis: 15, spy: 4, rad: 4,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 2,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | RECON.bits()),
    },
    // 18: heavy armor (har)
    LandChr {
        name: "hvy armor", sname: "har",
        lcm: 20, hcm: 10, bwork: 60, tech: 120, cost: 500,
        att: 2.0, def: 0.8, vul: 50, spd: 18, vis: 17, spy: 1, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 2, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::empty(),
    },
    // 19: armor (arm)
    LandChr {
        name: "armor", sname: "arm",
        lcm: 20, hcm: 10, bwork: 60, tech: 170, cost: 1000,
        att: 3.0, def: 1.5, vul: 40, spd: 33, vis: 16, spy: 2, rad: 2,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 2,
        nxlight: 0, nland: 0,
        flags: LIGHT,
    },
    // 20: security (sec)
    LandChr {
        name: "security", sname: "sec",
        lcm: 10, hcm: 5, bwork: 40, tech: 170, cost: 600,
        att: 1.0, def: 2.0, vul: 60, spd: 25, vis: 15, spy: 2, rad: 1,
        frg: 0, acc: 0, dam: 0, ammo: 1, aaf: 1,
        nxlight: 0, nland: 0,
        flags: LandChrFlags::from_bits_truncate(SEC.bits() | LIGHT.bits()),
    },
    // 21: radar unit (rad)
    LandChr {
        name: "radar unit", sname: "rad",
        lcm: 10, hcm: 5, bwork: 40, tech: 270, cost: 1000,
        att: 0.0, def: 0.0, vul: 50, spd: 33, vis: 15, spy: 3, rad: 0,
        frg: 0, acc: 0, dam: 0, ammo: 0, aaf: 2,
        nxlight: 1, nland: 0,
        flags: LandChrFlags::from_bits_truncate(LIGHT.bits() | RADAR.bits()),
    },
];

impl LandChr {
    /// Return the characteristic record for land unit type index `idx`.
    ///
    /// Returns `None` if `idx` is out of range.
    pub fn for_type(idx: usize) -> Option<&'static LandChr> {
        LCHR.get(idx)
    }

    /// Return a slice of all land unit type descriptors in index order.
    pub fn all() -> &'static [LandChr] {
        LCHR
    }

    /// Return the number of defined land unit types.
    pub fn count() -> usize {
        LCHR.len()
    }
}
