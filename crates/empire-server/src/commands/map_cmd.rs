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
// Ported from: src/lib/subs/maps.c, src/lib/commands/map.c,
//              src/lib/subs/border.c

// "map" / "bmap" / "smap" command — display ASCII hex-grid map.
// Usage: map <sector-spec>
// The hex grid places valid sectors at positions where (abs_x + abs_y) is even.
// Each column in the output corresponds to one absolute x position; alternate
// columns are spaces (invalid positions), giving the visual hex stagger.

use std::collections::HashMap;
use empire_db::{sectors, nations};
use empire_types::coords::Coord;
use empire_types::sector::{Sector, SectorType};
use crate::subs::geo;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let arg = args.trim();
    let wx = ctx.world_x;
    let wy = ctx.world_y;

    // Build lookup: (abs_x, abs_y) → char
    let mut lookup: HashMap<(Coord, Coord), char> = HashMap::new();
    for s in &all_sectors {
        lookup.insert((s.x, s.y), map_char(s, ctx.cnum, ctx.is_deity));
    }

    // Compute absolute display bounds from arg.
    // Supported: *, empty → full world; x,y → 11×11; x,y:dist; #N → realm bounding box
    let (abs_lx, display_width, abs_ly, display_height) =
        if let Some(n_str) = arg.strip_prefix('#') {
            let n: u16 = n_str.trim().parse().unwrap_or(0);
            let realms = nations::get_realms(ctx.db, ctx.cnum).await.unwrap_or_default();
            if let Some(r) = realms.iter().find(|rr| rr.realm == n) {
                // Use modular subtraction so realms that wrap across the world edge
                // still yield a positive width/height.
                let w = (((r.xh as i32 - r.xl as i32 + wx as i32) % wx as i32 + 1) as usize)
                    .min(wx as usize);
                let h = (((r.yh as i32 - r.yl as i32 + wy as i32) % wy as i32 + 1) as usize)
                    .min(wy as usize);
                (r.xl, w, r.yl, h)
            } else {
                // Unset realm — small view around capital
                let lx = geo::x_norm(ctx.x_abs(-10), wx);
                let ly = geo::y_norm(ctx.y_abs(-5), wy);
                (lx, 21usize, ly, 11usize)
            }
        } else {
            let (center_abs, radius) = parse_map_arg(arg, ctx);
            if radius < 0 {
                (0i16, wx as usize, 0i16, wy as usize)
            } else {
                let lx = geo::x_norm(center_abs.0 - (2 * radius) as Coord, wx);
                let width = (4 * radius + 1).min(wx) as usize;
                let ly = geo::y_norm(center_abs.1 - radius as Coord, wy);
                let height = (2 * radius + 1).min(wy) as usize;
                (lx, width, ly, height)
            }
        };

    // Player-relative x for the leftmost column
    let rel_lx = ctx.x_rel(abs_lx) as i32;

    let mut out = String::new();

    // Top border (tens then units rows)
    render_border(&mut out, rel_lx, display_width, wx);

    // Map rows: iterate absolute y in order, converting to player-relative for display
    for row in 0..display_height {
        let abs_y = geo::y_norm(abs_ly + row as Coord, wy);
        let rel_y = ctx.y_rel(abs_y);

        // Build this row: one char per column position
        let mut row_chars: Vec<char> = vec![' '; display_width];
        for col in 0..display_width {
            let abs_x = geo::x_norm(abs_lx + col as Coord, wx);
            // Only valid hex positions where (x + y) % 2 == 0
            if (abs_x as i32 + abs_y as i32) % 2 != 0 {
                continue;
            }
            row_chars[col] = lookup.get(&(abs_x, abs_y)).copied().unwrap_or(' ');
        }
        let row_str: String = row_chars.into_iter().collect();
        out.push_str(&format!("1 {:4} {} {}\n", rel_y, row_str, rel_y));
    }

    // Bottom border
    render_border(&mut out, rel_lx, display_width, wx);
    out.push_str("0 map\n");
    out
}

/// Returns the character to display for a sector on the map.
/// ref: map_char() in maps.c
fn map_char(s: &Sector, player_cnum: u8, is_deity: bool) -> char {
    let owner_or_god = s.own == player_cnum || is_deity;
    let t = s.sector_type;
    if owner_or_god
        || t == SectorType::Sea
        || t == SectorType::Mountain
        || t == SectorType::Wasteland
        || (s.own == 0 && (t == SectorType::Wilderness || t == SectorType::Plains))
    {
        t.mnemonic()
    } else {
        '?'
    }
}

/// Parse map area argument into (center_abs, radius).
/// radius < 0 means full-world view.
fn parse_map_arg(arg: &str, ctx: &CmdCtx) -> ((Coord, Coord), i32) {
    if arg.is_empty() || arg == "*" {
        return ((0, 0), -1); // full world
    }

    // Try x,y:dist
    if let Some(colon) = arg.find(':') {
        let (coord_part, dist_part) = arg.split_at(colon);
        let dist_part = &dist_part[1..];
        if let (Some((rx, ry)), Ok(dist)) = (parse_rel_xy(coord_part), dist_part.trim().parse::<i32>()) {
            let ax = ctx.x_abs(rx);
            let ay = ctx.y_abs(ry);
            return ((ax, ay), dist);
        }
    }

    // Try x,y — default radius 5 (11×11 view)
    if let Some((rx, ry)) = parse_rel_xy(arg) {
        let ax = ctx.x_abs(rx);
        let ay = ctx.y_abs(ry);
        return ((ax, ay), 5);
    }

    ((0, 0), -1) // fallback: full world
}

fn parse_rel_xy(s: &str) -> Option<(Coord, Coord)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}

/// Render the x-axis border as two rows of digits (tens then units).
/// Equivalent to border() in border.c with prefstr="     " and sep="".
fn render_border(out: &mut String, rel_lx: i32, width: usize, world_x: i32) {
    // Tens digits
    out.push_str("1      "); // 5-space prefix (matching "%4d " data-row prefix)
    for k in 0..width {
        let x = adjust_x(rel_lx + k as i32, world_x);
        out.push(tens_char(x));
    }
    out.push('\n');

    // Units digits
    out.push_str("1      ");
    for k in 0..width {
        let x = adjust_x(rel_lx + k as i32, world_x);
        let posi = x.unsigned_abs() as u32;
        out.push(char::from_digit(posi % 10, 10).unwrap_or('0'));
    }
    out.push('\n');
}

/// Wrap x into [-world_x/2, world_x/2).
fn adjust_x(x: i32, world_x: i32) -> i32 {
    let mut v = x;
    if v >= world_x / 2 {
        v -= world_x;
    } else if v < -(world_x / 2) {
        v += world_x;
    }
    v
}

/// Tens-place display character for player-relative x (matches border.c).
fn tens_char(x: i32) -> char {
    if x < 0 && x > -10 {
        '-' // single-digit negative: show '-' for the tens place
    } else {
        let posi = x.unsigned_abs() / 10;
        char::from_digit(posi % 10, 10).unwrap_or('0')
    }
}
