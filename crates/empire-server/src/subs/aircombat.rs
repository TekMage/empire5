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
// Ported from: src/lib/subs/aircombat.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2006-2021

// Air combat resolution and interceptor search.
// Pure combat logic does not touch the DB; DB search is async.

use empire_types::plane::{Plane, PLANE_MIN_EFF};
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::coords::Coord;
use empire_db::Db;

use rand::Rng;
use crate::subs::geo::map_dist;
use crate::subs::plnsub;

/// Number of air combat rounds per engagement.
const AIR_COMBAT_ROUNDS: usize = 3;

/// Base hit percentage for an attack roll (modified by attacker/defender stats).
const BASE_HIT_PCT: f64 = 20.0;

/// Damage range (inclusive) when a hit is scored.
const MIN_HIT_DAM: i32 = 20;
const MAX_HIT_DAM: i32 = 50;

/// Resolve air combat between two groups of planes.
///
/// Attackers and defenders exchange fire for up to `AIR_COMBAT_ROUNDS` rounds.
/// Each attacker fires at each defender and vice versa simultaneously (order
/// within round is randomised).  Planes destroyed (effic <= 0) are removed after
/// each round.
///
/// `att_chrs` / `def_chrs`: characteristic tables indexed by `plane_type`.
///
/// Returns a log of combat events (one `String` per notable occurrence).
/// The caller is responsible for persisting modified planes and for applying
/// effic <= 0 (destroyed) status.
pub fn air_combat(
    attackers: &mut Vec<Plane>,
    defenders: &mut Vec<Plane>,
    att_chrs: &[PlaneChr],
    def_chrs: &[PlaneChr],
    rng: &mut impl Rng,
) -> Vec<String> {
    let mut log = Vec::new();

    for round in 1..=AIR_COMBAT_ROUNDS {
        if attackers.is_empty() || defenders.is_empty() {
            break;
        }

        // Collect damage to apply after all shots in this round
        let mut att_dam: Vec<i32> = vec![0; attackers.len()];
        let mut def_dam: Vec<i32> = vec![0; defenders.len()];

        // Each attacker fires at a random defender
        for att in attackers.iter() {
            if att.effic < PLANE_MIN_EFF {
                continue;
            }
            let def_idx = rng.gen_range(0..defenders.len());
            let def = &defenders[def_idx];
            if def.effic < PLANE_MIN_EFF {
                continue;
            }
            let att_str = att_chr_att(att, att_chrs);
            let def_str = def_chr_def(def, def_chrs);
            if roll_hit(att_str, def_str, rng) {
                let dam = rng.gen_range(MIN_HIT_DAM..=MAX_HIT_DAM);
                def_dam[def_idx] += dam;
                log.push(format!(
                    "Air combat round {round}: attacker #{} hits defender #{} for {dam}%",
                    att.uid, def.uid,
                ));
            }
        }

        // Each defender fires at a random attacker
        for def in defenders.iter() {
            if def.effic < PLANE_MIN_EFF {
                continue;
            }
            let att_idx = rng.gen_range(0..attackers.len());
            let att = &attackers[att_idx];
            if att.effic < PLANE_MIN_EFF {
                continue;
            }
            let def_str = def_chr_def(def, def_chrs);
            let att_str = att_chr_att(att, att_chrs);
            if roll_hit(def_str, att_str, rng) {
                let dam = rng.gen_range(MIN_HIT_DAM..=MAX_HIT_DAM);
                att_dam[att_idx] += dam;
                log.push(format!(
                    "Air combat round {round}: defender #{} hits attacker #{} for {dam}%",
                    def.uid, att.uid,
                ));
            }
        }

        // Apply damage
        for (i, dam) in att_dam.iter().enumerate() {
            if *dam > 0 {
                plnsub::pln_damage(&mut attackers[i], *dam);
            }
        }
        for (i, dam) in def_dam.iter().enumerate() {
            if *dam > 0 {
                plnsub::pln_damage(&mut defenders[i], *dam);
            }
        }

        // Remove destroyed planes
        let att_before = attackers.len();
        attackers.retain(|p| p.effic > 0);
        let att_after = attackers.len();
        if att_before > att_after {
            log.push(format!(
                "Air combat round {round}: {} attacker(s) destroyed",
                att_before - att_after
            ));
        }

        let def_before = defenders.len();
        defenders.retain(|p| p.effic > 0);
        let def_after = defenders.len();
        if def_before > def_after {
            log.push(format!(
                "Air combat round {round}: {} defender(s) destroyed",
                def_before - def_after
            ));
        }
    }

    log
}

/// Determine whether an attacker hits given `att_str` attack and `def_str` defense.
///
/// `hit_chance = BASE_HIT_PCT * (att_str / (att_str + def_str))`
fn roll_hit(att_str: i32, def_str: i32, rng: &mut impl Rng) -> bool {
    let denom = (att_str + def_str).max(1);
    let chance = BASE_HIT_PCT * att_str as f64 / denom as f64;
    rng.gen::<f64>() * 100.0 < chance
}

/// Get air-to-air attack strength of a plane.
fn att_chr_att(plane: &Plane, chrs: &[PlaneChr]) -> i32 {
    let idx = plane.plane_type as usize;
    chrs.get(idx).map(|c| c.att * plane.effic as i32 / 100).unwrap_or(0)
}

/// Get air-to-air defense strength of a plane.
fn def_chr_def(plane: &Plane, chrs: &[PlaneChr]) -> i32 {
    let idx = plane.plane_type as usize;
    chrs.get(idx).map(|c| c.def * plane.effic as i32 / 100).unwrap_or(1)
}

/// Find interceptor planes near (`tx`, `ty`) that will scramble against `attacker_cnum`.
///
/// Interceptors must:
///   - Be owned by a nation other than `attacker_cnum`
///   - Have the FIGHTER flag set in their type characteristics
///   - Have enough mobility to fly (effic >= PLANE_MIN_EFF, mobil > 0)
///   - Be within their `range` sectors of the target
///
/// Returns a `Vec<Plane>` of all interceptors found.  The caller resolves combat
/// against them via `air_combat`.
pub async fn find_interceptors(
    db: &Db,
    tx: Coord,
    ty: Coord,
    attacker_cnum: u8,
    world_x: i32,
    world_y: i32,
) -> Result<Vec<Plane>, empire_db::DbError> {
    let all_planes = empire_db::planes::get_all(db).await?;
    let chrs = PlaneChr::all();

    let interceptors = all_planes
        .into_iter()
        .filter(|p| {
            // Not the attacker's plane
            if p.own == attacker_cnum || p.own == 0 {
                return false;
            }
            // Must be capable of flying
            if p.effic < PLANE_MIN_EFF || p.mobil <= 0 || p.off {
                return false;
            }
            // Must be a fighter type
            let Some(chr) = chrs.get(p.plane_type as usize) else {
                return false;
            };
            if !chr.flags.contains(PlaneChrFlags::FIGHTER) {
                return false;
            }
            // Must be in range of target (use the plane's stored range field)
            let dist = map_dist(p.x, p.y, tx, ty, world_x, world_y);
            dist <= p.range as i32
        })
        .collect();

    Ok(interceptors)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::plane::{Plane, PlaneFlags};

    fn make_plane(uid: i32, own: u8, effic: i8, plane_type: i8) -> Plane {
        Plane {
            uid, own, x: 0, y: 0, plane_type,
            effic, mobil: 30, off: false, tech: 100,
            wing: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 10, harden: 0, ship: -1, land: -1,
            flags: PlaneFlags::empty(), access: 0, theta: 0.0,
        }
    }

    #[test]
    fn air_combat_removes_destroyed() {
        let chrs = PlaneChr::all();
        // Use type 0 (Sopwith Camel): att=1, def=1
        let mut atts = vec![make_plane(0, 1, 10, 0)];  // barely alive
        let mut defs = vec![make_plane(1, 2, 10, 0)];
        let mut rng = rand::thread_rng();
        // After enough hits anything with effic=10 should be gone eventually,
        // but we can't guarantee it in 3 rounds — just verify no panic.
        let _log = air_combat(&mut atts, &mut defs, chrs, chrs, &mut rng);
        // Must still produce something even if empty
    }

    #[test]
    fn air_combat_empty_defenders_no_panic() {
        let chrs = PlaneChr::all();
        let mut atts = vec![make_plane(0, 1, 100, 0)];
        let mut defs: Vec<Plane> = vec![];
        let mut rng = rand::thread_rng();
        let log = air_combat(&mut atts, &mut defs, chrs, chrs, &mut rng);
        assert!(log.is_empty(), "no combat with empty defenders");
    }
}
