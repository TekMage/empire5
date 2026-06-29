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
// Ported from: src/lib/commands/dist.c

// "distribute" command — set the distribution center for sectors.
// Usage: distribute <sect-spec> [<X,Y>|.|h]
//   X,Y = absolute sector coordinates
//   .   = self (sector itself as dist center)
//   h   = self (highway shorthand, same as .)

use empire_db::sectors;
use empire_types::coords::Coord;
use super::ctx::CmdCtx;
use super::sector_sel::{SectSpec, parse_rel_xy};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return "10 Usage: distribute <sect-spec> [<X,Y>|.|h]\n".to_string();
    }

    let area_spec = parts[0].trim();
    let dest_spec = parts.get(1).copied().unwrap_or("").trim();

    let filter = match SectSpec::parse(area_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mut out = String::new();
    let mut count = 0u32;

    for mut s in all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity {
            continue;
        }
        if s.own == 0 {
            continue;
        }
        if !filter.matches(&s, ctx.world_x, ctx.world_y) {
            continue;
        }

        let xy = ctx.format_xy(s.x, s.y);
        let old_dist_xy = ctx.format_xy(s.dist_x, s.dist_y);

        if dest_spec.is_empty() {
            // Display-only mode
            out.push_str(&format!(
                "1 {} distributes to {}\n",
                xy, old_dist_xy
            ));
            continue;
        }

        // Resolve destination coordinates
        let (new_dx, new_dy): (Coord, Coord) = if dest_spec == "." || dest_spec == "h" {
            (s.x, s.y)
        } else {
            match parse_rel_xy(dest_spec) {
                Some((rx, ry)) => (ctx.x_abs(rx), ctx.y_abs(ry)),
                None => {
                    out.push_str(&format!(
                        "10 Bad destination '{}'\n", dest_spec
                    ));
                    return out;
                }
            }
        };

        // Validate that the target sector exists
        match sectors::get_at(ctx.db, new_dx, new_dy).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                let dest_fmt = ctx.format_xy(new_dx, new_dy);
                out.push_str(&format!("1 {} Sector {} doesn't exist\n", xy, dest_fmt));
                continue;
            }
            Err(e) => {
                out.push_str(&format!("1 {}: database error: {e}\n", xy));
                continue;
            }
        }

        s.dist_x = new_dx;
        s.dist_y = new_dy;

        match sectors::put(ctx.db, &s).await {
            Ok(_) => {
                let new_dist_xy = ctx.format_xy(new_dx, new_dy);
                out.push_str(&format!(
                    "1 {} distribution center set to {}\n",
                    xy, new_dist_xy
                ));
                count += 1;
            }
            Err(e) => {
                out.push_str(&format!("1 {}: database error: {e}\n", xy));
            }
        }
    }

    if count == 0 && out.is_empty() {
        out.push_str("1 No sectors matched\n");
    }
    out.push_str("0 distribute\n");
    out
}
