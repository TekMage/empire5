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

// Shared sector-spec matching helpers used by distribute, deliver, and other
// commands that accept a sector specifier argument.

use empire_types::sector::Sector;
use super::ctx::CmdCtx;
use crate::subs::geo;

/// Return true if sector `s` matches `spec` in the context of `ctx`.
///
/// Spec formats:
///   `*`       — all sectors
///   `X,Y`     — exact match (relative coordinates)
///   `X,Y:R`   — within radius R (relative coordinates)
pub fn matches_area(s: &Sector, spec: &str, ctx: &CmdCtx<'_>) -> bool {
    if spec.is_empty() || spec == "*" {
        return true;
    }
    if let Some(pos) = spec.find(':') {
        let (coord_part, dist_part) = spec.split_at(pos);
        let dist_part = &dist_part[1..];
        if let (Some((rx, ry)), Ok(dist)) =
            (parse_rel_xy(coord_part), dist_part.trim().parse::<i32>())
        {
            let ax = ctx.x_abs(rx);
            let ay = ctx.y_abs(ry);
            let d = geo::map_dist(s.x, s.y, ax, ay, ctx.world_x, ctx.world_y);
            return d <= dist;
        }
    }
    if let Some((rx, ry)) = parse_rel_xy(spec) {
        return s.x == ctx.x_abs(rx) && s.y == ctx.y_abs(ry);
    }
    // Unknown spec format — match all (safe fallback)
    true
}

/// Parse "X,Y" relative coordinate string.
pub fn parse_rel_xy(s: &str) -> Option<(i16, i16)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}
