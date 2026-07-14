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
// Ported from: src/lib/commands/laun.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995
//    Steve McClure, 1996-2000

// "launch" / "lnch" command — launch a missile or satellite at a target
// sector.
//
// Usage: launch PLANE-SPEC TARGET-SECT [y|n]
//
// Missiles (MISSILE flag): single-use, destroyed after launch. Damage:
// sector effic reduced by (plane.effic * 2)% (capped at 100).
//
// Satellites (SATELLITE flag, not MISSILE): the 3rd arg is required —
// "y" for geostationary orbit (stays put), "n" for a moving orbit.
// Launch can fail outright (booster failure) or drift one hex off
// target, both effic/tech-dependent (see subs::satsub). A successful
// launch positions the plane over the target and marks it in orbit; it
// becomes usable once its mobility fully regenerates (see 'satellite').
//
// PLANE-SPEC accepts a uid, a uid range, a comma list, "*", "~" (planes
// with no wing assigned), or a single letter naming a wing.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use empire_db::{news, planes, sectors};
use empire_types::news::NewsVerb;
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::plane::PlaneFlags;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::geo::{map_dist, x_norm, y_norm, DIROFF, DIR_FIRST, DIR_LAST};
use crate::subs::damage::damage;
use crate::subs::plnsub::plane_spec_matches;
use crate::subs::satsub::{sat_drift_chance, sat_is_in_orbit, sat_launch_failure_chance};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: launch PLANE-SPEC TARGET-SECT [y|n]\n".to_string();
    }

    let plane_spec = parts[0];
    let Some((rx, ry)) = parse_rel_xy(parts[1]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[1]);
    };
    let tx = ctx.x_abs(rx);
    let ty = ctx.y_abs(ry);
    let geostationary_arg = parts.get(2).copied();

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

    let candidates: Vec<_> = all_planes
        .into_iter()
        .filter(|p| {
            p.own == ctx.cnum
                && p.effic > 0
                && plane_spec_matches(plane_spec, p)
        })
        .collect();

    if candidates.is_empty() {
        return "10 No planes match that specification.\n".to_string();
    }

    let mut out = String::new();
    let mut rng = StdRng::from_entropy();
    let mut missile_attempts = 0u32;
    let mut hit_count = 0u32;
    let mut sector_touched = false;

    for missile in candidates {
        let Some(chr) = chrs.get(missile.plane_type as usize) else { continue };

        if chr.flags.contains(PlaneChrFlags::MISSILE) {
            missile_attempts += 1;
            let (touched, hit) = launch_missile(missile, chr, tx, ty, &mut target, ctx, &mut out).await;
            sector_touched |= touched;
            if hit { hit_count += 1; }
        } else if chr.flags.contains(PlaneChrFlags::SATELLITE) {
            launch_satellite(
                missile, chr, tx, ty, geostationary_arg, ctx, &mut rng, &mut out,
            ).await;
        } else {
            out.push_str(&format!("1 Plane #{}: isn't a missile or satellite!\n", missile.uid));
        }
    }

    if missile_attempts > 0 && hit_count == 0 {
        out.push_str("1 No missiles could reach the target.\n");
    }

    if sector_touched {
        if let Err(e) = sectors::put(ctx.db, &target).await {
            out.push_str(&format!("1 Warning: error saving target sector: {e}\n"));
        }
    }

    out.push_str("0 launch\n");
    out
}

/// Existing (simplified) missile-launch path. Returns (sector_touched, hit).
async fn launch_missile(
    mut missile: empire_types::plane::Plane,
    chr: &PlaneChr,
    tx: i16, ty: i16,
    target: &mut empire_types::sector::Sector,
    ctx: &CmdCtx<'_>,
    out: &mut String,
) -> (bool, bool) {
    let dist = map_dist(missile.x, missile.y, tx, ty, ctx.world_x, ctx.world_y);
    let max_range = chr.range;
    if dist > max_range {
        out.push_str(&format!(
            "1 Missile #{}: target out of range (dist {dist}, range {max_range})\n",
            missile.uid,
        ));
        return (false, false);
    }

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

    // Destroy the missile (one-use)
    missile.effic = 0;
    if let Err(e) = planes::put(ctx.db, &missile).await {
        out.push_str(&format!("1 Warning: missile save error: {e}\n"));
    }
    (true, true)
}

/// New satellite-placement path. Mirrors launch_sat() in laun.c.
#[allow(clippy::too_many_arguments)]
async fn launch_satellite(
    mut plane: empire_types::plane::Plane,
    chr: &PlaneChr,
    tx: i16, ty: i16,
    geostationary_arg: Option<&str>,
    ctx: &CmdCtx<'_>,
    rng: &mut StdRng,
    out: &mut String,
) {
    if sat_is_in_orbit(&plane, chr) {
        out.push_str(&format!("1 Plane #{}: already in orbit!\n", plane.uid));
        return;
    }
    if plane.effic < 40 {
        out.push_str(&format!(
            "1 Plane #{}: is damaged ({}%)\n", plane.uid, plane.effic
        ));
        return;
    }
    let Some(geo) = geostationary_arg else {
        out.push_str("1 Satellite launch requires a geostationary orbit? argument (y/n)\n");
        return;
    };
    let synchronous = matches!(geo, "y" | "Y" | "yes");

    let dist = map_dist(plane.x, plane.y, tx, ty, ctx.world_x, ctx.world_y);
    if dist > plane.range as i32 {
        out.push_str(&format!(
            "1 Plane #{}: range too great (dist {dist}, range {})\n", plane.uid, plane.range
        ));
        return;
    }

    out.push_str(&format!(
        "1 {} at {}; range {}, eff {}%\n",
        chr.name, ctx.format_xy(plane.x, plane.y), plane.range, plane.effic
    ));
    out.push_str("1 3... 2... 1... Blastoff!!!\n");

    if rng.gen::<f64>() < sat_launch_failure_chance(plane.effic) {
        out.push_str("1 KABOOOOM!  Range safety officer detonates booster!\n");
        plane.effic = 0;
        let _ = planes::put(ctx.db, &plane).await;
        return;
    }

    let mut sx = tx;
    let mut sy = ty;
    if rng.gen::<f64>() < sat_drift_chance(plane.tech as f64, plane.effic) {
        let dir = rng.gen_range(DIR_FIRST..=DIR_LAST);
        let (dx, dy) = DIROFF[dir];
        sx = x_norm(tx + dx, ctx.world_x);
        sy = y_norm(ty + dy, ctx.world_y);
        out.push_str("1 Your trajectory was a little off.\n");
    }

    // Broadcast news of the launch — no specific victim, always filed
    // (unlike most news calls this session, victim=0 is intentional here).
    let _ = news::add_news(ctx.db, ctx.cnum, NewsVerb::Launch as u8, 0, 1).await;

    plane.x = sx;
    plane.y = sy;
    plane.flags.insert(PlaneFlags::LAUNCHED);
    if synchronous {
        plane.flags.insert(PlaneFlags::SYNCHRONOUS);
    } else {
        plane.flags.remove(PlaneFlags::SYNCHRONOUS);
    }
    plane.mobil = (plane.mobil as i32 - dist).max(0) as i8;
    plane.ship = -1;
    plane.land = -1;

    let plane_mob_max = ctx.config.rates.plane_mob_max;
    let ready_in = (plane_mob_max - plane.mobil as i32).max(0);

    out.push_str(&format!(
        "1 {} positioned over {}, will be ready for use in {} time units\n",
        chr.name, ctx.format_xy(sx, sy), ready_in
    ));

    if let Err(e) = planes::put(ctx.db, &plane).await {
        out.push_str(&format!("1 Warning: satellite save error: {e}\n"));
    }
}
