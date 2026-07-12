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
// Ported from: src/lib/subs/plnsub.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Thomas Ruschak, 1992
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000
//    Markus Armbruster, 2006-2021

// Plane subsystem — capability checks, bomb effectiveness, fuel use.
// All functions are pure (no I/O, no DB).

use empire_types::plane::{Plane, PLANE_MIN_EFF};
use empire_types::plane_chr::PlaneChr;
use super::damage;

/// Minimum mobility required to fly a mission.
const MIN_PLANE_MOBIL: i8 = 0;

/// Return true if the plane is capable of flying a mission:
///   - effic >= PLANE_MIN_EFF (10%)
///   - mobil > MIN_PLANE_MOBIL
///   - not flagged off
pub fn pln_capable(plane: &Plane, _pchr: &PlaneChr) -> bool {
    plane.effic >= PLANE_MIN_EFF && plane.mobil > MIN_PLANE_MOBIL && !plane.off
}

/// Apply `dam` percentage damage to the plane.
/// Returns true if the plane is destroyed (effic <= 0).
///
/// Equivalent to `planedamage()` in damage.c.
pub fn pln_damage(plane: &mut Plane, dam: i32) -> bool {
    if dam <= 0 {
        return plane.effic <= 0;
    }
    let pct = damage::percent_damage(dam);
    let new_eff = damage::damage(plane.effic as i32, pct);
    plane.effic = new_eff as i8;
    plane.effic <= 0
}

/// Compute air-to-ground bombing effectiveness.
///
/// Formula (simplified from plnsub.c):
///   `eff = (pchr.load * plane.effic) / 100`
///
/// Higher load = more bombs; scaled by current plane efficiency.
pub fn pln_bomb_eff(plane: &Plane, pchr: &PlaneChr) -> i32 {
    if plane.effic < PLANE_MIN_EFF {
        return 0;
    }
    let base = (pchr.load * plane.effic as i32) / 100;
    base.max(0)
}

/// Compute intercept effectiveness for air-to-air combat.
///
/// Formula (simplified from plnsub.c):
///   `eff = (pchr.def * plane.effic) / 100`
pub fn pln_intercept_eff(plane: &Plane, pchr: &PlaneChr) -> i32 {
    if plane.effic < PLANE_MIN_EFF {
        return 0;
    }
    let base = (pchr.def * plane.effic as i32) / 100;
    base.max(0)
}

/// Deduct fuel (mobility) from a plane for a mission of `dist` sectors.
///
/// Cost = `pchr.fuel * dist` mobility points, minimum 1.
/// Clamps to 0 (cannot go negative).
pub fn pln_use_fuel(plane: &mut Plane, pchr: &PlaneChr, dist: i32) {
    let cost = (pchr.fuel * dist).max(1) as i8;
    plane.mobil = plane.mobil.saturating_sub(cost);
}

/// Return true if `plane` matches a plane-selector spec string. Shared by
/// `bomb`, `fly`, and `wingadd` so a wing letter works as a selector
/// everywhere planes are chosen, matching 4.4.1's <PLANE/WING> convention.
///
/// Accepted forms:
///   "*"        — every plane
///   "~"        — every plane with no wing assigned (the "null wing")
///   "c"        — every plane currently in wing 'c' (any single letter)
///   "5"        — plane uid 5
///   "0-5"      — plane uids 0 through 5
///   "2,14,23"  — plane uids 2, 14, and 23 (comma list; ranges allowed too)
pub fn plane_spec_matches(spec: &str, plane: &Plane) -> bool {
    let spec = spec.trim();
    if spec.is_empty() || spec == "*" {
        return true;
    }
    if spec == "~" {
        return plane.wing == ' ' || plane.wing == '\0';
    }
    if spec.len() == 1 {
        if let Some(c) = spec.chars().next() {
            if c.is_ascii_alphabetic() {
                return plane.wing == c;
            }
        }
    }
    for part in spec.split(',') {
        let part = part.trim().trim_start_matches('#');
        if let Ok(n) = part.parse::<i32>() {
            if n == plane.uid {
                return true;
            }
            continue;
        }
        if let Some((lo, hi)) = part.split_once('-') {
            if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<i32>(), hi.trim().parse::<i32>()) {
                if plane.uid >= lo && plane.uid <= hi {
                    return true;
                }
            }
        }
    }
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::plane::{Plane, PlaneFlags};
    use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};

    fn make_plane(effic: i8, mobil: i8) -> Plane {
        Plane {
            uid: 0, own: 1, x: 0, y: 0, plane_type: 9, // medium bomber
            effic, mobil, off: false, tech: 80,
            wing: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 14, harden: 0, ship: -1, land: -1,
            flags: PlaneFlags::empty(), access: 0, theta: 0.0,
        }
    }

    fn medium_bomber_chr() -> PlaneChr {
        PlaneChr {
            name: "medium bomber", sname: "mb",
            lcm: 14, hcm: 5, mil: 3, bwork: 44, tech: 80, cost: 1000,
            acc: 45, load: 4, att: 0, def: 5, range: 14, fuel: 3, stealth: 0,
            flags: PlaneChrFlags::from_bits_truncate(
                PlaneChrFlags::BOMBER.bits() | PlaneChrFlags::TACTICAL.bits()
            ),
        }
    }

    #[test]
    fn capable_ok() {
        let plane = make_plane(100, 30);
        let chr = medium_bomber_chr();
        assert!(pln_capable(&plane, &chr));
    }

    #[test]
    fn not_capable_no_mobil() {
        let plane = make_plane(100, 0);
        let chr = medium_bomber_chr();
        assert!(!pln_capable(&plane, &chr));
    }

    #[test]
    fn not_capable_low_effic() {
        let plane = make_plane(PLANE_MIN_EFF - 1, 30);
        let chr = medium_bomber_chr();
        assert!(!pln_capable(&plane, &chr));
    }

    #[test]
    fn bomb_eff_full() {
        let plane = make_plane(100, 30);
        let chr = medium_bomber_chr(); // load = 4
        assert_eq!(pln_bomb_eff(&plane, &chr), 4);
    }

    #[test]
    fn bomb_eff_half_effic() {
        let plane = make_plane(50, 30);
        let chr = medium_bomber_chr(); // load = 4
        assert_eq!(pln_bomb_eff(&plane, &chr), 2);
    }

    #[test]
    fn intercept_eff_computed() {
        let plane = make_plane(100, 30);
        let chr = medium_bomber_chr(); // def = 5
        assert_eq!(pln_intercept_eff(&plane, &chr), 5);
    }

    #[test]
    fn fuel_deducted() {
        let mut plane = make_plane(100, 30);
        let chr = medium_bomber_chr(); // fuel = 3
        pln_use_fuel(&mut plane, &chr, 2); // cost = 3*2 = 6
        assert_eq!(plane.mobil, 24);
    }

    #[test]
    fn damage_reduces_plane_effic() {
        let mut plane = make_plane(100, 30);
        let destroyed = pln_damage(&mut plane, 50);
        assert!(!destroyed, "plane should survive 50 damage from 100%");
        assert!(plane.effic < 100, "effic should decrease");
    }

    #[test]
    fn pln_damage_returns_true_when_already_zero() {
        let mut plane = make_plane(0, 30);
        let destroyed = pln_damage(&mut plane, 0);
        assert!(destroyed, "plane with effic=0 reports destroyed");
    }

    fn make_plane_uw(uid: i32, wing: char) -> Plane {
        let mut p = make_plane(100, 30);
        p.uid = uid;
        p.wing = wing;
        p
    }

    #[test]
    fn spec_star_matches_any_wing_or_uid() {
        assert!(plane_spec_matches("*", &make_plane_uw(42, 'c')));
        assert!(plane_spec_matches("*", &make_plane_uw(0, ' ')));
    }

    #[test]
    fn spec_tilde_matches_only_null_wing() {
        assert!(plane_spec_matches("~", &make_plane_uw(1, ' ')));
        assert!(!plane_spec_matches("~", &make_plane_uw(1, 'c')));
    }

    #[test]
    fn spec_letter_matches_wing_not_uid() {
        let plane = make_plane_uw(5, 'c');
        assert!(plane_spec_matches("c", &plane));
        assert!(!plane_spec_matches("d", &plane));
        // A bare letter is a wing selector, never falls back to uid parsing.
        let other = make_plane_uw(99, ' ');
        assert!(!plane_spec_matches("c", &other));
    }

    #[test]
    fn spec_uid_and_range_and_list() {
        assert!(plane_spec_matches("5", &make_plane_uw(5, ' ')));
        assert!(!plane_spec_matches("5", &make_plane_uw(6, ' ')));
        assert!(plane_spec_matches("0-5", &make_plane_uw(3, ' ')));
        assert!(!plane_spec_matches("0-5", &make_plane_uw(6, ' ')));
        assert!(plane_spec_matches("2,14,23", &make_plane_uw(14, ' ')));
        assert!(!plane_spec_matches("2,14,23", &make_plane_uw(15, ' ')));
    }
}
