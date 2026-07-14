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
// Ported from: src/lib/global/plane.config, include/plane.h (struct plchrstr)
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998
//    Markus Armbruster, 2006-2021

// Plane characteristic table.
// Rust equivalent of the C `plchr[]` array (struct plchrstr), compiled in from
// the values in plane.config rather than loaded at runtime.
//
// Use `PlaneChr::for_type(idx)` or `PlaneChr::all()` to access entries.

bitflags::bitflags! {
    /// Plane capability flags.  Correspond to P_* constants in include/plane.h.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlaneChrFlags: u32 {
        /// Bomber: strategic bombing ability (P_B).
        const BOMBER     = 1 << 0;
        /// Tactical: tactical bombing ability (P_T).
        const TACTICAL   = 1 << 1;
        /// Fighter/escort: interceptor (P_F).
        const FIGHTER    = 1 << 2;
        /// Cargo: can transport cargo (P_C).
        const CARGO      = 1 << 3;
        /// VTOL: vertical take-off and landing (P_V).
        const VTOL       = 1 << 4;
        /// Missile: used once, cannot be intercepted (P_M).
        const MISSILE    = 1 << 5;
        /// Light: can land on carriers (P_L).
        const LIGHT      = 1 << 6;
        /// Spy: spy ability (P_S).
        const SPY        = 1 << 7;
        /// Image: advanced imaging/spying ability (P_I).
        const IMAGE      = 1 << 8;
        /// Satellite: orbital ability (P_O).
        const SATELLITE  = 1 << 9;
        /// ABM: nuclear RV interceptor (P_N).
        const ABM        = 1 << 11;
        /// Extra-light: can be carried on ships as xlight (P_E).
        const XLIGHT     = 1 << 13;
        /// Helicopter: chopper (P_K).
        const HELO       = 1 << 14;
        /// ASW: anti-submarine warfare (P_A).
        const ASW        = 1 << 15;
        /// Para: can drop paratroopers (P_P).
        const PARA       = 1 << 16;
        /// Escort: escort mission capable (P_ESC).
        const ESCORT     = 1 << 17;
        /// Mine: can lay mines (P_MINE).
        const MINE       = 1 << 18;
        /// Sweep: can sweep mines (P_SWEEP).
        const SWEEP      = 1 << 19;
        /// Marine missile: missile that can hit ships (P_MAR).
        const MARINE     = 1 << 20;
    }
}

/// Per-plane-type descriptor.  ref: struct plchrstr in include/plane.h.
///
/// Values are compiled in from `src/lib/global/plane.config`.
#[derive(Debug, Clone, Copy)]
pub struct PlaneChr {
    /// Full name (e.g. "Sopwith Camel").
    pub name: &'static str,
    /// Short type abbreviation (e.g. "f1").
    pub sname: &'static str,
    /// LCM required to build to 100% efficiency.
    pub lcm: i32,
    /// HCM required to build to 100% efficiency.
    pub hcm: i32,
    /// Military (mil) required to build to 100% efficiency.
    pub mil: i32,
    /// Work units required to build to 100% efficiency.
    pub bwork: i32,
    /// Minimum tech level required to build.
    pub tech: i32,
    /// Build cost (dollars).
    pub cost: i32,
    /// Bombing accuracy (higher = better).
    pub acc: i32,
    /// Bomb load (also cargo capacity).
    pub load: i32,
    /// Air-to-air attack strength.
    pub att: i32,
    /// Air-to-air defense strength.
    pub def: i32,
    /// Maximum range in sectors.
    pub range: i32,
    /// Fuel consumption per sector.
    pub fuel: i32,
    /// Stealth rating (0 = visible, 100 = invisible).
    pub stealth: i32,
    /// Capability flags.
    pub flags: PlaneChrFlags,
}

// Flag shorthands for the table below.
const B:    PlaneChrFlags = PlaneChrFlags::BOMBER;
const T:    PlaneChrFlags = PlaneChrFlags::TACTICAL;
const F:    PlaneChrFlags = PlaneChrFlags::FIGHTER;
const C:    PlaneChrFlags = PlaneChrFlags::CARGO;
const V:    PlaneChrFlags = PlaneChrFlags::VTOL;
const M:    PlaneChrFlags = PlaneChrFlags::MISSILE;
const L:    PlaneChrFlags = PlaneChrFlags::LIGHT;
const S:    PlaneChrFlags = PlaneChrFlags::SPY;
const I:    PlaneChrFlags = PlaneChrFlags::IMAGE;
const O:    PlaneChrFlags = PlaneChrFlags::SATELLITE;
const N:    PlaneChrFlags = PlaneChrFlags::ABM;
const E:    PlaneChrFlags = PlaneChrFlags::XLIGHT;
const K:    PlaneChrFlags = PlaneChrFlags::HELO;
const A:    PlaneChrFlags = PlaneChrFlags::ASW;
const P:    PlaneChrFlags = PlaneChrFlags::PARA;
const ESC:  PlaneChrFlags = PlaneChrFlags::ESCORT;
const MINE: PlaneChrFlags = PlaneChrFlags::MINE;
const SWEP: PlaneChrFlags = PlaneChrFlags::SWEEP;
const MAR:  PlaneChrFlags = PlaneChrFlags::MARINE;

/// Static plane characteristic table.  Indices match type numbers in
/// plane.config.
static PLCHR: &[PlaneChr] = &[
    // 0: f1 — Sopwith Camel
    PlaneChr {
        name: "Sopwith Camel", sname: "f1",
        lcm: 8, hcm: 2, mil: 1, bwork: 32, tech: 50, cost: 400,
        acc: 90, load: 1, att: 1, def: 1, range: 4, fuel: 1, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | V.bits()),
    },
    // 1: f2 — P-51 Mustang
    PlaneChr {
        name: "P-51 Mustang", sname: "f2",
        lcm: 8, hcm: 2, mil: 1, bwork: 32, tech: 80, cost: 400,
        acc: 80, load: 1, att: 4, def: 4, range: 8, fuel: 1, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | L.bits()),
    },
    // 2: jf1 — F-4 Phantom
    PlaneChr {
        name: "F-4 Phantom", sname: "jf1",
        lcm: 12, hcm: 4, mil: 2, bwork: 40, tech: 125, cost: 1000,
        acc: 45, load: 1, att: 14, def: 14, range: 11, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | L.bits()),
    },
    // 3: jf2 — AV-8B Harrier
    PlaneChr {
        name: "AV-8B Harrier", sname: "jf2",
        lcm: 12, hcm: 4, mil: 2, bwork: 40, tech: 195, cost: 1400,
        acc: 30, load: 1, att: 17, def: 17, range: 14, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | V.bits() | L.bits()),
    },
    // 4: sf — F-117A Nighthawk
    PlaneChr {
        name: "F-117A Nighthawk", sname: "sf",
        lcm: 15, hcm: 5, mil: 2, bwork: 45, tech: 325, cost: 3000,
        acc: 45, load: 3, att: 19, def: 19, range: 20, fuel: 4, stealth: 80,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | L.bits()),
    },
    // 5: es — P-38 Lightning
    PlaneChr {
        name: "P-38 Lightning", sname: "es",
        lcm: 9, hcm: 3, mil: 1, bwork: 35, tech: 90, cost: 700,
        acc: 60, load: 1, att: 5, def: 5, range: 15, fuel: 2, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | ESC.bits()),
    },
    // 6: jes — F-14E jet escort
    PlaneChr {
        name: "F-14E jet escort", sname: "jes",
        lcm: 14, hcm: 8, mil: 2, bwork: 50, tech: 160, cost: 1400,
        acc: 60, load: 1, att: 10, def: 10, range: 25, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | ESC.bits()),
    },
    // 7: lb — TBD-1 Devastator
    PlaneChr {
        name: "TBD-1 Devastator", sname: "lb",
        lcm: 10, hcm: 3, mil: 1, bwork: 36, tech: 60, cost: 550,
        acc: 50, load: 2, att: 0, def: 3, range: 7, fuel: 1, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits() | V.bits() | L.bits()),
    },
    // 8: jl — A-6 Intruder
    PlaneChr {
        name: "A-6 Intruder", sname: "jl",
        lcm: 14, hcm: 4, mil: 2, bwork: 42, tech: 130, cost: 1000,
        acc: 25, load: 3, att: 0, def: 9, range: 11, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits() | L.bits()),
    },
    // 9: mb — medium bomber
    PlaneChr {
        name: "medium bomber", sname: "mb",
        lcm: 14, hcm: 5, mil: 3, bwork: 44, tech: 80, cost: 1000,
        acc: 45, load: 4, att: 0, def: 5, range: 14, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits()),
    },
    // 10: jfb — FB-111 Aardvark
    PlaneChr {
        name: "FB-111 Aardvark f/b", sname: "jfb",
        lcm: 20, hcm: 10, mil: 5, bwork: 60, tech: 140, cost: 1800,
        acc: 30, load: 7, att: 8, def: 8, range: 20, fuel: 5, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits()),
    },
    // 11: hb — B-26B Marauder
    PlaneChr {
        name: "B-26B Marauder", sname: "hb",
        lcm: 20, hcm: 6, mil: 2, bwork: 52, tech: 90, cost: 1100,
        acc: 90, load: 5, att: 0, def: 4, range: 15, fuel: 2, stealth: 0,
        flags: B,
    },
    // 12: jhb — B-52 Strato-Fortress
    PlaneChr {
        name: "B-52 Strato-Fortress", sname: "jhb",
        lcm: 26, hcm: 13, mil: 5, bwork: 72, tech: 150, cost: 3200,
        acc: 80, load: 12, att: 0, def: 11, range: 35, fuel: 6, stealth: 0,
        flags: B,
    },
    // 13: sb — B-2 stealth bomber
    PlaneChr {
        name: "B-2 stealth bomber", sname: "sb",
        lcm: 15, hcm: 5, mil: 2, bwork: 45, tech: 325, cost: 4000,
        acc: 25, load: 8, att: 0, def: 15, range: 28, fuel: 5, stealth: 80,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits()),
    },
    // 14: as — anti-sub plane
    PlaneChr {
        name: "anti-sub plane", sname: "as",
        lcm: 10, hcm: 3, mil: 2, bwork: 36, tech: 100, cost: 550,
        acc: 85, load: 2, att: 0, def: 3, range: 15, fuel: 2, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | A.bits() | MINE.bits() | SWEP.bits()),
    },
    // 15: np — naval plane
    PlaneChr {
        name: "naval plane", sname: "np",
        lcm: 20, hcm: 10, mil: 4, bwork: 60, tech: 135, cost: 1800,
        acc: 70, load: 3, att: 0, def: 4, range: 28, fuel: 2, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | C.bits() | L.bits() | A.bits() | MINE.bits() | SWEP.bits()),
    },
    // 16: nc — AH-1 Cobra
    PlaneChr {
        name: "AH-1 Cobra", sname: "nc",
        lcm: 8, hcm: 2, mil: 2, bwork: 32, tech: 160, cost: 800,
        acc: 55, load: 2, att: 0, def: 3, range: 11, fuel: 2, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | K.bits() | A.bits() | SWEP.bits()),
    },
    // 17: ac — AH-64 Apache
    PlaneChr {
        name: "AH-64 Apache", sname: "ac",
        lcm: 8, hcm: 2, mil: 2, bwork: 32, tech: 200, cost: 800,
        acc: 15, load: 1, att: 0, def: 9, range: 11, fuel: 2, stealth: 40,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | K.bits()),
    },
    // 18: tc — transport chopper
    PlaneChr {
        name: "transport chopper", sname: "tc",
        lcm: 8, hcm: 2, mil: 2, bwork: 32, tech: 135, cost: 800,
        acc: 0, load: 5, att: 0, def: 3, range: 7, fuel: 2, stealth: 40,
        flags: PlaneChrFlags::from_bits_truncate(C.bits() | V.bits() | K.bits() | P.bits()),
    },
    // 19: tr — C-56 Lodestar
    PlaneChr {
        name: "C-56 Lodestar", sname: "tr",
        lcm: 14, hcm: 5, mil: 3, bwork: 44, tech: 85, cost: 1000,
        acc: 0, load: 7, att: 0, def: 2, range: 15, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(C.bits() | P.bits()),
    },
    // 20: jt — C-141 Starlifter
    PlaneChr {
        name: "C-141 Starlifter", sname: "jt",
        lcm: 18, hcm: 5, mil: 3, bwork: 48, tech: 160, cost: 1500,
        acc: 0, load: 16, att: 0, def: 9, range: 35, fuel: 4, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(C.bits() | P.bits()),
    },
    // 21: zep — Zeppelin
    PlaneChr {
        name: "Zeppelin", sname: "zep",
        lcm: 6, hcm: 2, mil: 3, bwork: 30, tech: 70, cost: 1000,
        acc: 60, load: 2, att: 0, def: -3, range: 15, fuel: 2, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | C.bits() | V.bits() | S.bits()),
    },
    // 22: re — recon
    PlaneChr {
        name: "recon", sname: "re",
        lcm: 12, hcm: 4, mil: 2, bwork: 40, tech: 130, cost: 800,
        acc: 0, load: 0, att: 0, def: 4, range: 15, fuel: 2, stealth: 20,
        flags: S,
    },
    // 23: sp — E2-C Hawkeye
    PlaneChr {
        name: "E2-C Hawkeye", sname: "sp",
        lcm: 15, hcm: 5, mil: 2, bwork: 45, tech: 190, cost: 2000,
        acc: 0, load: 0, att: 0, def: 11, range: 32, fuel: 5, stealth: 50,
        flags: S,
    },
    // 24: lst — landsat
    PlaneChr {
        name: "landsat", sname: "lst",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 245, cost: 2000,
        acc: 0, load: 0, att: 0, def: 3, range: 41, fuel: 9, stealth: 0,
        flags: O,
    },
    // 25: ss — KH-7 spysat
    PlaneChr {
        name: "KH-7 spysat", sname: "ss",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 305, cost: 4000,
        acc: 0, load: 0, att: 0, def: 3, range: 61, fuel: 9, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(S.bits() | I.bits() | O.bits()),
    },
    // 26: mi — Harpoon
    PlaneChr {
        name: "Harpoon", sname: "mi",
        lcm: 8, hcm: 2, mil: 0, bwork: 32, tech: 160, cost: 300,
        acc: 50, load: 6, att: 0, def: 5, range: 6, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits() | L.bits() | MAR.bits()),
    },
    // 27: sam — Sea Sparrow
    PlaneChr {
        name: "Sea Sparrow", sname: "sam",
        lcm: 3, hcm: 1, mil: 0, bwork: 25, tech: 180, cost: 200,
        acc: 0, load: 0, att: 0, def: 18, range: 2, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(F.bits() | V.bits() | M.bits() | L.bits() | E.bits()),
    },
    // 28: ssm — V2
    PlaneChr {
        name: "V2", sname: "ssm",
        lcm: 15, hcm: 15, mil: 0, bwork: 65, tech: 145, cost: 800,
        acc: 60, load: 3, att: 0, def: 3, range: 4, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits()),
    },
    // 29: srbm — Atlas
    PlaneChr {
        name: "Atlas", sname: "srbm",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 200, cost: 1000,
        acc: 60, load: 6, att: 0, def: 5, range: 9, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits()),
    },
    // 30: irbm — Titan
    PlaneChr {
        name: "Titan", sname: "irbm",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 260, cost: 1500,
        acc: 60, load: 8, att: 0, def: 10, range: 15, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits()),
    },
    // 31: icbm — Minuteman
    PlaneChr {
        name: "Minuteman", sname: "icbm",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 310, cost: 3000,
        acc: 60, load: 10, att: 0, def: 15, range: 41, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits()),
    },
    // 32: slbm — Trident
    PlaneChr {
        name: "Trident", sname: "slbm",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 280, cost: 2000,
        acc: 60, load: 8, att: 0, def: 6, range: 23, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | V.bits() | M.bits() | L.bits()),
    },
    // 33: asat — anti-sat
    PlaneChr {
        name: "anti-sat", sname: "asat",
        lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 305, cost: 2000,
        acc: 50, load: 0, att: 0, def: 7, range: 13, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(V.bits() | M.bits() | O.bits()),
    },
    // 34: abm — Patriot
    PlaneChr {
        name: "Patriot", sname: "abm",
        lcm: 16, hcm: 8, mil: 0, bwork: 52, tech: 270, cost: 1500,
        acc: 50, load: 0, att: 0, def: 31, range: 12, fuel: 0, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(V.bits() | M.bits() | N.bits()),
    },
    // --- Modern tier (tech 335-460): current fighters/bombers above top
    // out around tech 325 (F-117A Nighthawk, B-2 stealth bomber). These
    // sit above that ceiling, following the same tech-vs-accuracy trade
    // established by every existing lineage (fighters: Sopwith Camel
    // acc 90 -> F-117A acc 45; bombers: B-26 acc 90 -> B-2 acc 25) --
    // higher tech buys stealth/att/def/range at the cost of accuracy.
    //
    // 35: fa18 — F/A-18 Super Hornet
    PlaneChr {
        name: "F/A-18 Super Hornet", sname: "fa18",
        lcm: 14, hcm: 5, mil: 2, bwork: 46, tech: 335, cost: 1800,
        acc: 40, load: 4, att: 20, def: 20, range: 16, fuel: 3, stealth: 0,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | L.bits()),
    },
    // 36: b52h — B-52H Stratofortress
    PlaneChr {
        name: "B-52H Stratofortress", sname: "b52h",
        lcm: 28, hcm: 14, mil: 5, bwork: 75, tech: 350, cost: 3600,
        acc: 85, load: 13, att: 0, def: 13, range: 40, fuel: 6, stealth: 0,
        flags: B,
    },
    // 37: f35 — F-35 Lightning II
    PlaneChr {
        name: "F-35 Lightning II", sname: "f35",
        lcm: 18, hcm: 7, mil: 3, bwork: 55, tech: 380, cost: 4500,
        acc: 35, load: 5, att: 24, def: 24, range: 18, fuel: 4, stealth: 70,
        flags: PlaneChrFlags::from_bits_truncate(T.bits() | F.bits() | V.bits() | L.bits()),
    },
    // 38: f22 — F-22 Raptor
    PlaneChr {
        name: "F-22 Raptor", sname: "f22",
        lcm: 16, hcm: 6, mil: 2, bwork: 50, tech: 405, cost: 5500,
        acc: 30, load: 2, att: 30, def: 30, range: 16, fuel: 4, stealth: 85,
        flags: F,
    },
    // 39: b21 — B-21 Raider
    PlaneChr {
        name: "B-21 Raider", sname: "b21",
        lcm: 20, hcm: 8, mil: 3, bwork: 60, tech: 425, cost: 6000,
        acc: 15, load: 10, att: 0, def: 22, range: 32, fuel: 5, stealth: 95,
        flags: PlaneChrFlags::from_bits_truncate(B.bits() | T.bits()),
    },
    // 40: f47 — F-47
    PlaneChr {
        name: "F-47", sname: "f47",
        lcm: 20, hcm: 8, mil: 3, bwork: 58, tech: 460, cost: 7000,
        acc: 20, load: 3, att: 35, def: 35, range: 22, fuel: 5, stealth: 92,
        flags: F,
    },
];

impl PlaneChr {
    /// Return the characteristic record for plane type index `idx`.
    ///
    /// Returns `None` if `idx` is out of range.
    pub fn for_type(idx: usize) -> Option<&'static PlaneChr> {
        PLCHR.get(idx)
    }

    /// Return a slice of all plane type descriptors in index order.
    pub fn all() -> &'static [PlaneChr] {
        PLCHR
    }

    /// Return the number of defined plane types.
    pub fn count() -> usize {
        PLCHR.len()
    }
}
