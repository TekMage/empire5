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

use std::collections::{HashMap, HashSet};
use empire_db::{sectors, nations, bmap};
use empire_types::coords::Coord;
use empire_types::sector::{Sector, SectorType};
use crate::subs::geo;
use super::ctx::CmdCtx;

// Visibility offsets per efficiency tier, mirroring getbit.c bitmaps[0..4].
// Each entry is (dx, dy) added to a sector's coords; includes the sector itself.
// Tier = effic/20, clamped to 4.
//   tier 0 (eff  0-20): immediate ring + self
//   tier 1 (eff 21-40): ~2-hex ring
//   tier 2 (eff 41-60): ~3-hex ring
//   tier 3 (eff 61-80): ~5-hex ring
//   tier 4 (eff 81+):   ~6-hex ring
const VIS0: &[(i16,i16)] = &[
    (-1,-1),(1,-1),(-2,0),(0,0),(2,0),(-1,1),(1,1),
];
const VIS1: &[(i16,i16)] = &[
    (0,-2),(-3,-1),(-1,-1),(1,-1),(3,-1),(-2,0),(0,0),(2,0),(-3,1),(-1,1),(1,1),(3,1),(0,2),
];
const VIS2: &[(i16,i16)] = &[
    (-2,-2),(0,-2),(2,-2),(-3,-1),(-1,-1),(1,-1),(3,-1),
    (-4,0),(-2,0),(0,0),(2,0),(4,0),(-3,1),(-1,1),(1,1),(3,1),(-2,2),(0,2),(2,2),
];
const VIS3: &[(i16,i16)] = &[
    (-1,-3),(1,-3),(-4,-2),(-2,-2),(0,-2),(2,-2),(4,-2),
    (-5,-1),(-3,-1),(-1,-1),(1,-1),(3,-1),(5,-1),
    (-4,0),(-2,0),(0,0),(2,0),(4,0),
    (-5,1),(-3,1),(-1,1),(1,1),(3,1),(5,1),
    (-4,2),(-2,2),(0,2),(2,2),(4,2),(-1,3),(1,3),
];
const VIS4: &[(i16,i16)] = &[
    (-3,-3),(-1,-3),(1,-3),(3,-3),(-4,-2),(-2,-2),(0,-2),(2,-2),(4,-2),
    (-5,-1),(-3,-1),(-1,-1),(1,-1),(3,-1),(5,-1),
    (-6,0),(-4,0),(-2,0),(0,0),(2,0),(4,0),(6,0),
    (-5,1),(-3,1),(-1,1),(1,1),(3,1),(5,1),
    (-4,2),(-2,2),(0,2),(2,2),(4,2),(-3,3),(-1,3),(1,3),(3,3),
];
const VIS_OFFSETS: [&[(i16,i16)]; 5] = [VIS0, VIS1, VIS2, VIS3, VIS4];

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let arg = args.trim();
    let wx = ctx.world_x;
    let wy = ctx.world_y;

    // Load fog-of-war map (deities see all).
    let mut bm = if ctx.is_deity {
        None
    } else {
        match bmap::get_bmap(ctx.db, ctx.cnum, wx as usize, wy as usize).await {
            Ok(b) => Some(b),
            Err(_) => None,
        }
    };

    // Seed bmap with own sectors if it's completely empty.
    let mut bmap_changed = false;
    if let Some(ref mut b) = bm {
        if b.is_empty() {
            for s in &all_sectors {
                if s.own == ctx.cnum {
                    b.set(s.x, s.y, s.sector_type.mnemonic() as u8);
                    bmap_changed = true;
                }
            }
        }
    }

    // Build visibility set using efficiency-scaled offsets (mirrors bitinit2() in
    // 4.4.1 getbit.c).  Each owned sector's efficiency tier determines how far it
    // can see; the union of all visible coords is the player's current sight range.
    let visible: HashSet<(Coord, Coord)> = if ctx.is_deity {
        HashSet::new()
    } else {
        let mut vis = HashSet::new();
        for s in &all_sectors {
            if s.own == ctx.cnum {
                let tier = ((s.effic as usize) / 20).min(4);
                for &(dx, dy) in VIS_OFFSETS[tier] {
                    let nx = geo::x_norm(s.x + dx, wx);
                    let ny = geo::y_norm(s.y + dy, wy);
                    vis.insert((nx, ny));
                }
            }
        }
        vis
    };

    // Build lookup: (abs_x, abs_y) → char applying fog of war.
    let mut lookup: HashMap<(Coord, Coord), char> = HashMap::new();
    for s in &all_sectors {
        let ch = if ctx.is_deity {
            map_char(s, ctx.cnum, true)
        } else if visible.contains(&(s.x, s.y)) {
            // In sight range: show actual terrain; enemy-owned → '?'
            map_char(s, ctx.cnum, false)
        } else {
            // Outside range: show bmap (previous intel) or blank
            fog_map_char(s, ctx.cnum, bm.as_ref())
        };
        lookup.insert((s.x, s.y), ch);

        // Update bmap with anything identifiable in the current sight range
        if let Some(ref mut b) = bm {
            if visible.contains(&(s.x, s.y)) {
                let t = s.sector_type;
                // Store the mnemonic for own/unowned/topology sectors.
                // Don't overwrite a known mnemonic with '?' for enemy sectors.
                if s.own == ctx.cnum || s.own == 0
                    || t == SectorType::Sea
                    || t == SectorType::Mountain
                    || t == SectorType::Wasteland
                {
                    let mnem = t.mnemonic() as u8;
                    if b.get(s.x, s.y) != mnem {
                        b.set(s.x, s.y, mnem);
                        bmap_changed = true;
                    }
                }
            }
        }
    }

    // Persist bmap if changed
    if bmap_changed {
        if let Some(ref b) = bm {
            let _ = bmap::put_bmap(ctx.db, ctx.cnum, b).await;
        }
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

/// Outside current sight range: show own sectors and previously-seen bmap data.
fn fog_map_char(s: &Sector, cnum: u8, bm: Option<&bmap::Bmap>) -> char {
    if s.own == cnum {
        return s.sector_type.mnemonic();
    }
    if let Some(b) = bm {
        let seen = b.get(s.x, s.y);
        if seen != 0 { return seen as char; }
    }
    ' '
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
