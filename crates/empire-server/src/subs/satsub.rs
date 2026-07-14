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
// Ported from: src/lib/commands/laun.c, src/lib/commands/sate.c,
// src/lib/subs/satmap.c, src/lib/gen/round.c

// Satellite subsystem — orbit eligibility, launch chance rolls, report
// range, and the deterministic "noise" pattern used to fuzz spy reports
// below 100% efficiency. All functions are pure (no I/O, no DB).

use empire_types::plane::{Plane, PlaneFlags};
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use crate::subs::tech::techfact;

/// Return true if `plane` is a satellite type currently in orbit.
/// Mirrors `pln_is_in_orbit()`: has SATELLITE capability, not a missile,
/// and has actually been launched.
pub fn sat_is_in_orbit(plane: &Plane, chr: &PlaneChr) -> bool {
    chr.flags.contains(PlaneChrFlags::SATELLITE)
        && !chr.flags.contains(PlaneChrFlags::MISSILE)
        && plane.flags.contains(PlaneFlags::LAUNCHED)
}

/// Return true if an orbiting satellite has regained enough mobility to
/// be used (`pln_mobil >= plane_mob_max`).
pub fn sat_is_ready(plane: &Plane, plane_mob_max: i32) -> bool {
    plane.mobil as i32 >= plane_mob_max
}

/// Satellite report range: `techfact(20, tech) * effic/100`.
pub fn sat_range(tech: f64, effic: i8) -> i32 {
    let base = techfact(20.0, tech);
    (base * effic as f64 / 100.0) as i32
}

/// Booster failure chance on launch: `0.07 + (100-effic)/100`.
pub fn sat_launch_failure_chance(effic: i8) -> f64 {
    0.07 + (100 - effic as i32) as f64 / 100.0
}

/// Trajectory drift chance: `1 - i/(i+50)` where `i = tech + effic`.
/// Lower tech/efficiency drifts more often.
pub fn sat_drift_chance(tech: f64, effic: i8) -> f64 {
    let i = tech + effic as f64;
    1.0 - i / (i + 50.0)
}

/// Round `n` to the nearest multiple of `m` (half rounds up).
/// Mirrors `roundintby()`: `(n + m/2) / m * m` using integer division.
pub fn round_int_by(n: i32, m: i32) -> i32 {
    if m <= 0 { return n; }
    (n + m / 2) / m * m
}

/// Return true if the given rotating 0..100 `counter` value falls in a
/// "noisy" (skip this contact) slot for a satellite at `effic`.
///
/// Mirrors satmap.c's precomputed `noise[100*n/(100-eff)] = 1` array for
/// `n in 0..(100-eff)`, without needing to materialize the array: slot
/// `counter` is noisy iff some `n` maps to it, i.e.
/// `exists n in 0..(100-eff): 100*n/(100-eff) == counter`.
pub fn is_noisy_slot(counter: usize, effic: i8) -> bool {
    let gap = 100 - effic as i32;
    if gap <= 0 { return false; }
    for n in 0..gap {
        if (100 * n / gap) as usize == counter {
            return true;
        }
    }
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plane(effic: i8, mobil: i8, launched: bool) -> Plane {
        Plane {
            uid: 0, own: 1, x: 0, y: 0, plane_type: 24, // landsat
            effic, mobil, off: false, tech: 245,
            wing: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 255, harden: 0, ship: -1, land: -1,
            flags: if launched { PlaneFlags::LAUNCHED } else { PlaneFlags::empty() },
            access: 0, theta: 0.0,
        }
    }

    fn landsat_chr() -> PlaneChr {
        PlaneChr {
            name: "landsat", sname: "lst",
            lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 245, cost: 2000,
            acc: 0, load: 0, att: 0, def: 0, range: 255, fuel: 0, stealth: 0,
            flags: PlaneChrFlags::SATELLITE.union(PlaneChrFlags::IMAGE),
        }
    }

    fn spysat_chr() -> PlaneChr {
        PlaneChr {
            name: "spysat", sname: "ss",
            lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 305, cost: 4000,
            acc: 0, load: 0, att: 0, def: 0, range: 255, fuel: 0, stealth: 0,
            flags: PlaneChrFlags::SATELLITE.union(PlaneChrFlags::SPY),
        }
    }

    fn missile_chr() -> PlaneChr {
        PlaneChr {
            name: "ICBM", sname: "icbm",
            lcm: 20, hcm: 20, mil: 0, bwork: 80, tech: 310, cost: 3000,
            acc: 0, load: 0, att: 0, def: 0, range: 255, fuel: 0, stealth: 0,
            flags: PlaneChrFlags::MISSILE,
        }
    }

    #[test]
    fn in_orbit_requires_launched_flag() {
        let chr = landsat_chr();
        assert!(!sat_is_in_orbit(&make_plane(100, 0, false), &chr));
        assert!(sat_is_in_orbit(&make_plane(100, 0, true), &chr));
    }

    #[test]
    fn in_orbit_false_for_missiles() {
        let chr = missile_chr();
        assert!(!sat_is_in_orbit(&make_plane(100, 0, true), &chr));
    }

    #[test]
    fn spysat_flags_distinct_from_landsat() {
        assert!(spysat_chr().flags.contains(PlaneChrFlags::SPY));
        assert!(!spysat_chr().flags.contains(PlaneChrFlags::IMAGE));
        assert!(landsat_chr().flags.contains(PlaneChrFlags::IMAGE));
        assert!(!landsat_chr().flags.contains(PlaneChrFlags::SPY));
    }

    #[test]
    fn ready_requires_full_mobility() {
        assert!(!sat_is_ready(&make_plane(100, 100, true), 127));
        assert!(sat_is_ready(&make_plane(100, 127, true), 127));
    }

    #[test]
    fn range_scales_with_effic_and_tech() {
        let full = sat_range(245.0, 100);
        let half = sat_range(245.0, 50);
        assert!(full > half);
        assert!(full > 0);
    }

    #[test]
    fn launch_failure_chance_higher_at_low_effic() {
        assert!(sat_launch_failure_chance(50) > sat_launch_failure_chance(100));
        assert!((sat_launch_failure_chance(100) - 0.07).abs() < 1e-9);
    }

    #[test]
    fn drift_chance_lower_at_high_tech_and_effic() {
        assert!(sat_drift_chance(300.0, 100) < sat_drift_chance(0.0, 40));
    }

    #[test]
    fn round_int_by_nearest_multiple() {
        assert_eq!(round_int_by(47, 10), 50);
        assert_eq!(round_int_by(44, 10), 40);
        assert_eq!(round_int_by(83, 50), 100);
        assert_eq!(round_int_by(0, 50), 0);
    }

    #[test]
    fn no_noise_at_full_efficiency() {
        for c in 0..100 {
            assert!(!is_noisy_slot(c, 100));
        }
    }

    #[test]
    fn heavy_noise_at_low_efficiency() {
        // effic=1 -> gap=99, nearly every slot is noisy
        let noisy_count = (0..100).filter(|&c| is_noisy_slot(c, 1)).count();
        assert!(noisy_count >= 90, "expected heavy noise, got {noisy_count}/100");
    }

    #[test]
    fn partial_noise_at_mid_efficiency() {
        // effic=50 -> gap=50, roughly half the slots noisy
        let noisy_count = (0..100).filter(|&c| is_noisy_slot(c, 50)).count();
        assert!(noisy_count > 30 && noisy_count < 70, "got {noisy_count}/100");
    }
}
