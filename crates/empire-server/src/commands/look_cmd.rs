// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/look.c (c_lookout / do_look(EF_SHIP))

// "look"/"lookout" command — lookout from a ship: reveal the immediate
// 7-hex patch around it (self + 6 neighbors, non-water only), and report
// any foreign ships within visual range. Distinct from 'sradar'/'radar'
// (a much longer-range terrain sweep with no enemy-unit detail) and
// 'sonar' (sonar-equipped ships only, active pinging) -- 'look' is the
// short-range "eyes on deck" check every ship gets for free.
//
// Usage: look <ship-spec>
//   look *        look out from every owned ship
//   look 5        look out from ship 5
//   look c        look out from every ship in fleet c

use empire_db::{bmap, sectors, ships};
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use super::radar_cmd::build_coord_map;
use crate::subs::lookout::{look_neighbors, look_ship_contacts};
use crate::subs::shpsub::ship_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = if args.trim().is_empty() { "*" } else { args.trim() };

    let all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let all_sectors = match sectors::get_all(ctx.db).await {
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

    let mut out = String::new();
    let mut looked_any = false;

    for s in &all_ships {
        if s.own != ctx.cnum { continue; }
        if !ship_spec_matches(spec_str, s) { continue; }
        let Some(chr) = ShipChr::for_type(s.ship_type as usize) else { continue };

        looked_any = true;
        out.push_str(&format!("1 Lookout from ship #{} @ {}\n", s.uid, ctx.format_xy(s.x, s.y)));

        for line in look_neighbors(ctx, &all_sectors, &coord_map, s.x, s.y, &mut bm) {
            out.push_str(&format!("1 {line}\n"));
        }
        for line in look_ship_contacts(ctx, &all_ships, &all_sectors, &coord_map, s, chr) {
            out.push_str(&format!("1 {line}\n"));
        }
    }

    if !looked_any {
        out.push_str(&format!("1 {spec_str}: No matching ship(s)\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 look\n");
    out
}
