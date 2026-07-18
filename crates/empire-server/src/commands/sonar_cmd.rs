// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/sona.c (c_sonar)

// "sonar" command — active sonar sweep from a sonar-equipped ship: sweeps
// terrain like 'radar'/'sradar' (using a different, shorter-ranged formula
// capped at 7 hexes), and actively pings nearby ships -- including
// submarines, which neither 'sradar' nor plain 'look' can ever find.
// A pinged ship's owner gets a telegram if they have the sonar flag on
// (see 'info toggle').
//
// Usage: sonar <ship-spec>
//   sonar *        sweep from every eligible owned ship
//   sonar 5        sweep from ship 5
//
// KNOWN GAPS (v1): 4.4.1's sonar additionally requires an unobstructed
// all-water line of sight to each contact (land or another ship blocks the
// ping) -- not modeled here; this is a plain radius sweep, same fidelity
// level as 'radar'/'sradar'/'lradar'. Mine detection at tech>=310 is also
// not ported (no mine-laying mechanic exists yet at all -- see 'info bomb'
// for the same gap noted on the inline nav tokens).

use empire_db::relations::Relation;
use empire_db::{bmap, nations, relations, sectors, ships, telegrams};
use empire_types::coords::Coord;
use empire_types::nation::NatFlags;
use empire_types::sector::{Sector, SectorType};
use empire_types::ship::Ship;
use empire_types::ship_chr::{ShipChr, ShipChrFlags};
use std::collections::HashMap;

use super::ctx::CmdCtx;
use super::radar_cmd::{build_coord_map, render_sweep_grid};
use crate::subs::geo::map_dist;
use crate::subs::shpsub::{ship_spec_matches, shp_visib};
use crate::subs::tech::techfact;

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
    let mut swept_any = false;

    for s in &all_ships {
        if s.own != ctx.cnum { continue; }
        if !ship_spec_matches(spec_str, s) { continue; }
        let Some(chr) = ShipChr::for_type(s.ship_type as usize) else { continue };
        if !is_sonar_eligible(s, chr, &all_sectors, &coord_map) { continue; }

        swept_any = true;
        sonar_sweep_one(ctx, &all_ships, &all_sectors, &coord_map, s, chr, &mut out, &mut bm).await;
    }

    if !swept_any {
        out.push_str(&format!("1 {spec_str}: No eligible sonar-equipped ship(s) in water\n"));
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 sonar\n");
    out
}

/// True if `ship` can run a sonar sweep at all: SONAR capability, currently
/// sitting in a sea sector. Shared by the standalone `sonar` command and
/// `navigate`'s inline `s` token.
pub(crate) fn is_sonar_eligible(
    ship: &Ship,
    chr: &ShipChr,
    all_sectors: &[Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
) -> bool {
    if !chr.flags.contains(ShipChrFlags::SONAR) { return false; }
    coord_map.get(&(ship.x, ship.y))
        .map(|&i| all_sectors[i].sector_type == SectorType::Sea)
        .unwrap_or(false)
}

/// Run one ship's sonar sweep (terrain grid + active ping against nearby
/// foreign ships, including subs) into `out`/`bm`. Mirrors the body of
/// `c_sonar()`'s per-ship loop in sona.c. Caller checks
/// [`is_sonar_eligible`] first.
pub(crate) async fn sonar_sweep_one(
    ctx: &CmdCtx<'_>,
    all_ships: &[Ship],
    all_sectors: &[Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
    s: &Ship,
    chr: &ShipChr,
    out: &mut String,
    bm: &mut bmap::Bmap,
) {
    let wx = ctx.world_x;
    let wy = ctx.world_y;

    let range = techfact(chr.vrnge as f64, s.tech as f64);
    let srange = ((7.0 * range * s.effic as f64 / 200.0) as i32).min(7).max(0);

    render_sweep_grid(
        ctx, all_sectors, coord_map, s.x, s.y, srange, s.effic, "Sonar",
        out, bm,
    );

    for targ in all_ships {
        if targ.own == ctx.cnum || targ.own == 0 { continue; }
        let Some(tchr) = ShipChr::for_type(targ.ship_type as usize) else { continue };

        let dist = map_dist(s.x, s.y, targ.x, targ.y, wx, wy);
        if dist > srange { continue; }

        let visib = shp_visib(targ, tchr);
        let ping_base = (chr.vrnge.max(10) as f64) * range / 10.0;
        let ping_base = ping_base.min(7.0);
        let ping_range = ((ping_base.max(2.0) * targ.effic as f64) / 100.0) as i32;
        let vrange = (ping_base * s.effic as f64 / 200.0) as i32;

        if dist > ping_range { continue; }

        if tchr.flags.contains(ShipChrFlags::SONAR) && targ.own != 0 {
            if let Ok(Some(target_nat)) = nations::get_by_cnum(ctx.db, targ.own).await {
                if target_nat.flags.contains(NatFlags::SONAR) {
                    let body = format!(
                        "Sonar ping from {} detected by ship #{}!",
                        crate::subs::geo::format_xy(&target_nat, s.x, s.y, wx, wy),
                        targ.uid,
                    );
                    let _ = telegrams::send(ctx.db, targ.own, 0, telegrams::TEL_NORM, &body).await;
                }
            }
        }

        if dist > vrange { continue; }

        let is_sub = tchr.flags.contains(ShipChrFlags::SUBMARINE);
        let relation = relations::get(ctx.db, targ.own, ctx.cnum).await.unwrap_or(Relation::Neutral);
        let line = if is_sub && relation < Relation::Friendly {
            let combined = chr.vrnge as f64 + visib;
            if combined < 8.0 {
                format!("Sonar detects sub #{} @ {}", targ.uid, ctx.format_xy(targ.x, targ.y))
            } else if combined < 10.0 {
                format!("Sonar detects {} @ {}", tchr.name, ctx.format_xy(targ.x, targ.y))
            } else {
                format!("Sonar detects Nation #{} {} @ {}", targ.own, tchr.name, ctx.format_xy(targ.x, targ.y))
            }
        } else {
            format!("Sonar detects Nation #{} {} @ {}", targ.own, tchr.name, ctx.format_xy(targ.x, targ.y))
        };
        out.push_str(&format!("1 {line}\n"));
    }
}
