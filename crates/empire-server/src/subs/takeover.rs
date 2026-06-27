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
// Ported from: src/lib/subs/takeover.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2007-2016

// Sector takeover logic: update ownership, generate guerrillas (CHE).
//
// NOTE: Unit (plane/land) takeover requires the land chr table (L_SPY flag)
// which is not yet implemented.  That portion is deferred to Phase 5 when
// the unit type descriptor tables are added.  See `takeover_units()` below.

use empire_types::sector::{Sector, CHE_MAX};
use empire_types::land::LandUnit;
use empire_types::plane::Plane;
use empire_types::coords::NatId;
use empire_types::commodity::Item;

// ── Sector takeover ───────────────────────────────────────────────────────────

/// Take over `sector` for `new_owner`.
///
/// This function:
/// 1. Clears distribution info (dist entries, thresholds).
/// 2. Sets `avail = 0` (no work this cycle).
/// 3. Generates CHE (guerrilla fighters) from civilian population.
/// 4. Adjusts loyalty and old_own.
/// 5. Sets `own = new_owner`.
///
/// `hap_factor`: happiness-based CHE suppression — from `hap_fact()` in C
///   (ratio of attacker's happiness to defender's).  Pass 1.0 if unknown.
/// `rng_n`: a random number in [0, 100) for probabilistic CHE generation.
///   (Use the session's PRNG or pass 0 for deterministic tests.)
///
/// Returns the list of changes to apply to the sector (the sector itself
/// is mutated; caller persists it via `empire_db::sectors::put`).
///
/// Equivalent to the core of C's `takeover(sp, newown)`.
pub fn takeover_sector(
    sector: &mut Sector,
    new_owner: NatId,
    hap_factor: f64,
    rng_n: i32,
) {
    // 1. Clear distribution state
    sector.del = [empire_types::sector::DistEntry::default(); 26];
    sector.dist_x = sector.x;
    sector.dist_y = sector.y;
    if sector.own == 0 {
        sector.off = false;
    } else {
        sector.off = true;
    }

    // 2. Workforce is zero — new occupier hasn't organized yet
    sector.avail = 0;

    // 3. Generate CHE from civilians
    let civ = sector.items.get(Item::Civil) as i32;
    let old_che = sector.che as i32;

    // n = (50 - loyalty) + (rng_n - 26)
    // rng_n is 0..100, subtracting 26 gives -26..74
    let n = (50 - sector.loyal as i32) + (rng_n - 26);

    let che_count = if n > 0 && sector.own == sector.old_own {
        // CHE only rises if we're taking over the original owner's sector
        let mut count = (civ * n / 3000) + 5;
        if count * 2 > civ { count = civ / 2; }
        // Attacker's happiness suppresses CHE
        if hap_factor > 0.0 {
            count = (count as f64 / hap_factor) as i32;
        }
        count = count.min(CHE_MAX as i32 - old_che);
        if count > 0 {
            sector.items.set(
                Item::Civil,
                (civ - count).max(0) as i16,
            );
            old_che + count
        } else {
            old_che
        }
    } else {
        old_che
    };

    sector.che = che_count.min(CHE_MAX as i32) as u8;

    // CHE target: who they fight
    if new_owner != sector.old_own {
        sector.che_target = new_owner;
    }
    if sector.che_target == 0 {
        sector.che = 0;
    }

    // 4. Loyalty and old_own
    if sector.old_own == new_owner || civ == 0 {
        // Re-taking your own sector: full loyalty reset
        sector.loyal   = 0;
        sector.old_own = new_owner;
    } else {
        // Taking someone else's sector: partial loyalty
        sector.loyal = 50;
    }

    // 5. Transfer ownership
    sector.own = new_owner;
}

// ── Unit takeover outcomes ────────────────────────────────────────────────────

/// Outcome of attempting to take over a single unit during sector conquest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitTakeoverOutcome {
    /// Unit is now owned by the conqueror (possibly reduced efficiency).
    Captured { new_effic: i8 },
    /// Crew destroyed the unit rather than surrender.
    BlownUp,
    /// Spy unit escaped detection (still owned by original owner).
    SpyEscaped,
    /// Spy unit was caught and executed.
    SpyExecuted,
}

/// Attempt to take over a land unit during sector conquest.
///
/// `rng_roll`: random 0..100 for resist/escape decisions.
/// `is_spy`: whether this land type has L_SPY flag (from LandChr table).
/// `spy_detect_threshold`: `LND_SPY_DETECT_CHANCE(effic)` — probability spy is caught.
///   Pass `effic / 2` as a percentage; or 0 to always let spies escape.
///
/// Returns `None` if unit should not be taken over (wrong owner, aboard ship, etc.).
/// Equivalent to the land-unit portion of C's `takeover()`.
pub fn takeover_land_unit(
    unit: &mut LandUnit,
    conqueror: NatId,
    sector_owner: NatId,
    rng_roll: i32,
    is_spy: bool,
    spy_detect_chance_pct: i32,
) -> Option<UnitTakeoverOutcome> {
    // Skip units that don't belong to the sector's (former) owner
    if unit.own == conqueror || unit.own == 0 { return None; }
    if unit.own != sector_owner { return None; }
    // Units aboard ships or other land units aren't captured here
    if unit.ship >= 0 || unit.carried_by_land >= 0 { return None; }

    if is_spy {
        if rng_roll >= spy_detect_chance_pct {
            // Spy evaded detection
            return Some(UnitTakeoverOutcome::SpyEscaped);
        }
        unit.own = 0; // Executed
        return Some(UnitTakeoverOutcome::SpyExecuted);
    }

    // Regular unit: crew may destroy it (efficiency loss 29..129)
    let n = (unit.effic as i32 - 29 - rng_roll).max(0);
    unit.effic = n as i8;

    if unit.effic < empire_types::land::LAND_MIN_EFF {
        unit.effic = 0;
        Some(UnitTakeoverOutcome::BlownUp)
    } else {
        unit.own = conqueror;
        Some(UnitTakeoverOutcome::Captured { new_effic: unit.effic })
    }
}

/// Attempt to take over a plane during sector conquest.
/// Planes are always captured (no spy/destroy logic for planes).
///
/// Returns `None` if unit should not be taken over.
/// Equivalent to the plane portion of C's `takeover()`.
pub fn takeover_plane(
    plane: &mut Plane,
    conqueror: NatId,
    sector_owner: NatId,
) -> Option<()> {
    if plane.own != sector_owner { return None; }
    plane.own = conqueror;
    Some(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::sector::{Sector, SectorType, DistEntry};
    use empire_types::commodity::Inventory;

    fn make_sector(own: u8, old_own: u8, loyal: u8, civs: i16) -> Sector {
        let mut s = Sector {
            uid: 0, own, x: 4, y: 4,
            sector_type: SectorType::Capital, effic: 80, mobil: 0,
            off: false, loyal, terr: [0; 4], dterr: 0,
            dist_x: 10, dist_y: 10, avail: 50, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Capital,
            min: 0, gmin: 0, fertil: 0, oil: 0, uran: 0,
            old_own, che: 0, che_target: 0,
            items: Inventory::zero(),
            del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        s.items.set(Item::Civil, civs);
        s
    }

    #[test]
    fn takeover_changes_owner() {
        let mut s = make_sector(1, 1, 0, 500);
        takeover_sector(&mut s, 2, 1.0, 50);
        assert_eq!(s.own, 2);
    }

    #[test]
    fn takeover_clears_dist() {
        let mut s = make_sector(1, 1, 0, 500);
        s.dist_x = 20; s.dist_y = 20;
        takeover_sector(&mut s, 2, 1.0, 50);
        assert_eq!(s.dist_x, s.x);
        assert_eq!(s.dist_y, s.y);
    }

    #[test]
    fn takeover_loyal_zero_when_recapturing() {
        // Taking back your own old sector → loyalty 0
        let mut s = make_sector(1, 2, 50, 500);
        takeover_sector(&mut s, 2, 1.0, 50);
        assert_eq!(s.loyal, 0);
        assert_eq!(s.old_own, 2);
    }

    #[test]
    fn takeover_loyal_50_enemy_sector() {
        let mut s = make_sector(1, 1, 0, 500);
        takeover_sector(&mut s, 2, 1.0, 50);
        assert_eq!(s.loyal, 50);
    }

    #[test]
    fn takeover_avail_zeroed() {
        let mut s = make_sector(1, 1, 0, 500);
        s.avail = 200;
        takeover_sector(&mut s, 2, 1.0, 50);
        assert_eq!(s.avail, 0);
    }
}
