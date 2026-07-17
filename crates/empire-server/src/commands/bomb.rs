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
// Ported from: src/lib/commands/bomb.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000

// "bomb" command — strategic and tactical bombing mission.
//
// Usage: bomb PLANE-SPEC TARGET-SECT [COMMODITY]
//        bomb PLANE-SPEC TARGET-SECT ship SHIP-UID
//
// PLANE-SPEC: "*" for all owned planes, a single uid, a uid range
// ("0-5"), a comma list, "~" for planes with no wing, or a single
// letter naming a wing (see 'info wingadd').
// TARGET-SECT: destination sector (player-relative "X,Y").
// COMMODITY: optional commodity letter to target (default: 'i' = sector efficiency).
// SHIP-UID: pin-point bomb a specific ship sitting in TARGET-SECT instead
//   of the sector itself — see 'info bomb' for how to find an enemy
//   ship's uid ('satellite'/'recon' with a SPY-flagged plane).

use empire_db::{news, planes, sectors, ships};
use empire_types::news::NewsVerb;
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::sector::SectorType;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::aircombat::{air_combat, find_interceptors};
use crate::subs::plnsub::{pln_capable, pln_bomb_eff, pln_use_fuel, plane_spec_matches};
use crate::subs::damage::damage;
use crate::subs::shpsub::shp_damage;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: bomb PLANE-SPEC TARGET-SECT [COMMODITY]\n".to_string();
    }

    let plane_spec = parts[0];
    let Some((rx, ry)) = parse_rel_xy(parts[1]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[1]);
    };
    let tx = ctx.x_abs(rx);
    let ty = ctx.y_abs(ry);

    // "ship SHIP-UID" targets a specific ship sitting in the target
    // sector instead of the sector itself — everything else (plane
    // selection, interception) is shared with sector bombing.
    let ship_target: Option<i32> = if parts.get(2).map(|s| s.eq_ignore_ascii_case("ship")).unwrap_or(false) {
        match parts.get(3).and_then(|s| s.parse::<i32>().ok()) {
            Some(uid) => Some(uid),
            None => return "10 Usage: bomb PLANE-SPEC TARGET-SECT ship SHIP-UID\n".to_string(),
        }
    } else {
        None
    };
    let _commodity = parts.get(2).copied().unwrap_or("i");

    // Load target sector
    let mut target_sector = match sectors::get_at(ctx.db, tx, ty).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(tx, ty)),
        Err(e)   => return format!("10 DB error: {e}\n"),
    };

    // Cannot bomb sea sectors (nothing there) — unless pin-pointing a
    // ship, which is exactly where ships actually sit.
    if target_sector.sector_type == SectorType::Sea && ship_target.is_none() {
        return format!(
            "10 {} is a sea sector — nothing to bomb.\n",
            ctx.format_xy(tx, ty)
        );
    }

    // Resolve and validate the ship target up front, before spending any
    // planes' fuel/mobility on an interception pass that would go to waste.
    let target_ship = if let Some(uid) = ship_target {
        let ship = match ships::get(ctx.db, uid).await {
            Ok(Some(s)) => s,
            Ok(None) => return format!("10 No such ship #{uid}\n"),
            Err(e) => return format!("10 DB error: {e}\n"),
        };
        if ship.own == 0 || ship.x != tx || ship.y != ty {
            return format!(
                "10 Ship #{uid} not spotted at {}\n",
                ctx.format_xy(tx, ty)
            );
        }
        Some(ship)
    } else {
        None
    };

    // Load all planes owned by the player
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let chrs = PlaneChr::all();

    // Select attacking planes matching the spec
    let mut att_planes: Vec<_> = all_planes
        .into_iter()
        .filter(|p| {
            p.own == ctx.cnum
                && plane_spec_matches(plane_spec, p)
        })
        .filter(|p| {
            let Some(chr) = chrs.get(p.plane_type as usize) else { return false; };
            pln_capable(p, chr)
        })
        .collect();

    if att_planes.is_empty() {
        return "10 No capable planes match that specification.\n".to_string();
    }

    // A submarine can only be pin-pointed by ASW-capable planes — matches
    // 4.4.1's ship_bomb(), which searches for subs via asw_shipsatxy()
    // only when the flight has P_A capability, and otherwise can't find
    // them at all.
    if let Some(ship) = &target_ship {
        let is_sub = ShipChr::for_type(ship.ship_type as usize)
            .map(|c| c.flags.contains(ShipChrFlags::SUBMARINE))
            .unwrap_or(false);
        if is_sub && !att_planes.iter().any(|p| {
            chrs.get(p.plane_type as usize)
                .map(|c| c.flags.contains(PlaneChrFlags::ASW))
                .unwrap_or(false)
        }) {
            return format!(
                "10 Ship #{} is a submarine — none of these planes are ASW-capable\n",
                ship.uid
            );
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "1 Bombing mission to {}\n",
        ctx.format_xy(tx, ty),
    ));
    out.push_str(&format!("1 {} plane(s) airborne\n", att_planes.len()));

    // Find interceptors at target
    let mut interceptors = match find_interceptors(
        ctx.db, tx, ty, ctx.cnum, ctx.world_x, ctx.world_y,
    ).await {
        Ok(v) => v,
        Err(e) => {
            out.push_str(&format!("1 Warning: could not load interceptors: {e}\n"));
            vec![]
        }
    };

    // Air combat if interceptors present
    if !interceptors.is_empty() {
        out.push_str(&format!("1 Air combat: {} interceptor(s) scramble\n", interceptors.len()));
        let mut rng = StdRng::from_entropy();
        let int_chrs = PlaneChr::all();
        let combat_log = air_combat(&mut att_planes, &mut interceptors, chrs, int_chrs, &mut rng);
        for line in combat_log {
            out.push_str(&format!("1 {line}\n"));
        }

        let lost_att = att_planes.iter().filter(|p| p.effic <= 0).count();
        let lost_def = interceptors.iter().filter(|p| p.effic <= 0).count();
        out.push_str(&format!(
            "1 Air combat result: {} attacker(s) lost, {} interceptor(s) lost\n",
            lost_att, lost_def,
        ));

        // Save interceptors (some may be destroyed)
        for plane in &interceptors {
            let _ = planes::put(ctx.db, plane).await;
        }
    }

    // Keep only surviving attackers
    att_planes.retain(|p| p.effic > 0);

    if att_planes.is_empty() {
        out.push_str("1 All attacking planes destroyed — bombing aborted.\n");
        out.push_str("0 bomb\n");
        return out;
    }

    out.push_str(&format!("1 {} plane(s) reach target\n", att_planes.len()));

    // Compute bombing damage — sum of all plane bomb loads
    // Use SmallRng (Send-safe) to avoid crossing await points with ThreadRng
    let mut rng = StdRng::from_entropy();
    let total_bomb_eff: i32 = att_planes.iter().map(|p| {
        let Some(chr) = chrs.get(p.plane_type as usize) else { return 0; };
        pln_bomb_eff(p, chr)
    }).sum();

    // Add randomness: damage = total_bomb_eff * random(0.5..1.5)
    let mult = 0.5 + rng.gen::<f64>();

    if let Some(mut ship) = target_ship {
        // Pin-point bombing a ship is twice as accurate as area bombing a
        // sector — matches 4.4.1's `dam = 2 * pln_damage(...)` in ship_bomb().
        let raw_damage = (total_bomb_eff as f64 * mult * 2.0) as i32;
        let dam = raw_damage.clamp(1, 100);

        let armor = ShipChr::for_type(ship.ship_type as usize).map(|c| c.armor).unwrap_or(0);
        let old_effic = ship.effic;
        let sunk = shp_damage(&mut ship, dam, armor);

        out.push_str(&format!(
            "1 Bombing: {dam} damage to ship #{} (effic {old_effic}% -> {}%)\n",
            ship.uid, ship.effic
        ));
        if sunk {
            out.push_str(&format!("1 Ship #{} sinks!\n", ship.uid));
        }

        // File a news item — mirrors nreport(player->cnum, N_SHP_BOMB / N_SUB_BOMB, ...)
        let is_sub = ShipChr::for_type(ship.ship_type as usize)
            .map(|c| c.flags.contains(ShipChrFlags::SUBMARINE))
            .unwrap_or(false);
        let verb = if is_sub { NewsVerb::SubBomb } else { NewsVerb::ShpBomb };
        if ship.own != ctx.cnum {
            let _ = news::add_news(ctx.db, ctx.cnum, verb as u8, ship.own, 1).await;
        }

        if let Err(e) = ships::put(ctx.db, &ship).await {
            out.push_str(&format!("1 Warning: error saving ship: {e}\n"));
        }
    } else {
        let raw_damage = (total_bomb_eff as f64 * mult) as i32;
        let bomb_pct = raw_damage.clamp(1, 100);

        // Apply to sector efficiency
        let old_effic = target_sector.effic as i32;
        let new_effic = damage(old_effic, bomb_pct) as i8;
        target_sector.effic = new_effic;

        out.push_str(&format!(
            "1 Bombing: {bomb_pct}% damage to sector (effic {old_effic}% → {new_effic}%)\n"
        ));

        // File a news item — mirrors nreport(player->cnum, N_SCT_BOMB, target.sct_own, 1)
        let _ = news::add_news(ctx.db, ctx.cnum, NewsVerb::SctBomb as u8, target_sector.own, 1).await;

        // Save sector
        if let Err(e) = sectors::put(ctx.db, &target_sector).await {
            out.push_str(&format!("1 Warning: error saving sector: {e}\n"));
        }
    }

    // Deduct fuel from attacking planes and save
    for mut plane in att_planes {
        let Some(chr) = chrs.get(plane.plane_type as usize) else { continue; };
        // Rough distance: use 1 for simplicity (full round-trip cost handled in fly)
        pln_use_fuel(&mut plane, chr, 1);
        let _ = planes::put(ctx.db, &plane).await;
    }

    out.push_str("0 bomb\n");
    out
}
