// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/rada.c (c_lradar / radar(EF_LAND))

// "lradar" command — sweep from a radar-capable land unit and reveal
// nearby terrain, the land-unit counterpart to the sector-sourced 'radar'.
//
// Usage: lradar <unit-spec>
//   lradar *        sweep from every eligible owned radar unit
//   lradar 5        sweep from unit 5
//   lradar c        sweep from every unit in army c
//
// <unit-spec> accepts a uid, a uid range, a comma list, "*", "~" (units
// with no army assigned), or a single letter naming an army (see
// 'info army'). A unit must have the radar capability, be off no ship
// and no other unit, to sweep. Range uses the unit's `spy` stat (same
// formula as sector radar, just a different spy-power input) — see
// 'info radar' for the range/detail formula.
//
// v1 gap: 4.4.1's `lradar` also accepts a sector-spec (redundant with
// 'radar', which already covers radar-station sectors) — not ported
// here to keep the two commands' roles distinct.

use empire_db::land_units;
use empire_db::bmap;
use empire_types::land_chr::{LandChr, LandChrFlags};

use super::ctx::CmdCtx;
use super::radar_cmd::{build_coord_map, render_radar_sweep, seed_bmap_if_blank};
use crate::subs::lndsub::land_spec_matches;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = if args.trim().is_empty() { "*" } else { args.trim() };

    let all_units = match land_units::get_all(ctx.db).await {
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

    for u in &all_units {
        if u.own != ctx.cnum { continue; }
        if !land_spec_matches(spec_str, u) { continue; }

        let Some(lchr) = LandChr::for_type(u.land_type as usize) else { continue };
        if !lchr.flags.contains(LandChrFlags::RADAR) {
            out.push_str(&format!("1 Unit #{} can't use radar!\n", u.uid));
            continue;
        }
        if u.ship >= 0 {
            out.push_str(&format!(
                "1 Unit #{} is stowed on ship #{}, and can't use radar!\n", u.uid, u.ship
            ));
            continue;
        }
        if u.carried_by_land >= 0 {
            out.push_str(&format!(
                "1 Unit #{} is stowed on land unit #{}, and can't use radar!\n",
                u.uid, u.carried_by_land
            ));
            continue;
        }

        swept_any = true;
        render_radar_sweep(
            ctx, &all_sectors, &coord_map,
            u.x, u.y, u.effic, u.tech as f64, lchr.spy as f64,
            &mut out, &mut bm,
        );
    }

    if !swept_any {
        out.push_str(&format!("1 {spec_str}: No radar-capable land unit(s)\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 lradar\n");
    out
}
