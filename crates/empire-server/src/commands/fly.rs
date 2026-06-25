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
// Ported from: src/lib/commands/fly.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995

// "fly" command — move planes to a friendly destination airfield or harbor.
//
// Usage: fly PLANE-SPEC DEST-SECT
//
// PLANE-SPEC: "*" for all owned planes, single uid, or comma-separated uids.
// DEST-SECT: destination sector (player-relative "X,Y").
//
// Planes can only land at friendly sectors with airfield (a), naval base (n),
// or harbor (*) types, or their own carrier ship.

use empire_db::{planes, sectors};
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::SectorType;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::geo::map_dist;
use crate::subs::plnsub::{pln_capable, pln_use_fuel};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: fly PLANE-SPEC DEST-SECT\n".to_string();
    }

    let plane_spec = parts[0];
    let Some((rx, ry)) = parse_rel_xy(parts[1]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[1]);
    };
    let dx = ctx.x_abs(rx);
    let dy = ctx.y_abs(ry);

    // Validate destination sector
    let dest = match sectors::get_at(ctx.db, dx, dy).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(dx, dy)),
        Err(e)   => return format!("10 DB error: {e}\n"),
    };

    // Destination must be friendly (owned by player or allied) and a valid landing site
    let friendly = dest.own == ctx.cnum || ctx.is_deity;
    if !friendly {
        return format!(
            "10 {} is not a friendly sector — planes cannot land there.\n",
            ctx.format_xy(dx, dy),
        );
    }

    let can_land = matches!(
        dest.sector_type,
        SectorType::Airfield | SectorType::Naval | SectorType::Harbor | SectorType::Missile
    );
    if !can_land {
        return format!(
            "10 {} is not a valid airfield/harbor — planes cannot land there.\n",
            ctx.format_xy(dx, dy),
        );
    }

    // Load planes
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let chrs = PlaneChr::all();

    let selected: Vec<_> = all_planes
        .into_iter()
        .filter(|p| p.own == ctx.cnum && plane_spec_matches(plane_spec, p.uid))
        .collect();

    if selected.is_empty() {
        return "10 No planes match that specification.\n".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!(
        "1 Flying {} plane(s) to {}\n",
        selected.len(),
        ctx.format_xy(dx, dy),
    ));

    let mut flew = 0u32;
    let mut grounded = 0u32;

    for mut plane in selected {
        let Some(chr) = chrs.get(plane.plane_type as usize) else {
            out.push_str(&format!("1 Plane #{}: unknown type — skipped\n", plane.uid));
            grounded += 1;
            continue;
        };

        // Check capability
        if !pln_capable(&plane, chr) {
            out.push_str(&format!(
                "1 Plane #{}: not capable of flying (effic {}%, mob {})\n",
                plane.uid, plane.effic, plane.mobil,
            ));
            grounded += 1;
            continue;
        }

        // Range check — use stored chr.range
        let dist = map_dist(plane.x, plane.y, dx, dy, ctx.world_x, ctx.world_y);
        let max_range = chr.range as i32;
        if dist > max_range {
            out.push_str(&format!(
                "1 Plane #{}: out of range (dist {dist}, range {max_range})\n",
                plane.uid,
            ));
            grounded += 1;
            continue;
        }

        // Deduct fuel
        pln_use_fuel(&mut plane, chr, dist);

        // Move plane to destination
        plane.x = dx;
        plane.y = dy;

        if let Err(e) = planes::put(ctx.db, &plane).await {
            out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
        } else {
            flew += 1;
        }
    }

    if flew > 0 {
        out.push_str(&format!("1 {flew} plane(s) landed safely at {}.\n", ctx.format_xy(dx, dy)));
    }
    if grounded > 0 {
        out.push_str(&format!("1 {grounded} plane(s) could not fly.\n"));
    }

    out.push_str("0 fly\n");
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
