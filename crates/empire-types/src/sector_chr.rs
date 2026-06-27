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
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/global/sect.config, include/sect.h (struct dchrstr)
// Known contributors to the original:
//    Dave Pare, 1986
//    Jeff Bailey
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998
//    Markus Armbruster, 2006-2021

// Sector type descriptor table.  Rust equivalent of the C `dchr[]` array
// (struct dchrstr, loaded from sect.config at runtime in the C server; here
// compiled in as statics).
//
// Indexed by `SectorType as i8`, offset by 1 because Sea = -1.
// Use `SectorChr::for_type(t)` rather than indexing directly.

use crate::sector::SectorType;

/// Index into the ProductChr table (-1 = no product).
pub type ProdIndex = i8;

/// Packaging type for items moved through this sector.
/// Used as an index into ItemChr.packing[]: IPKG=0, NPKG=1, WPKG=2, UPKG=3, BPKG=4.
/// When sector efficiency is < 60%, IPKG (0) is used instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    Norm = 1,  // NPKG — standard land sectors
    Ware = 2,  // WPKG — harbors and warehouses (10× better for most items)
    Bank = 4,  // BPKG — banks
}

/// Per-sector-type descriptor.  ref: struct dchrstr in include/sect.h
#[derive(Debug, Clone, Copy)]
pub struct SectorChr {
    /// Product produced by this sector type (-1 = none).
    pub prd: ProdIndex,
    /// Production efficiency factor (%).  Sector must be >= 60% effic to produce.
    pub peff: i32,
    /// Build work units needed to raise efficiency 1%.
    pub bwork: i32,
    /// Build cash cost per efficiency point.
    pub cost: i32,
    /// Maintenance cost per ETU (dollars).
    pub maint: i32,
    /// Maximum population (at 0% efficiency; 10x at 100%).
    pub maxpop: i32,
    /// Movement cost at 0% efficiency.  -1.0 means impassable (water, sanct, wasteland).
    pub mob0: f32,
    /// Movement cost at 100% efficiency.
    pub mob1: f32,
    /// Packaging type applied when sector is >= 60% efficient.
    pub pkg: Packing,
    /// True for sector types that are naturally water (no mobility accumulation).
    pub is_water: bool,
    /// True for sanctuaries (players cannot take over, no loyalty change).
    pub is_sanct: bool,
    /// True for deity-only sector types.
    pub is_deity: bool,
    /// True for enlistment centers (converts civs → mil).
    pub is_enlist: bool,
    /// Human-readable name.
    pub name: &'static str,
}

// ── Product indices (match product_chr.rs ProductChr table order) ─────────────
// Negative means "no production".
pub const PRD_NONE:   i8 = -1;
pub const PRD_IRON:   i8 =  0;  // iron ore
pub const PRD_DUST:   i8 =  1;  // gold dust
pub const PRD_FOOD:   i8 =  2;  // food
pub const PRD_OIL:    i8 =  3;  // oil
pub const PRD_RAD:    i8 =  4;  // radioactive materials
pub const PRD_SHELL:  i8 =  5;  // shells
pub const PRD_GUN:    i8 =  6;  // guns
pub const PRD_PETROL: i8 =  7;  // petrol
pub const PRD_BAR:    i8 =  8;  // gold bars
pub const PRD_LCM:    i8 =  9;  // light construction materials
pub const PRD_HCM:    i8 = 10;  // heavy construction materials
pub const PRD_TECH:   i8 = 11;  // technological breakthroughs
pub const PRD_MED:    i8 = 12;  // medical discoveries (research)
pub const PRD_EDU:    i8 = 13;  // education (graduates)
pub const PRD_HAP:    i8 = 14;  // happiness (strollers)

// ── Static descriptor table ───────────────────────────────────────────────────
//
// Order matches Empire 4.4.1 sect.config uid column exactly (0-33).
// Indexed directly by SectorType as u8; no offset needed.
//
//  0 sea  1 mountain  2 sanctuary  3 wasteland  4 wilderness  5 capital
//  6 uranium  7 park  8 defense  9 shell  10 mine  11 gold
// 12 harbor  13 warehouse  14 airfield  15 agri  16 oil
// 17 lightmanuf  18 heavymanuf  19 fortress  20 tech  21 research
// 22 nuclear  23 library  24 highway  25 radar  26 headquarters
// 27 bridgehead  28 bridgespan  29 bank  30 refinery  31 enlist
// 32 plains  33 bridgetower

// Helper macro for standard land sectors (mob0=0.4, mob1=0.2, Norm packing).
macro_rules! land_schr {
    ($prd:expr, $peff:expr, $bwork:expr, $cost:expr, $maint:expr, $maxpop:expr,
     $enlist:expr, $deity:expr, $sanct:expr, $name:expr) => {
        SectorChr {
            prd: $prd, peff: $peff, bwork: $bwork, cost: $cost, maint: $maint,
            maxpop: $maxpop, mob0: 0.4, mob1: 0.2, pkg: Packing::Norm,
            is_water: false, is_sanct: $sanct, is_deity: $deity,
            is_enlist: $enlist, name: $name,
        }
    };
}

const DCHR: &[SectorChr] = &[
    //  0: "."  sea — impassable water
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 0, mob0: -1.0, mob1: -1.0, pkg: Packing::Norm,
                is_water: true, is_sanct: false, is_deity: true,
                is_enlist: false, name: "sea" },
    //  1: "^"  mountain — deity, slow, produces gold dust
    SectorChr { prd: PRD_DUST, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 100, mob0: 2.4, mob1: 1.2, pkg: Packing::Norm,
                is_water: false, is_sanct: false, is_deity: true,
                is_enlist: false, name: "mountain" },
    //  2: "s"  sanctuary — deity-only, impassable, player start zone
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 1000, mob0: -1.0, mob1: -1.0, pkg: Packing::Norm,
                is_water: false, is_sanct: true, is_deity: true,
                is_enlist: false, name: "sanctuary" },
    //  3: "\"  wasteland — deity-only, impassable (nuclear fallout)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 0, mob0: -1.0, mob1: -1.0, pkg: Packing::Norm,
                is_water: false, is_sanct: false, is_deity: true,
                is_enlist: false, name: "wasteland" },
    //  4: "-"  wilderness — passable but slow (mob matches at all efficiencies)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.4, pkg: Packing::Norm,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "wilderness" },
    //  5: "c"  capital / city (maint=1 per ETU)
    land_schr!(PRD_NONE,   0, 100, 100, 1, 1000, false, false, false, "capital"),
    //  6: "u"  uranium mine
    land_schr!(PRD_RAD,  100, 100, 100, 0, 1000, false, false, false, "uranium mine"),
    //  7: "p"  park (happiness)
    land_schr!(PRD_HAP,  100, 100, 100, 0, 1000, false, false, false, "park"),
    //  8: "d"  defense plant (guns)
    land_schr!(PRD_GUN,  100, 100, 100, 0, 1000, false, false, false, "defense plant"),
    //  9: "i"  shell industry
    land_schr!(PRD_SHELL,100, 100, 100, 0, 1000, false, false, false, "shell industry"),
    // 10: "m"  mine (iron ore)
    land_schr!(PRD_IRON, 100, 100, 100, 0, 1000, false, false, false, "mine"),
    // 11: "g"  gold mine (gold dust)
    land_schr!(PRD_DUST, 100, 100, 100, 0, 1000, false, false, false, "gold mine"),
    // 12: "h"  harbor (WPKG = 10× packing bonus)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.2, pkg: Packing::Ware,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "harbor" },
    // 13: "w"  warehouse (WPKG)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.2, pkg: Packing::Ware,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "warehouse" },
    // 14: "*"  airfield
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "airfield"),
    // 15: "a"  agribusiness (food, peff=900)
    land_schr!(PRD_FOOD, 900, 100, 100, 0, 1000, false, false, false, "agribusiness"),
    // 16: "o"  oil field
    land_schr!(PRD_OIL,  100, 100, 100, 0, 1000, false, false, false, "oil field"),
    // 17: "j"  light manufacturing (LCM)
    land_schr!(PRD_LCM,  100, 100, 100, 0, 1000, false, false, false, "light manufacturing"),
    // 18: "k"  heavy manufacturing (HCM)
    land_schr!(PRD_HCM,  100, 100, 100, 0, 1000, false, false, false, "heavy manufacturing"),
    // 19: "f"  fortress (cost=500)
    land_schr!(PRD_NONE,   0, 100, 500, 0, 1000, false, false, false, "fortress"),
    // 20: "t"  technical center
    land_schr!(PRD_TECH, 100, 100, 100, 0, 1000, false, false, false, "technical center"),
    // 21: "r"  research lab
    land_schr!(PRD_MED,  100, 100, 100, 0, 1000, false, false, false, "research lab"),
    // 22: "n"  nuclear plant (no automatic production; used to build nukes)
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "nuclear plant"),
    // 23: "l"  library / school (education)
    land_schr!(PRD_EDU,  100, 100, 100, 0, 1000, false, false, false, "library/school"),
    // 24: "+"  highway (mob1=0.0: fully built = free movement)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.0, pkg: Packing::Norm,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "highway" },
    // 25: ")"  radar installation
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "radar installation"),
    // 26: "!"  headquarters
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "headquarters"),
    // 27: "#"  bridge head (land end of a bridge)
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "bridge head"),
    // 28: "="  bridge span (over water)
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "bridge span"),
    // 29: "b"  bank (BPKG packaging)
    SectorChr { prd: PRD_BAR, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.2, pkg: Packing::Bank,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "bank" },
    // 30: "%"  refinery (petrol)
    land_schr!(PRD_PETROL,100, 100, 100, 0, 1000, false, false, false, "refinery"),
    // 31: "e"  enlistment center (converts civs → mil)
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, true,  false, false, "enlistment center"),
    // 32: "~"  plains (passable, mob matches all efficiencies like wilderness)
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, mob0: 0.4, mob1: 0.4, pkg: Packing::Norm,
                is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "plains" },
    // 33: "@"  bridge tower (submarine foundation; dstr/ostr=0)
    land_schr!(PRD_NONE,   0, 100, 100, 0, 1000, false, false, false, "bridge tower"),
];

impl SectorChr {
    /// Look up the descriptor for a sector type.
    /// SectorType uids match DCHR indices directly (Sea=0 → DCHR[0]).
    pub fn for_type(t: SectorType) -> &'static SectorChr {
        let idx = t as u8 as usize;
        if idx < DCHR.len() { &DCHR[idx] } else { &DCHR[0] }
    }

    /// True if this sector type never produces anything.
    pub fn is_productive(&self) -> bool {
        self.prd >= 0 && self.peff > 0
    }

    /// Movement cost per mobility unit for a sector at the given efficiency.
    /// Returns -1.0 if the sector type is impassable (sea, wasteland, unknown).
    /// Interpolates linearly between mob0 (0% eff) and mob1 (100% eff).
    pub fn mcost(&self, effic: i8) -> f64 {
        if self.mob0 < 0.0 {
            return -1.0;
        }
        let eff = effic.max(0) as f64 / 100.0;
        let cost = self.mob0 as f64 + (self.mob1 as f64 - self.mob0 as f64) * eff;
        cost.max(0.001)
    }

    /// Return the packing multiplier for this sector given its efficiency.
    /// Below 60% always returns 1 (IPKG).
    pub fn pack_mult(&self, item_packing: &[i32; 5], effic: i8) -> i32 {
        if effic < 60 {
            item_packing[0]  // IPKG
        } else {
            item_packing[self.pkg as usize]
        }
    }
}
