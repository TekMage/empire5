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
//
// PLANE-SPEC: "*" for all owned planes, single uid, or comma-separated uids.
// TARGET-SECT: destination sector (player-relative "X,Y").
// COMMODITY: optional commodity letter to target (default: 'i' = sector efficiency).

use empire_db::{planes, sectors};
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::SectorType;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::aircombat::{air_combat, find_interceptors};
use crate::subs::plnsub::{pln_capable, pln_bomb_eff, pln_use_fuel};
use crate::subs::damage::damage;

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
    let _commodity = parts.get(2).copied().unwrap_or("i");

    // Load target sector
    let mut target_sector = match sectors::get_at(ctx.db, tx, ty).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(tx, ty)),
        Err(e)   => return format!("10 DB error: {e}\n"),
    };

    // Cannot bomb sea sectors (nothing there)
    if target_sector.sector_type == SectorType::Sea {
        return format!(
            "10 {} is a sea sector — nothing to bomb.\n",
            ctx.format_xy(tx, ty)
        );
    }

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
                && plane_spec_matches(plane_spec, p.uid)
        })
        .filter(|p| {
            let Some(chr) = chrs.get(p.plane_type as usize) else { return false; };
            pln_capable(p, chr)
        })
        .collect();

    if att_planes.is_empty() {
        return "10 No capable planes match that specification.\n".to_string();
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
    let raw_damage = (total_bomb_eff as f64 * mult) as i32;
    let bomb_pct = raw_damage.clamp(1, 100);

    // Apply to sector efficiency
    let old_effic = target_sector.effic as i32;
    let new_effic = damage(old_effic, bomb_pct) as i8;
    target_sector.effic = new_effic;

    out.push_str(&format!(
        "1 Bombing: {bomb_pct}% damage to sector (effic {old_effic}% → {new_effic}%)\n"
    ));

    // Save sector
    if let Err(e) = sectors::put(ctx.db, &target_sector).await {
        out.push_str(&format!("1 Warning: error saving sector: {e}\n"));
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

/// Return true if `uid` matches the plane spec string.
fn plane_spec_matches(spec: &str, uid: i32) -> bool {
    if spec == "*" {
        return true;
    }
    for part in spec.split(',') {
        let part = part.trim().trim_start_matches('#');
        if let Ok(n) = part.parse::<i32>() {
            if n == uid {
                return true;
            }
        }
    }
    false
}
