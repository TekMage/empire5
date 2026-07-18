// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/look.c (c_llookout / do_look(EF_LAND))

// "llook"/"llookout" command — lookout from a land unit: reveal the
// immediate 7-hex patch around it (self + 6 neighbors, non-water only),
// and report any foreign land units/planes within visual range. The
// land-unit counterpart to 'look'/'lookout' for ships.
//
// Usage: llook <unit-spec>
//   llook *        look out from every eligible owned unit
//   llook 5        look out from unit 5
//   llook c        look out from every unit in army c
//
// A unit needs militia aboard to look out, unless it's a spy unit (spies
// don't need military) -- matches 4.4.1's gate in do_look().

use rand::SeedableRng;
use rand::rngs::StdRng;

use empire_db::{bmap, land_units, planes, sectors};
use empire_types::commodity::Item;
use empire_types::land_chr::{LandChr, LandChrFlags};

use super::ctx::CmdCtx;
use super::radar_cmd::build_coord_map;
use crate::subs::lndsub::land_spec_matches;
use crate::subs::lookout::{look_land_contacts, look_neighbors};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = if args.trim().is_empty() { "*" } else { args.trim() };

    let all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let all_planes = match planes::get_all(ctx.db).await {
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

    let mut rng = StdRng::from_entropy();
    let mut out = String::new();
    let mut looked_any = false;

    for u in &all_units {
        if u.own != ctx.cnum { continue; }
        if !land_spec_matches(spec_str, u) { continue; }
        if u.ship >= 0 || u.carried_by_land >= 0 { continue; }

        let Some(lchr) = LandChr::for_type(u.land_type as usize) else { continue };
        let is_spy = lchr.flags.contains(LandChrFlags::SPY);
        if u.items.get(Item::Milit) <= 0 && !is_spy { continue; }

        looked_any = true;
        out.push_str(&format!("1 Lookout from unit #{} @ {}\n", u.uid, ctx.format_xy(u.x, u.y)));

        for line in look_neighbors(ctx, &all_sectors, &coord_map, u.x, u.y, &mut bm) {
            out.push_str(&format!("1 {line}\n"));
        }
        for line in look_land_contacts(ctx, &all_units, &all_planes, u, lchr, &mut rng) {
            out.push_str(&format!("1 {line}\n"));
        }
    }

    if !looked_any {
        out.push_str(&format!("1 {spec_str}: No matching unit(s)\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 llook\n");
    out
}
