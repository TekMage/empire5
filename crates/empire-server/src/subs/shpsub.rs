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
// Ported from: src/lib/subs/shpsub.c, src/lib/subs/landgun.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2006-2021

// Ship combat subsystem — gunnery, torpedoes, range, and damage helpers.
// All functions are pure (no I/O, no DB); callers persist mutated objects.

use rand::Rng;
use empire_types::ship::Ship;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};
use empire_types::commodity::Item;
use empire_types::coords::Coord;
use crate::subs::geo::map_dist;
use crate::subs::tech::techfact;

/// Guns a ship can fire this shot, before checking ammo on hand: the ship
/// type's gun limit, the guns actually carried, and half the crew (rounded
/// up), whichever is smallest. Mirrors `shp_usable_guns()`/`shp_fire()`'s
/// first clamp in landgun.c.
pub fn shp_guns_fired(mil: i32, item_gun: i32, glim: i32) -> i32 {
    glim.min(item_gun).min((mil + 1) / 2).max(0)
}

/// Shells needed to fire `guns` guns: `(guns+1)/2`.
pub fn shp_shells_needed(guns: i32) -> i32 {
    if guns <= 0 { 0 } else { (guns + 1) / 2 }
}

/// Re-clamp `guns` to however many shells are actually on hand (2 guns per
/// shell). Mirrors `shp_fire()`'s second clamp, applied after the first.
pub fn shp_guns_after_ammo(guns: i32, shells_available: i32) -> i32 {
    guns.min(shells_available * 2).max(0)
}

/// Eligibility to fire deck guns: effic>=60, has a gun limit, has guns and
/// shells aboard, has at least one crew. Mirrors `c_fire()`'s ship branch.
pub fn shp_can_fire(ship: &Ship, mchr: &ShipChr) -> bool {
    ship.effic >= 60
        && mchr.glim != 0
        && ship.items.get(Item::Gun) > 0
        && ship.items.get(Item::Shell) > 0
        && ship.items.get(Item::Milit) >= 1
}

/// Eligibility to fire torpedoes: TORP capability, guns+shells+crew aboard,
/// effic>=60, mobility remaining. Mirrors `shp_torp()`'s gate.
pub fn shp_can_torp(ship: &Ship, mchr: &ShipChr) -> bool {
    mchr.flags.contains(ShipChrFlags::TORP)
        && ship.items.get(Item::Gun) > 0
        && ship.items.get(Item::Shell) >= 3
        && ship.items.get(Item::Milit) >= 1
        && ship.effic >= 60
        && ship.mobil > 0
}

/// `seagun()`: effic/100 * sum_{1..guns}(9 + roll(0..6)).
pub fn seagun_damage(effic: i8, guns: i32, rng: &mut impl Rng) -> i32 {
    if guns <= 0 { return 0; }
    let raw: i32 = (0..guns).map(|_| 9 + rng.gen_range(0..6)).sum();
    (raw * effic as i32) / 100
}

/// Effective gun range for `ship`: `techfact(tech, frnge/2)`.
pub fn shp_gun_range(ship: &Ship, mchr: &ShipChr) -> i32 {
    techfact(mchr.frnge as f64 / 2.0, ship.tech as f64) as i32
}

/// Effective torpedo range for `ship`: `techfact(tech, frnge*2) * effic/100`.
pub fn shp_torp_range(ship: &Ship, mchr: &ShipChr) -> i32 {
    (techfact(mchr.frnge as f64 * 2.0, ship.tech as f64) * ship.effic as f64 / 100.0) as i32
}

/// Return true if the ship's guns can reach (tx, ty). Wraps `geo::map_dist`
/// against the tech-scaled gun range (not the raw `mchr.frnge`).
pub fn shp_in_range(
    ship: &Ship,
    mchr: &ShipChr,
    tx: Coord,
    ty: Coord,
    world_x: i32,
    world_y: i32,
) -> bool {
    let range = shp_gun_range(ship, mchr);
    if range <= 0 {
        return false;
    }
    map_dist(ship.x, ship.y, tx, ty, world_x, world_y) <= range
}

/// Torpedo damage roll (rolled unconditionally by the caller; only applied
/// on a hit): `40 + roll(0..40) + roll(0..40)`.
pub fn torpedo_damage_roll(rng: &mut impl Rng) -> i32 {
    40 + rng.gen_range(0..40) + rng.gen_range(0..40)
}

/// Torpedo hit chance. `visibility` is the *firing* ship's own visibility
/// stat (a stealthy attacker gets a better shot, not a harder-to-hit
/// target) — confirmed against `shp_torp_hitchance()`, which takes the
/// firer, not the victim.
pub fn torpedo_hit_chance(range: i32, visibility: i32) -> f64 {
    let base = 0.9 / (range as f64 + 1.0);
    let vis_bonus = if visibility < 6 { (5 - visibility) as f64 * 0.03 } else { 0.0 };
    base + vis_bonus
}

/// Apply `dam` damage to `ship`, adjusted for armor.  Returns `true` if the
/// ship is sunk (effic <= 0).  Delegates to `damage::ship_damage_armored`.
pub fn shp_damage(ship: &mut Ship, dam: i32, armor_pct: i32) -> bool {
    if dam > 0 {
        super::damage::ship_damage_armored(ship, dam, armor_pct, 1.0);
    }
    ship.effic <= 0
}

/// Return true if the ship is still afloat (effic > 0).
pub fn shp_is_afloat(ship: &Ship) -> bool {
    ship.effic > 0
}

/// Result of one ship-vs-ship gunnery exchange.
pub struct GunneryResult {
    pub damage_dealt: i32,
    pub target_sunk: bool,
    pub guns: i32,
    pub shells_used: i32,
}

/// One ship-vs-ship gunnery exchange: computes guns/damage, deducts ammo and
/// mobility from `attacker`, applies armor-adjusted damage to `defender`.
/// Caller has already checked range/eligibility. A second call with the
/// roles reversed is how counter-fire is implemented — this function never
/// calls itself, so there's no recursion risk.
pub fn shp_fire_at_ship(
    attacker: &mut Ship,
    att_mchr: &ShipChr,
    defender: &mut Ship,
    def_mchr: &ShipChr,
    rng: &mut impl Rng,
) -> GunneryResult {
    let guns = shp_guns_fired(
        attacker.items.get(Item::Milit) as i32,
        attacker.items.get(Item::Gun) as i32,
        att_mchr.glim,
    );
    let guns = shp_guns_after_ammo(guns, attacker.items.get(Item::Shell) as i32);
    if guns <= 0 {
        return GunneryResult { damage_dealt: 0, target_sunk: false, guns: 0, shells_used: 0 };
    }

    let shells_used = shp_shells_needed(guns);
    attacker.items.add(Item::Shell, -(shells_used as i16));
    attacker.mobil = (attacker.mobil as i32 - 15).max(-100) as i8;

    let dam = seagun_damage(attacker.effic, guns, rng);
    let sunk = shp_damage(defender, dam, def_mchr.armor);

    GunneryResult { damage_dealt: dam, target_sunk: sunk, guns, shells_used }
}

/// Result of one ship-vs-ship torpedo shot.
pub struct TorpResult {
    pub hit: bool,
    pub damage: i32,
    pub target_sunk: bool,
}

/// One ship-vs-ship torpedo shot: deducts 3 shells and mobility from
/// `attacker` unconditionally, rolls damage and hit chance, applies damage
/// to `defender` only on a hit. Caller has already checked eligibility
/// (`shp_can_torp`) and range. A second call with roles reversed implements
/// counter-fire — no recursion.
pub fn shp_torp_at_ship(
    attacker: &mut Ship,
    att_mchr: &ShipChr,
    defender: &mut Ship,
    def_mchr: &ShipChr,
    dist: i32,
    rng: &mut impl Rng,
) -> TorpResult {
    attacker.items.add(Item::Shell, -3);
    // v1 simplification: flat mobility cost (no generic ship move-cost
    // formula exists yet in empire5 to derive "half of").
    attacker.mobil = (attacker.mobil as i32 - 10).max(-100) as i8;

    let dam_roll = torpedo_damage_roll(rng);
    let hit_chance = torpedo_hit_chance(dist, att_mchr.visib);
    let hit = rng.gen::<f64>() < hit_chance;

    if !hit {
        return TorpResult { hit: false, damage: 0, target_sunk: false };
    }

    let sunk = shp_damage(defender, dam_roll, def_mchr.armor);
    TorpResult { hit: true, damage: dam_roll, target_sunk: sunk }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use empire_types::ship::{Ship, RetreatFlags};
    use empire_types::ship_chr::{ShipChr, ShipChrFlags};
    use empire_types::commodity::Inventory;

    fn make_ship(effic: i8, tech: i16) -> Ship {
        Ship {
            uid: 0, own: 1, x: 0, y: 0, ship_type: 13, // battleship
            effic, mobil: 60, off: false, tech,
            fleet: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            items: Inventory::zero(), pstage: 0, ptime: 0, access: 0,
            name: "USS Test".into(),
            orig_x: 0, orig_y: 0, orig_own: 1,
            retreat_flags: RetreatFlags::empty(), retreat_path: String::new(),
        }
    }

    fn battleship_chr() -> ShipChr {
        ShipChr {
            name: "battleship", sname: "bb",
            lcm: 50, hcm: 70, bwork: 210, tech: 45, cost: 1800,
            armor: 95, speed: 25, visib: 35, vrnge: 6, frnge: 10, glim: 7,
            nxlight: 1, nchoppers: 0, nplanes: 0, nland: 2,
            flags: ShipChrFlags::empty(),
        }
    }

    fn submarine_chr() -> ShipChr {
        ShipChr {
            name: "submarine", sname: "sb",
            lcm: 30, hcm: 30, bwork: 110, tech: 60, cost: 650,
            armor: 25, speed: 20, visib: 5, vrnge: 4, frnge: 3, glim: 3,
            nxlight: 0, nchoppers: 0, nplanes: 0, nland: 0,
            flags: ShipChrFlags::TORP.union(ShipChrFlags::SUBMARINE),
        }
    }

    #[test]
    fn guns_fired_limited_by_crew() {
        // mil=3 -> (3+1)/2=2, glim=7, item_gun=7 -> min is 2
        assert_eq!(shp_guns_fired(3, 7, 7), 2);
    }

    #[test]
    fn guns_fired_limited_by_glim() {
        assert_eq!(shp_guns_fired(100, 7, 3), 3);
    }

    #[test]
    fn guns_fired_zero_glim() {
        assert_eq!(shp_guns_fired(100, 7, 0), 0);
    }

    #[test]
    fn shells_needed_rounds_up() {
        assert_eq!(shp_shells_needed(1), 1);
        assert_eq!(shp_shells_needed(2), 1);
        assert_eq!(shp_shells_needed(3), 2);
        assert_eq!(shp_shells_needed(0), 0);
    }

    #[test]
    fn guns_after_ammo_clamps_down() {
        // 5 guns wanted, but only 1 shell on hand -> 2 guns worth of ammo
        assert_eq!(shp_guns_after_ammo(5, 1), 2);
        assert_eq!(shp_guns_after_ammo(5, 10), 5);
    }

    #[test]
    fn can_fire_requires_effic_60() {
        let mut ship = make_ship(59, 100);
        ship.items.set(Item::Gun, 5);
        ship.items.set(Item::Shell, 5);
        ship.items.set(Item::Milit, 5);
        assert!(!shp_can_fire(&ship, &battleship_chr()));
        ship.effic = 60;
        assert!(shp_can_fire(&ship, &battleship_chr()));
    }

    #[test]
    fn can_fire_requires_ammo_and_crew() {
        let mut ship = make_ship(100, 100);
        ship.items.set(Item::Milit, 5);
        assert!(!shp_can_fire(&ship, &battleship_chr()), "no guns/shells yet");
        ship.items.set(Item::Gun, 5);
        assert!(!shp_can_fire(&ship, &battleship_chr()), "still no shells");
        ship.items.set(Item::Shell, 5);
        assert!(shp_can_fire(&ship, &battleship_chr()));
    }

    #[test]
    fn can_torp_requires_torp_flag() {
        let mut ship = make_ship(100, 100);
        ship.items.set(Item::Gun, 5);
        ship.items.set(Item::Shell, 5);
        ship.items.set(Item::Milit, 5);
        assert!(!shp_can_torp(&ship, &battleship_chr()), "battleship has no TORP flag");
        assert!(shp_can_torp(&ship, &submarine_chr()));
    }

    #[test]
    fn can_torp_requires_three_shells() {
        let mut ship = make_ship(100, 100);
        ship.items.set(Item::Gun, 5);
        ship.items.set(Item::Shell, 2);
        ship.items.set(Item::Milit, 5);
        assert!(!shp_can_torp(&ship, &submarine_chr()));
    }

    #[test]
    fn seagun_damage_scales_with_effic() {
        let mut rng = StdRng::seed_from_u64(1);
        let full = seagun_damage(100, 3, &mut rng);
        let mut rng2 = StdRng::seed_from_u64(1);
        let half = seagun_damage(50, 3, &mut rng2);
        assert_eq!(half, full / 2);
    }

    #[test]
    fn seagun_damage_zero_guns() {
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(seagun_damage(100, 0, &mut rng), 0);
    }

    #[test]
    fn gun_range_scales_with_tech() {
        let low = make_ship(100, 0);
        let high = make_ship(100, 200);
        let chr = battleship_chr(); // frnge=10
        assert!(shp_gun_range(&high, &chr) > shp_gun_range(&low, &chr));
    }

    #[test]
    fn in_range_true() {
        let ship = make_ship(100, 100);
        let chr = battleship_chr(); // frnge=10 -> techfact(5.0,100)=2.14
        let range = shp_gun_range(&ship, &chr);
        assert!(shp_in_range(&ship, &chr, range as i16, 0, 64, 32));
    }

    #[test]
    fn in_range_false_too_far() {
        let ship = make_ship(100, 100);
        let chr = battleship_chr();
        assert!(!shp_in_range(&ship, &chr, 30, 0, 64, 32));
    }

    #[test]
    fn torpedo_hit_chance_bonus_for_stealthy_firer() {
        let sub_chance = torpedo_hit_chance(2, 2); // visib=2, well under the <6 cutoff
        let battleship_chance = torpedo_hit_chance(2, 35);
        assert!(sub_chance > battleship_chance);
    }

    #[test]
    fn torpedo_hit_chance_decays_with_range() {
        let close = torpedo_hit_chance(0, 35);
        let far = torpedo_hit_chance(20, 35);
        assert!(close > far);
    }

    #[test]
    fn torpedo_damage_roll_bounds() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let d = torpedo_damage_roll(&mut rng);
            assert!((40..=118).contains(&d), "roll {d} out of bounds");
        }
    }

    #[test]
    fn shp_damage_no_armor_vs_armored() {
        let mut bare = make_ship(100, 100);
        let mut armored = make_ship(100, 100);
        shp_damage(&mut bare, 50, 0);
        shp_damage(&mut armored, 50, 95);
        assert!(armored.effic > bare.effic, "armor should reduce effective damage");
    }

    #[test]
    fn shp_damage_reports_sunk_when_zero() {
        let mut ship = make_ship(0, 100);
        assert!(shp_damage(&mut ship, 0, 0));
    }

    #[test]
    fn shp_is_afloat_checks_effic() {
        let mut ship = make_ship(1, 100);
        assert!(shp_is_afloat(&ship));
        ship.effic = 0;
        assert!(!shp_is_afloat(&ship));
    }

    #[test]
    fn fire_at_ship_deducts_ammo_and_mobility() {
        let mut attacker = make_ship(100, 100);
        attacker.items.set(Item::Gun, 7);
        attacker.items.set(Item::Shell, 10);
        attacker.items.set(Item::Milit, 20);
        let mut defender = make_ship(100, 100);
        let chr = battleship_chr();
        let mut rng = StdRng::seed_from_u64(3);

        let result = shp_fire_at_ship(&mut attacker, &chr, &mut defender, &chr, &mut rng);

        assert!(result.guns > 0);
        assert_eq!(attacker.mobil, 60 - 15);
        assert!(attacker.items.get(Item::Shell) < 10);
        assert!(defender.effic < 100);
    }

    #[test]
    fn fire_at_ship_no_guns_no_effect() {
        let mut attacker = make_ship(100, 100); // no guns/shells/mil loaded
        let mut defender = make_ship(100, 100);
        let chr = battleship_chr();
        let mut rng = StdRng::seed_from_u64(3);

        let result = shp_fire_at_ship(&mut attacker, &chr, &mut defender, &chr, &mut rng);

        assert_eq!(result.damage_dealt, 0);
        assert_eq!(defender.effic, 100);
        assert_eq!(attacker.mobil, 60, "no mobility spent when nothing fires");
    }

    #[test]
    fn torp_at_ship_spends_three_shells_on_miss_too() {
        let mut attacker = make_ship(100, 100);
        attacker.items.set(Item::Gun, 3);
        attacker.items.set(Item::Shell, 10);
        attacker.items.set(Item::Milit, 5);
        let mut defender = make_ship(100, 100);
        let att_chr = submarine_chr();
        let def_chr = battleship_chr();
        // long range -> ~0 hit chance, should reliably miss
        let mut rng = StdRng::seed_from_u64(42);

        let result = shp_torp_at_ship(&mut attacker, &att_chr, &mut defender, &def_chr, 1000, &mut rng);

        assert!(!result.hit);
        assert_eq!(attacker.items.get(Item::Shell), 7, "3 shells spent regardless of hit");
        assert_eq!(defender.effic, 100);
    }
}
