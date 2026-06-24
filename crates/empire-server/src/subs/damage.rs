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
// Ported from: src/lib/subs/damage.c
// Known contributors to the original:
//    Dave Pare, 1989
//    Steve McClure, 1997
//    Markus Armbruster, 2004-2012

// Damage application functions.
// All functions are pure (no I/O, no DB); callers persist the mutated objects.

use empire_types::commodity::{Inventory, Item};
use empire_types::ship::Ship;
use empire_types::land::LandUnit;
use empire_types::plane::Plane;

// ── Core damage primitives ────────────────────────────────────────────────────

/// Convert a "raw" damage value to an effective damage percentage.
/// Equivalent to the C macro `PERCENT_DAMAGE(x) = 100*x/(x+100)`.
/// This asymptotically approaches 100% — even infinite fire never quite
/// destroys everything.
pub fn percent_damage(x: i32) -> i32 {
    100 * x / (x + 100)
}

/// Apply `pct` percentage damage to `amt`, rounding toward nearest.
/// Equivalent to C's `damage(amt, pct)`.
/// Returns the surviving amount (amt – loss).
pub fn damage(amt: i32, pct: i32) -> i32 {
    if amt <= 0 { return 0; }
    let loss = (amt as f64 * pct as f64 / 100.0).round() as i32;
    (amt - loss).max(0)
}

/// Apply asymptotic ("effdamage") damage — uses `percent_damage` first.
/// Equivalent to C's `effdamage(amt, dam)`.
pub fn eff_damage(amt: i32, dam: i32) -> i32 {
    damage(amt, percent_damage(dam))
}

// ── Commodity damage ─────────────────────────────────────────────────────────

/// Damage all commodities in an inventory by `pct` percent.
/// People (Civil, Milit, Uw) are further reduced by `people_damage` factor.
/// Equivalent to C's `item_damage(pct, item)`.
/// `people_damage`: econfig "people_damage" (default 1.0; 0.0 = no civ casualties).
pub fn item_damage(pct: i32, inv: &mut Inventory, people_damage: f64) {
    for item in [
        Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
        Item::Iron,  Item::Dust,  Item::Bar,   Item::Food, Item::Oil,
        Item::Lcm,   Item::Hcm,  Item::Uw,    Item::Rad,
    ] {
        let cur = inv.get(item) as i32;
        if cur == 0 { continue; }
        let mut lose = (cur as f64 * pct as f64 / 100.0).round() as i32;
        if matches!(item, Item::Civil | Item::Milit | Item::Uw) {
            lose = (people_damage * lose as f64).round() as i32;
        }
        let remaining = (cur - lose).max(0);
        inv.set(item, remaining as i16);
    }
}

/// Apply commodity damage accounting for item-specific rules.
/// Equivalent to C's `commdamage(amt, dam, vtype)`.
pub fn comm_damage(amt: i32, dam: i32, item: Item, people_damage: f64) -> i32 {
    let lost = amt - eff_damage(amt, dam);
    let effective_lost = if matches!(item, Item::Civil | Item::Milit | Item::Uw) {
        (people_damage * lost as f64).round() as i32
    } else {
        lost
    };
    (amt - effective_lost).max(0)
}

// ── Unit damage ───────────────────────────────────────────────────────────────

/// Apply raw damage to a ship (bypasses armor).
/// Equivalent to C's `ship_damage(sp, dam)`.
/// Caller is responsible for logging and persisting.
pub fn ship_damage(ship: &mut Ship, dam: i32, people_damage: f64) {
    if dam <= 0 { return; }
    let dam = dam.min(100);
    ship.effic = damage(ship.effic as i32, dam) as i8;
    if ship.mobil > 0 {
        ship.mobil = damage(ship.mobil as i32, dam) as i8;
    }
    item_damage(dam, &mut ship.items, people_damage);
}

/// Apply armor-adjusted damage to a ship.
/// Equivalent to C's `shipdamage(sp, dam)` with shp_armor lookup.
/// `armor_pct`: ship type's armor rating (0 = no armor; 100 = half damage).
pub fn ship_damage_armored(ship: &mut Ship, dam: i32, armor_pct: i32, people_damage: f64) {
    let effective = (dam as f64 / (1.0 + armor_pct as f64 / 100.0)) as i32;
    ship_damage(ship, effective, people_damage);
}

/// Apply raw damage to a land unit.
/// Equivalent to C's `land_damage(lp, dam)`.
/// `is_spy`: from lchr[land_type].l_flags & L_SPY — spies die instantly.
pub fn land_damage(land: &mut LandUnit, dam: i32, is_spy: bool, people_damage: f64) {
    if dam <= 0 { return; }
    let dam = dam.min(100);
    if is_spy {
        land.effic = 0;
    } else {
        land.effic = damage(land.effic as i32, dam) as i8;
        if land.mobil > 0 {
            land.mobil = damage(land.mobil as i32, dam) as i8;
        }
        item_damage(dam, &mut land.items, people_damage);
    }
}

/// Apply vulnerability-and-fortification adjusted damage to a land unit.
/// Equivalent to C's `landdamage(lp, dam)`.
/// `land_mob_max`: configured maximum mobility (default 127).
/// `vul_pct`: unit vulnerability percentage from lchr (0..200+).
pub fn land_damage_combat(
    land: &mut LandUnit,
    dam: i32,
    land_mob_max: f64,
    vul_pct: i32,
    is_spy: bool,
    people_damage: f64,
) {
    let factor = land_mob_max / (land_mob_max + land.harden as f64)
        * vul_pct as f64 / 100.0;
    let effective = (factor * dam as f64).round() as i32;
    land_damage(land, effective, is_spy, people_damage);
}

/// Apply raw damage to a plane.
/// Equivalent to C's `planedamage(pp, dam)`.
pub fn plane_damage(plane: &mut Plane, dam: i32) {
    if dam <= 0 { return; }
    let dam = dam.min(100);
    plane.effic = damage(plane.effic as i32, dam) as i8;
    if plane.mobil > 0 {
        plane.mobil = damage(plane.mobil as i32, dam) as i8;
    }
}

// ── Nuke damage calculation ───────────────────────────────────────────────────

/// Calculate nuke damage at `range` sectors from ground zero.
/// `blast`: weapon blast radius (nchr.n_blast).
/// `dam_pct`: weapon's maximum damage percentage (nchr.n_dam).
/// `airburst`: true for airburst (wider area, lower peak damage).
/// Returns 0 if target is out of blast radius or damage < 5%.
/// Equivalent to C's `nukedamage(ncp, range, airburst)`.
pub fn nuke_damage(blast: i32, dam_pct: i32, range: i32, airburst: bool) -> i32 {
    let rad = if airburst { (blast as f64 * 1.5) as i32 } else { blast };
    if rad < range { return 0; }
    let dam = if airburst {
        (dam_pct as f64 * 0.75) as i32 - range * 20
    } else {
        (dam_pct as f64 / (range as f64 + 1.0)) as i32
    };
    if dam < 5 { 0 } else { dam }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::commodity::Inventory;

    #[test]
    fn percent_damage_midpoint() {
        // PERCENT_DAMAGE(100) = 100*100/(100+100) = 50
        assert_eq!(percent_damage(100), 50);
    }

    #[test]
    fn damage_zero_input() {
        assert_eq!(damage(0, 50), 0);
    }

    #[test]
    fn damage_full() {
        // 100% damage = lose everything
        assert_eq!(damage(100, 100), 0);
    }

    #[test]
    fn damage_half() {
        assert_eq!(damage(100, 50), 50);
    }

    #[test]
    fn item_damage_leaves_zero_items_alone() {
        let mut inv = Inventory::zero();
        item_damage(50, &mut inv, 1.0);
        assert_eq!(inv.get(Item::Civil), 0);
    }

    #[test]
    fn item_damage_reduces_commodities() {
        let mut inv = Inventory::zero();
        inv.set(Item::Shell, 100);
        item_damage(50, &mut inv, 1.0);
        assert_eq!(inv.get(Item::Shell), 50);
    }

    #[test]
    fn nuke_damage_at_zero_range() {
        let dam = nuke_damage(5, 100, 0, false);
        assert_eq!(dam, 100); // dam_pct/(0+1) = 100
    }

    #[test]
    fn nuke_damage_out_of_range() {
        assert_eq!(nuke_damage(5, 100, 10, false), 0);
    }
}
