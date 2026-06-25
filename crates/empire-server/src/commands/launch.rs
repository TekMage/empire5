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
// Ported from: src/lib/commands/lnch.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000

// "launch" / "lnch" command — launch missile/ICBM at a target sector.
//
// Usage: launch PLANE-SPEC TARGET-SECT
//
// Only planes with the MISSILE flag in their PlaneChr can be launched.
// Missiles are single-use: they are destroyed after launch (effic set to 0).
// Damage: sector effic reduced by (plane.effic * 2)% (capped at 100).

use empire_db::{planes, sectors};
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::geo::map_dist;
use crate::subs::damage::damage;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: launch PLANE-SPEC TARGET-SECT\n".to_string();
    }

    let plane_spec = parts[0];
    let Some((rx, ry)) = parse_rel_xy(parts[1]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[1]);
    };
    let tx = ctx.x_abs(rx);
    let ty = ctx.y_abs(ry);

    // Load target sector
    let mut target = match sectors::get_at(ctx.db, tx, ty).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(tx, ty)),
        Err(e)   => return format!("10 DB error: {e}\n"),
    };

    // Load all planes
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let chrs = PlaneChr::all();

    // Select matching missiles (player-owned, MISSILE flag, alive)
    let candidates: Vec<_> = all_planes
        .into_iter()
        .filter(|p| {
            p.own == ctx.cnum
                && p.effic > 0
                && plane_spec_matches(plane_spec, p.uid)
        })
        .filter(|p| {
            chrs.get(p.plane_type as usize)
                .map(|c| c.flags.contains(PlaneChrFlags::MISSILE))
                .unwrap_or(false)
        })
        .collect();

    if candidates.is_empty() {
        return "10 No missiles match that specification.\n".to_string();
    }

    let mut out = String::new();
    let mut hit_count = 0u32;

    for mut missile in candidates {
        let Some(chr) = chrs.get(missile.plane_type as usize) else { continue; };

        // Range check
        let dist = map_dist(missile.x, missile.y, tx, ty, ctx.world_x, ctx.world_y);
        let max_range = chr.range as i32;
        if dist > max_range {
            out.push_str(&format!(
                "1 Missile #{}: target out of range (dist {dist}, range {max_range})\n",
                missile.uid,
            ));
            continue;
        }

        // Compute damage: effic * 2, capped at 100
        let dam_pct = (missile.effic as i32 * 2).min(100);
        let old_effic = target.effic as i32;
        let new_effic = damage(old_effic, dam_pct) as i8;

        out.push_str(&format!(
            "1 Launching missile #{} at {}\n",
            missile.uid,
            ctx.format_xy(tx, ty),
        ));
        out.push_str(&format!(
            "1 Hit! {dam_pct}% damage — sector effic {old_effic}% → {new_effic}%\n"
        ));

        target.effic = new_effic;
        hit_count += 1;

        // Destroy the missile (one-use)
        missile.effic = 0;
        if let Err(e) = planes::put(ctx.db, &missile).await {
            out.push_str(&format!("1 Warning: missile save error: {e}\n"));
        }
    }

    if hit_count == 0 {
        out.push_str("1 No missiles could reach the target.\n");
        out.push_str("0 launch\n");
        return out;
    }

    // Save sector with accumulated damage
    if let Err(e) = sectors::put(ctx.db, &target).await {
        out.push_str(&format!("1 Warning: error saving target sector: {e}\n"));
    }

    out.push_str("0 launch\n");
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
