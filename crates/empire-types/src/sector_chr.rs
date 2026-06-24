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
// Order: Sea(-1), Land(0), Mountain(1), Agri(2), Uranium(3), Plain(4),
//        Park(5), Urban(6), Research(7), Wasteland(8), Defense(9),
//        Bank(10), Engineer(11), Airfield(12), Highway(13), Radar(14),
//        Naval(15), Missile(16), Harbor(17), Fort(18), Tech(19),
//        Bravery(20), LightIndus(21), HeavyIndus(22), Gold(23), Oil(24),
//        Unknown(25)
//
// Characteristics from sect.config; matched by semantic meaning (mnemonic
// may differ between Rust enum and C config).

const DCHR: &[SectorChr] = &[
    // Sea (-1 → index 0): "." — deep water
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 0, is_water: true, is_sanct: false, is_deity: true,
                is_enlist: false, name: "sea" },
    // Land (0 → index 1): "-" — wilderness
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "wilderness" },
    // Mountain (1 → index 2): "^" — mountain (produces gold dust)
    SectorChr { prd: PRD_DUST, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 100, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "mountain" },
    // Agri (2 → index 3): "g" — agribusiness (produces food)
    SectorChr { prd: PRD_FOOD, peff: 900, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "agribusiness" },
    // Uranium (3 → index 4): "u" — uranium mine
    SectorChr { prd: PRD_RAD, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "uranium mine" },
    // Plain (4 → index 5): "p" — plains (produces happiness)
    SectorChr { prd: PRD_HAP, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "plains" },
    // Park (5 → index 6): "P" — park (produces happiness)
    SectorChr { prd: PRD_HAP, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "park" },
    // Urban (6 → index 7): "c" — capital / city
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 1,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "capital" },
    // Research (7 → index 8): "r" — research lab (produces research levels)
    SectorChr { prd: PRD_MED, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "research lab" },
    // Wasteland (8 → index 9): "w" — wasteland
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 0, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "wasteland" },
    // Defense (9 → index 10): "d" — defense plant (produces guns)
    SectorChr { prd: PRD_GUN, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "defense plant" },
    // Bank (10 → index 11): "b" — bank (produces gold bars)
    SectorChr { prd: PRD_BAR, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "bank" },
    // Engineer (11 → index 12): "e" — enlistment center
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: true, name: "enlistment center" },
    // Airfield (12 → index 13): "a" — airfield
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "airfield" },
    // Highway (13 → index 14): "h" — highway
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "highway" },
    // Radar (14 → index 15): "j" — radar installation
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "radar installation" },
    // Naval (15 → index 16): "n" — naval base
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "naval base" },
    // Missile (16 → index 17): "m" — missile base
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "missile base" },
    // Harbor (17 → index 18): "*" — harbor
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "harbor" },
    // Fort (18 → index 19): "f" — fort
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 500, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "fort" },
    // Tech (19 → index 20): "t" — technical center (produces tech)
    SectorChr { prd: PRD_TECH, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "technical center" },
    // Bravery (20 → index 21): "s" — shrine of bravery (produces happiness)
    SectorChr { prd: PRD_HAP, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "shrine of bravery" },
    // LightIndus (21 → index 22): "l" — light manufacturing (produces LCM)
    SectorChr { prd: PRD_LCM, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "light manufacturing" },
    // HeavyIndus (22 → index 23): "k" — heavy manufacturing (produces HCM)
    SectorChr { prd: PRD_HCM, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "heavy manufacturing" },
    // Gold (23 → index 24): "G" — gold mine (produces gold dust)
    SectorChr { prd: PRD_DUST, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "gold mine" },
    // Oil (24 → index 25): "o" — oil field
    SectorChr { prd: PRD_OIL, peff: 100, bwork: 100, cost: 100, maint: 0,
                maxpop: 1000, is_water: false, is_sanct: false, is_deity: false,
                is_enlist: false, name: "oil field" },
    // Unknown (25 → index 26): "?" — uncharted / unknown
    SectorChr { prd: PRD_NONE, peff: 0, bwork: 100, cost: 0, maint: 0,
                maxpop: 0, is_water: false, is_sanct: false, is_deity: true,
                is_enlist: false, name: "unknown" },
];

impl SectorChr {
    /// Look up the descriptor for a sector type.
    /// `SectorType::Sea` (discriminant -1) maps to index 0.
    pub fn for_type(t: SectorType) -> &'static SectorChr {
        let idx = (t as i8 + 1) as usize;
        if idx < DCHR.len() { &DCHR[idx] } else { &DCHR[0] }
    }

    /// True if this sector type never produces anything.
    pub fn is_productive(&self) -> bool {
        self.prd >= 0 && self.peff > 0
    }
}
