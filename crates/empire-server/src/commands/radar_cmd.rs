// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/rada.c, src/lib/subs/radmap.c

// "radar" command — sweep from owned radar sectors and reveal nearby terrain.
//
// Usage: radar <sector-spec>
//   e.g. radar *        (sweep all your radar sectors)
//   e.g. radar 4,4      (sweep radar sector at 4,4)
//
// Range = floor( techfact(tech, spy) * eff/100 )
// techfact(tech, spy) = spy * (50 + tech) / (200 + tech)
//
// Within range/3: shows actual sector mnemonic.
// Beyond range/3: shows '?' (detected but not identified).
// Own sectors, water, mountains, and wasteland always show actual mnemonic.
//
// A radar-capable land unit can also sweep — see 'lradar'/'info lradar'.
// (4.4.1's ships with SONAR capability can source radar too; empire5
// doesn't wire that up yet, same documented gap as lradar's v1 scope.)

use std::collections::HashMap;
use empire_db::{sectors, bmap};
use empire_db::bmap::Bmap;
use empire_types::sector::{Sector, SectorType};
use empire_types::coords::Coord;
use crate::subs::geo::{map_dist, xydist_range, xy_in_range, x_norm, y_norm};
use crate::subs::tech::techfact;
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;

// Radar spy power for a ) sector (matches 4.4.1 hardcoded value of 16).
const RADAR_SPY: f64 = 16.0;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = if args.trim().is_empty() { "*" } else { args.trim() };

    let filter = match SectSpec::parse(spec_str, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let coord_map = build_coord_map(&all_sectors);
    let wx = ctx.world_x;
    let wy = ctx.world_y;
    let tech = ctx.nat.tech;

    let mut bm = match bmap::get_bmap(ctx.db, ctx.cnum, wx as usize, wy as usize).await {
        Ok(b) => b,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    seed_bmap_if_blank(&mut bm, &all_sectors, ctx.cnum);

    let mut out = String::new();
    let mut swept_any = false;

    for s in &all_sectors {
        if s.own != ctx.cnum { continue; }
        if s.sector_type != SectorType::Radar { continue; }
        if s.effic < 60 { continue; }
        if !filter.matches(s, wx, wy) { continue; }

        swept_any = true;
        render_radar_sweep(
            ctx, &all_sectors, &coord_map,
            s.x, s.y, s.effic, tech, RADAR_SPY,
            &mut out, &mut bm,
        );
    }

    if !swept_any {
        out.push_str(&format!("1 {spec_str}: No radar sector(s)\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 radar\n");
    out
}

pub(crate) fn build_coord_map(all_sectors: &[Sector]) -> HashMap<(Coord, Coord), usize> {
    all_sectors.iter()
        .enumerate().map(|(i, s)| ((s.x, s.y), i)).collect()
}

pub(crate) fn seed_bmap_if_blank(bm: &mut Bmap, all_sectors: &[Sector], cnum: u8) {
    if bm.is_empty() {
        for s in all_sectors {
            if s.own == cnum {
                bm.set(s.x, s.y, s.sector_type.mnemonic() as u8);
            }
        }
    }
}

/// Compute radar range from efficiency, tech level, and spy power.
/// range = floor( (50+tech)/(200+tech) * spy * eff/100 ), minimum 1.
pub(crate) fn radar_range(eff: i8, tech: f64, spy: f64) -> i32 {
    let r = (techfact(spy, tech) * eff as f64 / 100.0) as i32;
    r.max(1)
}

/// Render one radar sweep centered at (cx,cy) into `out`, updating `bm`.
/// Shared by `radar` (sector-sourced) and `lradar` (land-unit-sourced).
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_radar_sweep(
    ctx: &CmdCtx<'_>,
    all_sectors: &[Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
    cx: Coord, cy: Coord,
    effic: i8,
    tech: f64,
    spy: f64,
    out: &mut String,
    bm: &mut Bmap,
) {
    let wx = ctx.world_x;
    let wy = ctx.world_y;
    let range = radar_range(effic, tech, spy);

    out.push_str(&format!(
        "1 Radar at {} efficiency {}%, max range {}\n",
        ctx.format_xy(cx, cy), effic, range
    ));

    let scan_range = xydist_range(cx, cy, range, wx, wy);

    let display_range = range;
    let disp_lx = x_norm(cx - (2 * display_range) as Coord, wx);
    let disp_ly = y_norm(cy - display_range as Coord, wy);
    let disp_w  = (4 * display_range + 1).min(wx) as usize;
    let disp_h  = (2 * display_range + 1).min(wy) as usize;

    let mut grid: Vec<Vec<char>> = vec![vec![' '; disp_w]; disp_h];

    for row in 0..disp_h {
        let abs_y = y_norm(disp_ly + row as Coord, wy);
        for col in 0..disp_w {
            let abs_x = x_norm(disp_lx + col as Coord, wx);
            if (abs_x as i32 + abs_y as i32) % 2 != 0 { continue; }
            if !xy_in_range(abs_x, abs_y, &scan_range) { continue; }

            let dist = map_dist(cx, cy, abs_x, abs_y, wx, wy);
            if dist > range { continue; }

            let ch = if let Some(&si) = coord_map.get(&(abs_x, abs_y)) {
                let sec = &all_sectors[si];
                radar_char(sec, dist, range, ctx.cnum)
            } else {
                '.'
            };

            grid[row][col] = ch;
            if ch != ' ' {
                bm.set(abs_x, abs_y, ch as u8);
            }
        }
    }

    let center_row = y_norm(cy - disp_ly, wy) as usize;
    let center_col = x_norm(cx - disp_lx, wx) as usize;
    if center_row < disp_h && center_col < disp_w {
        grid[center_row][center_col] = '0';
        bm.set(cx, cy, b'0');
    }

    let rel_lx = ctx.x_rel(disp_lx) as i32;
    render_radar_border(out, rel_lx, disp_w, wx);
    for row in 0..disp_h {
        let abs_y = y_norm(disp_ly + row as Coord, wy);
        let rel_y = ctx.y_rel(abs_y);
        let row_str: String = grid[row].iter().collect();
        out.push_str(&format!("1 {:4} {} {}\n", rel_y, row_str, rel_y));
    }
    render_radar_border(out, rel_lx, disp_w, wx);
    out.push_str("1\n");
}

/// Character to display for a sector during a radar sweep.
/// Matches rad_char() in radmap.c:
///   own sectors, water, mountain, wasteland: always show actual mnemonic
///   within range/3: show actual mnemonic
///   otherwise: '?'
pub(crate) fn radar_char(s: &Sector, dist: i32, range: i32, cnum: u8) -> char {
    if s.own == cnum
        || s.sector_type == SectorType::Sea
        || s.sector_type == SectorType::Mountain
        || s.sector_type == SectorType::Wasteland
        || dist <= range / 3
    {
        s.sector_type.mnemonic()
    } else {
        '?'
    }
}

/// Silently sweep terrain into `bm` centered at (cx,cy) — same range and
/// visibility rule as `render_radar_sweep`, but with no textual output.
/// Used by ship navigation to passively reveal terrain as a ship moves,
/// mirroring 4.4.1's rad_map_set() (called via unit_rad_map_set() from
/// unit_move() in unitsub.c) — every ship sweeps from its own position
/// using its type's visual range (m_vrnge/ShipChr::vrnge) as "spy" power,
/// no dedicated radar sector required.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sweep_bmap(
    coord_map: &HashMap<(Coord, Coord), usize>,
    all_sectors: &[Sector],
    cx: Coord, cy: Coord,
    effic: i8, tech: f64, spy: f64,
    owner: u8,
    world_x: i32, world_y: i32,
    bm: &mut Bmap,
) {
    let range = radar_range(effic, tech, spy);
    let disp_lx = x_norm(cx - (2 * range) as Coord, world_x);
    let disp_ly = y_norm(cy - range as Coord, world_y);
    let disp_w = (4 * range + 1).min(world_x) as usize;
    let disp_h = (2 * range + 1).min(world_y) as usize;

    for row in 0..disp_h {
        let abs_y = y_norm(disp_ly + row as Coord, world_y);
        for col in 0..disp_w {
            let abs_x = x_norm(disp_lx + col as Coord, world_x);
            if (abs_x as i32 + abs_y as i32) % 2 != 0 { continue; }
            let dist = map_dist(cx, cy, abs_x, abs_y, world_x, world_y);
            if dist > range { continue; }
            let ch = if let Some(&si) = coord_map.get(&(abs_x, abs_y)) {
                radar_char(&all_sectors[si], dist, range, owner)
            } else {
                '.'
            };
            bm.set(abs_x, abs_y, ch as u8);
        }
    }
    bm.set(cx, cy, b'0');
}

pub(crate) fn render_radar_border(out: &mut String, rel_lx: i32, width: usize, world_x: i32) {
    out.push_str("1      ");
    for k in 0..width {
        let x = adjust_x(rel_lx + k as i32, world_x);
        out.push(tens_char(x));
    }
    out.push('\n');
    out.push_str("1      ");
    for k in 0..width {
        let x = adjust_x(rel_lx + k as i32, world_x);
        let posi = x.unsigned_abs() as u32;
        out.push(char::from_digit(posi % 10, 10).unwrap_or('0'));
    }
    out.push('\n');
}

fn adjust_x(x: i32, world_x: i32) -> i32 {
    let mut v = x;
    if v >= world_x / 2 { v -= world_x; }
    else if v < -(world_x / 2) { v += world_x; }
    v
}

fn tens_char(x: i32) -> char {
    if x < 0 && x > -10 { '-' }
    else { char::from_digit(x.unsigned_abs() / 10 % 10, 10).unwrap_or('0') }
}
