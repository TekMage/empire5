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
// Ported from: src/lib/subs/attsub.c
// Known contributors to the original:
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2006-2021

// Ground combat resolution — the core attack algorithm.
// Pure functions; callers supply pre-loaded data and persist changes.

use empire_types::sector::{Sector, SectorType};
use empire_types::land::LandUnit;
use empire_types::land_chr::LandChr;
use empire_types::nation::NatStatus;
use empire_db::relations::Relation;

use rand::Rng;

/// Result of a ground combat engagement.
#[derive(Debug)]
pub struct AttackResult {
    /// True if the attacker captured the sector.
    pub attacker_wins: bool,
    /// Military casualties taken by the attacker.
    pub att_casualties: i32,
    /// Military casualties taken by the defender.
    pub def_casualties: i32,
    /// True if the sector ownership has changed.
    pub sector_taken: bool,
    /// Human-readable combat log for the issuing player.
    pub log: Vec<String>,
}

/// Multiplier applied to defender strength when in a fort sector.
const FORT_BONUS: f64 = 1.5;

/// Fraction of strength converted to casualties per round (random range).
const CASUALTY_LO: f64 = 0.1;
const CASUALTY_HI: f64 = 0.3;

/// Attacker must exceed this multiple of defender strength to win.
const WIN_RATIO: f64 = 1.5;

/// Resolve a ground attack.
///
/// # Parameters
///
/// * `attackers` — attacking land units.
/// * `att_lchr` — land characteristic table (use `LandChr::all()`).
/// * `att_mil` — extra military committed to the attack (from sector items).
/// * `defender` — the target sector.
/// * `def_units` — defending land units in the target sector.
/// * `def_lchr` — land characteristic table.
/// * `def_mil` — defending military in the sector.
/// * `tech_att` — attacker tech factor (`1.0 + tech / 100.0`).
/// * `tech_def` — defender tech factor.
/// * `rng` — random number generator.
///
/// # Algorithm (simplified from attsub.c)
///
/// 1. Compute attacker strength: mil * tech_att + sum(unit.effic * lchr.att).
/// 2. Compute defender strength: mil * tech_def * (sector.effic / 100) + unit contributions.
/// 3. Apply fort bonus to defender if sector is `Fort`.
/// 4. Single combat round: both sides take casualties proportional to enemy strength.
/// 5. Attacker wins if att_str > def_str * WIN_RATIO after round.
pub fn att_resolve(
    attackers: &[LandUnit],
    att_lchr: &[LandChr],
    att_mil: i32,
    defender: &Sector,
    def_units: &[LandUnit],
    def_lchr: &[LandChr],
    def_mil: i32,
    tech_att: f64,
    tech_def: f64,
    rng: &mut impl Rng,
) -> AttackResult {
    let mut log = Vec::new();

    // 1. Attacker strength
    let unit_att: f64 = attackers.iter().map(|u| {
        let idx = u.land_type as usize;
        let chr_att = att_lchr.get(idx).map(|c| c.att as f64).unwrap_or(1.0);
        u.effic as f64 * chr_att
    }).sum();
    let att_str = att_mil as f64 * tech_att + unit_att;

    // 2. Defender strength — scaled by sector efficiency
    let sector_eff_factor = defender.effic as f64 / 100.0;
    let unit_def: f64 = def_units.iter().map(|u| {
        let idx = u.land_type as usize;
        let chr_def = def_lchr.get(idx).map(|c| c.def as f64).unwrap_or(1.0);
        u.effic as f64 * chr_def
    }).sum();
    let mut def_str = def_mil as f64 * tech_def * sector_eff_factor + unit_def;

    // 3. Fort bonus
    if defender.sector_type == SectorType::Fort {
        def_str *= FORT_BONUS;
        log.push("Defender has fort bonus (x1.5).".to_string());
    }

    log.push(format!(
        "Attack: Att strength {:.1} vs Def strength {:.1}",
        att_str, def_str
    ));

    // 4. Combat round — casualties
    let roll_att: f64 = rng.gen_range(CASUALTY_LO..=CASUALTY_HI);
    let roll_def: f64 = rng.gen_range(CASUALTY_LO..=CASUALTY_HI);

    let att_casualties = (def_str * roll_att) as i32;
    let def_casualties = (att_str * roll_def) as i32;

    log.push(format!(
        "Casualties: Att {att_casualties}, Def {def_casualties}"
    ));

    // 5. Outcome: attacker needs significantly more strength to take sector
    let remaining_att = (att_str - att_casualties as f64).max(0.0);
    let remaining_def = (def_str - def_casualties as f64).max(0.0);

    let attacker_wins = remaining_att > remaining_def * WIN_RATIO;
    let sector_taken  = attacker_wins;

    if attacker_wins {
        log.push("Attacker wins!".to_string());
    } else {
        log.push("Defender holds!".to_string());
    }

    AttackResult {
        attacker_wins,
        att_casualties,
        def_casualties,
        sector_taken,
        log,
    }
}

/// Return true if `att` is at war with `def`.
///
/// War means the attacker's relation toward the defender is `AtWar`.
/// Deities can always attack anyone.
///
/// `att_status` — the NatStatus of the attacking nation.
/// `relation` — att's current diplomatic stance toward def.
pub fn at_war(att_status: NatStatus, relation: Relation) -> bool {
    att_status == NatStatus::Deity || relation == Relation::AtWar
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::sector::{Sector, SectorType, DistEntry};
    use empire_types::commodity::Inventory;
    use rand::SeedableRng;

    fn make_sector(sector_type: SectorType, effic: i8) -> Sector {
        Sector {
            uid: 0, own: 2, x: 0, y: 0,
            sector_type, effic, mobil: 0,
            off: false, loyal: 50, terr: [0; 4], dterr: 0,
            dist_x: 0, dist_y: 0, avail: 0, flags: 0, elev: 0,
            work: 100, coastal: false, new_type: sector_type,
            min: 0, gmin: 0, fertil: 0, oil: 0, uran: 0,
            old_own: 2, che: 0, che_target: 0,
            items: Inventory::zero(),
            del: [DistEntry::default(); 26],
            mines: 0, pstage: 0, ptime: 0, fallout: 0,
        }
    }

    #[test]
    fn overwhelming_attack_wins() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let defender = make_sector(SectorType::Urban, 100);
        // Attacker: 5000 mil, tech_att 2.0 → str 10_000
        // Defender: 100 mil, tech_def 1.0 → str 100
        let result = att_resolve(
            &[], LandChr::all(),
            5000, &defender, &[], LandChr::all(),
            100, 2.0, 1.0, &mut rng,
        );
        assert!(result.attacker_wins, "5000 mil should overwhelm 100 mil");
        assert!(result.sector_taken);
    }

    #[test]
    fn weak_attack_fails() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let defender = make_sector(SectorType::Urban, 100);
        // Attacker: 10 mil vs Defender: 1000 mil
        let result = att_resolve(
            &[], LandChr::all(),
            10, &defender, &[], LandChr::all(),
            1000, 1.0, 1.0, &mut rng,
        );
        assert!(!result.attacker_wins, "10 mil cannot beat 1000 mil");
    }

    #[test]
    fn at_war_active_at_war() {
        assert!(at_war(NatStatus::Active, Relation::AtWar));
    }

    #[test]
    fn at_war_deity_always() {
        assert!(at_war(NatStatus::Deity, Relation::Neutral));
    }

    #[test]
    fn at_war_neutral_blocked() {
        assert!(!at_war(NatStatus::Active, Relation::Neutral));
    }

    #[test]
    fn fort_bonus_in_log() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let defender = make_sector(SectorType::Fort, 100);
        let result = att_resolve(
            &[], LandChr::all(),
            10, &defender, &[], LandChr::all(),
            100, 1.0, 1.0, &mut rng,
        );
        assert!(result.log.iter().any(|l| l.contains("fort bonus")));
    }
}
