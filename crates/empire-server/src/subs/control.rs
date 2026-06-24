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
// Ported from: src/lib/subs/control.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Markus Armbruster, 2014-2016

// Military control and sector abandonment checks.
// Pure functions; callers supply pre-loaded land unit slices.

use empire_types::sector::Sector;
use empire_types::land::LandUnit;
use empire_types::commodity::Item;

// ── Security strength ─────────────────────────────────────────────────────────

/// Calculate the military security strength in `sector`.
/// Returns `(total_strength, security_unit_effic_sum)`.
///   - `total_strength`: all military present (sector + own land units).
///   - `security_unit_effic_sum`: sum of efficiencies of L_SECURITY units
///     (units with the security flag), used for CHE suppression.
/// `land_units`: all land units at this sector's coordinates.
/// `is_security_unit`: closure returning true for land types with L_SECURITY flag.
///   (Requires land chr table — pass `|_| false` until LandChr is implemented.)
///
/// Equivalent to C's `security_strength(sp, seceffp)`.
pub fn security_strength(
    sector: &Sector,
    land_units: &[LandUnit],
    is_security_unit: impl Fn(i8) -> bool,
) -> (f64, i32) {
    let mut strength = sector.items.get(Item::Milit) as f64;
    let mut sec_eff = 0i32;

    for u in land_units {
        // Only count units owned by the sector owner, not aboard ships or other land units
        if u.own != sector.own { continue; }
        if u.ship >= 0 || u.carried_by_land >= 0 { continue; }

        let mil = u.items.get(Item::Milit) as f64;
        strength += mil;

        if is_security_unit(u.land_type) {
            // Security units add their own military count again (doubled effectiveness)
            strength += mil * u.effic as f64 / 100.0;
            sec_eff += u.effic as i32;
        }
    }

    (strength, sec_eff)
}

// ── Military control ──────────────────────────────────────────────────────────

/// Return true if the sector owner has military control.
/// A sector is out of control when: the old owner differs from current owner
/// AND (military * 10 < civilian count).
/// `land_units`: all land units at this sector's coordinates.
///
/// Equivalent to C's `military_control(sp)`.
pub fn military_control(
    sector: &Sector,
    land_units: &[LandUnit],
    is_security_unit: impl Fn(i8) -> bool,
) -> bool {
    if sector.old_own != sector.own {
        let (tot_mil, _) = security_strength(sector, land_units, is_security_unit);
        if (tot_mil * 10.0) < sector.items.get(Item::Civil) as f64 {
            return false;
        }
    }
    true
}

// ── Abandonment check ─────────────────────────────────────────────────────────

/// Return true if removing `remove_amount` of `item` from `sector`
/// (along with moving all land units in `moving_units` away) would
/// abandon the sector.
///
/// A sector is abandoned when it has no civilians, no military, and no
/// own land units remaining.
///
/// `land_units`: all land units currently in sector (owned by the player,
///   not aboard ships, not aboard other land units).
/// `moving_units`: UIDs of land units about to leave (subtracted from count).
///
/// Equivalent to C's `would_abandon(sp, vtype, amnt, land_list)`.
pub fn would_abandon(
    sector: &Sector,
    item: Option<Item>,
    remove_amount: i32,
    land_units: &[LandUnit],
    moving_unit_uids: &[i32],
) -> bool {
    // Only civ/mil removal can trigger abandonment
    if !matches!(item, Some(Item::Civil) | Some(Item::Milit)) {
        return false;
    }

    let mut mil  = sector.items.get(Item::Milit) as i32;
    let mut civs = sector.items.get(Item::Civil) as i32;

    if item == Some(Item::Milit) { mil  -= remove_amount; }
    if item == Some(Item::Civil) { civs -= remove_amount; }

    if sector.own == 0 || civs > 0 || mil > 0 {
        return false;
    }

    // Count own land units not aboard anything, then subtract those moving
    let own_land: i32 = land_units.iter()
        .filter(|u| u.own == sector.own && u.carried_by_land < 0 && u.ship < 0)
        .count() as i32;

    let remaining = own_land - moving_unit_uids.len() as i32;
    remaining <= 0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::sector::{Sector, SectorType, DistEntry, CHE_MAX};
    use empire_types::commodity::Inventory;

    fn make_sector(own: u8, civs: i16, mil: i16) -> Sector {
        let mut s = Sector {
            uid: 0, own, x: 0, y: 0,
            sector_type: SectorType::Urban, effic: 100, mobil: 127,
            off: false, loyal: 0, terr: [0; 4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: SectorType::Urban,
            min: 0, gmin: 0, fertil: 0, oil: 0, uran: 0,
            old_own: own,
            che: 0, che_target: 0,
            items: Inventory::zero(),
            del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        };
        let _ = CHE_MAX; // suppress unused warning
        s.items.set(Item::Civil, civs);
        s.items.set(Item::Milit, mil);
        s
    }

    #[test]
    fn control_owned_sector_always_controlled() {
        let s = make_sector(1, 1000, 0);
        // old_own == own, so military_control is trivially true
        assert!(military_control(&s, &[], |_| false));
    }

    #[test]
    fn control_lost_sector_no_mil() {
        let mut s = make_sector(2, 1000, 0);
        s.old_own = 1; // conquered
        // 0 * 10 < 1000 → not controlled
        assert!(!military_control(&s, &[], |_| false));
    }

    #[test]
    fn control_lost_sector_enough_mil() {
        let mut s = make_sector(2, 100, 20);
        s.old_own = 1; // conquered
        // 20 * 10 = 200 >= 100 → controlled
        assert!(military_control(&s, &[], |_| false));
    }

    #[test]
    fn would_abandon_no_civ_no_mil_no_land() {
        let s = make_sector(1, 0, 0);
        assert!(would_abandon(&s, Some(Item::Civil), 0, &[], &[]));
    }

    #[test]
    fn would_abandon_still_has_civs() {
        let s = make_sector(1, 100, 0);
        assert!(!would_abandon(&s, Some(Item::Civil), 50, &[], &[]));
    }
}
