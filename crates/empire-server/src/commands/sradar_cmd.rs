// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/rada.c (c_radar / radar(EF_SHIP))

// "sradar" command — sweep from a ship and reveal nearby terrain, the
// ship counterpart to the sector-sourced 'radar' and the land-unit-sourced
// 'lradar'.
//
// Usage: sradar <ship-spec>
//   sradar *        sweep from every owned ship
//   sradar 5        sweep from ship 5
//   sradar c        sweep from every ship in fleet c
//
// <ship-spec> accepts a uid, a uid range, a comma list, "*", "~" (ships
// with no fleet assigned), or a single letter naming a fleet (see
// 'info fleetadd'). Unlike 'lradar' (which requires the RADAR capability),
// every ship type can sweep using its own visual range (ShipChr::vrnge) —
// matches 4.4.1's `radar(EF_SHIP)`, which only special-cases SONAR-flagged
// ships for an additional submarine-detection bonus. That bonus isn't
// modeled here (see 'info sonar' for the dedicated sonar command, which
// covers active sub-hunting); this command is the plain terrain/ship sweep
// every ship gets for free.

use empire_db::{bmap, ships};

use super::ctx::CmdCtx;
use super::radar_cmd::{build_coord_map, render_radar_sweep, seed_bmap_if_blank};
use crate::subs::shpsub::ship_spec_matches;
use empire_types::ship_chr::ShipChr;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = if args.trim().is_empty() { "*" } else { args.trim() };

    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let all_sectors = match empire_db::sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let coord_map = build_coord_map(&all_sectors);

    let wx = ctx.world_x;
    let wy = ctx.world_y;

    let mut bm = match bmap::get_bmap(ctx.db, ctx.cnum, wx as usize, wy as usize).await {
        Ok(b) => b,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    seed_bmap_if_blank(&mut bm, &all_sectors, ctx.cnum);

    let mut out = String::new();
    let mut swept_any = false;

    for s in &all_ships {
        if s.own != ctx.cnum { continue; }
        if !ship_spec_matches(spec_str, s) { continue; }
        let Some(chr) = ShipChr::for_type(s.ship_type as usize) else { continue };

        swept_any = true;
        render_radar_sweep(
            ctx, &all_sectors, &coord_map,
            s.x, s.y, s.effic, s.tech as f64, chr.vrnge as f64,
            &mut out, &mut bm,
        );
    }

    if !swept_any {
        out.push_str(&format!("1 {spec_str}: No matching ship(s)\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 sradar\n");
    out
}
