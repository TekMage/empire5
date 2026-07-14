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
// Ported from: src/lib/commands/reco.c, src/lib/subs/aircombat.c
// (the PM_R recon-reporting branch of ac_encounter())

// "recon" / "sweep" command — fly a reconnaissance mission along a path,
// reporting on every sector overflown, then land at the destination.
//
// Usage: recon <spy-plane-spec> <escort-plane-spec> <ap-sect> <path|dest>
//   sweep <spy-plane-spec> <escort-plane-spec> <ap-sect> <path|dest>
//
// <spy-plane-spec>/<escort-plane-spec>: "none"/"~" for no planes of that
//   role, or anything plane_spec_matches accepts (uid, range, comma list,
//   "*", a wing letter — see 'info wingadd'). Escort planes must be
//   fighter/escort-capable; both lists must currently be sitting exactly
//   at <ap-sect> (a simplification — see 'info recon' for documented gaps).
// <ap-sect>: assembly point, player-relative "X,Y".
// <path|dest>: a direction-string route (u/j/n/b/g/y, 'h' to stop early)
//   or a destination "X,Y" to auto-path toward.
//
// Report detail at each sector along the way depends on which capable
// planes are still alive in the flight: SPY-flagged survivors get the
// full satellite-style intel report (same rendering as 'satellite');
// otherwise a generic terrain + foreign-presence report. Losing your
// spy plane mid-route downgrades the rest of the report, matching
// 4.4.1's per-step capability recompute.
//
// v1 gaps (see 'info recon'): no Anti-Sub Patrol report variant, and a
// single combined interception check at the destination rather than
// per-hex flak (matches bomb.rs's existing simplification level).
//
// 'sweep' differs from 'recon' in one respect, ported from
// plane_sweep() in aircombat.c: at every sea sector overflown, each
// still-alive mission-list (<spy-plane-spec>) plane flagged SWEEP gets
// one roll to clear a mine, if the sector has any (escorts don't
// participate, matching 4.4.1's plane_sweep(bomb_list, ...) call —
// only the mission list is passed, not esc_list).

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use empire_db::{land_units, planes, sectors, ships};
use empire_types::coords::Coord;
use empire_types::land_chr::{LandChr, LandChrFlags};
use empire_types::plane::Plane;
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::sector::SectorType;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use super::satellite_cmd::format_sect_row;
use crate::subs::aircombat::{air_combat, find_interceptors};
use crate::subs::geo::{dir_from_char, x_norm, y_norm, DIROFF, DIR_FIRST, DIR_LAST};
use crate::subs::pathfind::find_path;
use crate::subs::plnsub::{pln_capable, pln_use_fuel, plane_spec_matches};

pub async fn run(args: &str, ctx: &CmdCtx<'_>, sweep_mode: bool) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 4 {
        return "10 Usage: recon <spy-plane-spec> <escort-plane-spec> <ap-sect> <path|dest>\n".to_string();
    }
    let spy_spec = parts[0];
    let escort_spec = parts[1];
    let Some((rx, ry)) = parse_rel_xy(parts[2]) else {
        return format!("10 Bad assembly point '{}'\n", parts[2]);
    };
    let ap_x = ctx.x_abs(rx);
    let ap_y = ctx.y_abs(ry);
    let route_str = parts[3];

    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let chrs = PlaneChr::all();

    let is_none = |spec: &str| spec.is_empty() || spec == "none" || spec == "~";

    let mut spy_planes: Vec<Plane> = if is_none(spy_spec) {
        Vec::new()
    } else {
        all_planes.iter()
            .filter(|p| {
                p.own == ctx.cnum && p.x == ap_x && p.y == ap_y
                    && plane_spec_matches(spy_spec, p)
                    && chrs.get(p.plane_type as usize).map(|c| pln_capable(p, c)).unwrap_or(false)
            })
            .cloned().collect()
    };
    let mut escort_planes: Vec<Plane> = if is_none(escort_spec) {
        Vec::new()
    } else {
        all_planes.iter()
            .filter(|p| {
                p.own == ctx.cnum && p.x == ap_x && p.y == ap_y
                    && plane_spec_matches(escort_spec, p)
                    && chrs.get(p.plane_type as usize).map(|c| {
                        pln_capable(p, c)
                            && (c.flags.contains(PlaneChrFlags::FIGHTER) || c.flags.contains(PlaneChrFlags::ESCORT))
                    }).unwrap_or(false)
            })
            .cloned().collect()
    };

    if spy_planes.is_empty() && escort_planes.is_empty() {
        return "10 No capable planes match that specification at the assembly point.\n".to_string();
    }

    // Build the flight path: a direction-string route, or a destination
    // sector to auto-path toward. Planes overfly anything, so no terrain
    // passability filter (unlike navigate.rs's ship routing).
    let path = match build_flight_path(route_str, ap_x, ap_y, ctx) {
        Ok(p) => p,
        Err(e) => return format!("10 {e}\n"),
    };

    let trailer = if sweep_mode { "sweep" } else { "recon" };
    let mut out = String::new();
    out.push_str(&format!("1 {} mission from {}\n",
        if sweep_mode { "Sweep" } else { "Recon" }, ctx.format_xy(ap_x, ap_y)));

    let mut rng = StdRng::from_entropy();

    // Walk the path, reporting at every sector visited (including the
    // assembly point itself).
    let mut cx = ap_x;
    let mut cy = ap_y;
    report_sector(ctx, cx, cy, &spy_planes, &escort_planes, chrs, sweep_mode, &mut rng, &mut out).await;

    for &dir in &path {
        let (dx, dy) = DIROFF[dir as usize];
        cx = x_norm(cx + dx, ctx.world_x);
        cy = y_norm(cy + dy, ctx.world_y);
        report_sector(ctx, cx, cy, &spy_planes, &escort_planes, chrs, sweep_mode, &mut rng, &mut out).await;
    }

    // Single combined interception check at the destination (matches
    // bomb.rs's existing simplification level — see module doc gaps).
    let mut all_mission: Vec<Plane> = spy_planes.drain(..).chain(escort_planes.drain(..)).collect();
    let mut interceptors = match find_interceptors(ctx.db, cx, cy, ctx.cnum, ctx.world_x, ctx.world_y).await {
        Ok(v) => v,
        Err(e) => {
            out.push_str(&format!("1 Warning: could not load interceptors: {e}\n"));
            vec![]
        }
    };
    if !interceptors.is_empty() {
        out.push_str(&format!("1 Air combat: {} interceptor(s) scramble\n", interceptors.len()));
        let int_chrs = PlaneChr::all();
        let combat_log = air_combat(&mut all_mission, &mut interceptors, chrs, int_chrs, &mut rng);
        for line in combat_log {
            out.push_str(&format!("1 {line}\n"));
        }
        for plane in &interceptors {
            let _ = planes::put(ctx.db, plane).await;
        }
    }

    all_mission.retain(|p| p.effic > 0);
    if all_mission.is_empty() {
        out.push_str("1 All planes lost — no survivors to land.\n");
        out.push_str(&format!("0 {trailer}\n"));
        return out;
    }

    // Land at the destination — a friendly airfield/harbor, or aboard
    // a friendly carrier there (same rules fly.rs enforces; see its
    // module doc for the carrier-selection simplification).
    let landing = match sectors::get_at(ctx.db, cx, cy).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let ships_here = match ships::get_at_xy(ctx.db, cx, cy).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let ship_chrs = ShipChr::all();
    let carriers = crate::subs::shipcarry::eligible_carriers(
        &ships_here, ship_chrs, ctx.cnum, ctx.is_deity, cx, cy,
    );
    let sector_ok = landing.as_ref().is_some_and(|s| {
        s.own == ctx.cnum && matches!(s.sector_type, SectorType::Airfield | SectorType::Harbor)
    });

    if !sector_ok && carriers.is_empty() {
        out.push_str(&format!(
            "1 {} is not a friendly airfield/harbor — planes cannot land there.\n",
            ctx.format_xy(cx, cy)
        ));
        out.push_str(&format!("0 {trailer}\n"));
        return out;
    }

    let mut manifests: Vec<Vec<Plane>> = Vec::with_capacity(carriers.len());
    for c in &carriers {
        manifests.push(planes::get_on_ship(ctx.db, c.uid).await.unwrap_or_default());
    }

    let path_len = path.len() as i32;
    let mut landed = 0u32;
    let mut crashed = 0u32;
    for mut plane in all_mission {
        let Some(chr) = chrs.get(plane.plane_type as usize) else { continue };
        pln_use_fuel(&mut plane, chr, path_len.max(1));

        let mut landed_on = None;
        for (i, c) in carriers.iter().enumerate() {
            let Some(c_chr) = ship_chrs.get(c.ship_type as usize) else { continue };
            if crate::subs::shipcarry::ship_can_carry(c_chr, &manifests[i], chrs, chr.flags) {
                landed_on = Some(i);
                break;
            }
        }

        if let Some(i) = landed_on {
            let c = carriers[i];
            plane.ship = c.uid;
            plane.x = c.x;
            plane.y = c.y;
            if planes::put(ctx.db, &plane).await.is_ok() {
                manifests[i].push(plane.clone());
                out.push_str(&format!("1 Plane #{} landed aboard ship #{}\n", plane.uid, c.uid));
                landed += 1;
            }
        } else if sector_ok {
            plane.x = cx;
            plane.y = cy;
            if planes::put(ctx.db, &plane).await.is_ok() {
                landed += 1;
            }
        } else {
            plane.effic = 0;
            if planes::put(ctx.db, &plane).await.is_ok() {
                out.push_str(&format!("1 Plane #{}: no room to land — crashes and burns\n", plane.uid));
                crashed += 1;
            }
        }
    }

    if landed > 0 {
        out.push_str(&format!("1 {landed} plane(s) landed at {}\n", ctx.format_xy(cx, cy)));
    }
    if crashed > 0 {
        out.push_str(&format!("1 {crashed} plane(s) had nowhere to land and crashed.\n"));
    }
    out.push_str(&format!("0 {trailer}\n"));
    out
}

/// Parse a route string into a sequence of direction indices (1-6), or
/// auto-path toward a destination "X,Y". Mirrors navigate.rs's
/// build_route, minus the terrain-passability filter (planes fly over
/// anything) and the 'v'/VIEW_MARKER sentinel (recon always reports
/// every step, there's no separate opt-in view action).
fn build_flight_path(route_str: &str, from_x: Coord, from_y: Coord, ctx: &CmdCtx<'_>) -> Result<Vec<u8>, String> {
    if let Some((rx, ry)) = parse_rel_xy(route_str) {
        let dx = ctx.x_abs(rx);
        let dy = ctx.y_abs(ry);
        let dirs = find_path(from_x, from_y, dx, dy, ctx.world_x, ctx.world_y, |_, _| true);
        if dirs.is_empty() && (from_x != dx || from_y != dy) {
            return Err(format!("no path to {}", ctx.format_xy(dx, dy)));
        }
        return Ok(dirs);
    }

    let mut dirs = Vec::new();
    for ch in route_str.chars() {
        match dir_from_char(ch) {
            Some(d) if d >= DIR_FIRST && d <= DIR_LAST => dirs.push(d as u8),
            Some(0) => break, // DIR_STOP ('h')
            _ => return Err(format!("unknown direction character '{ch}'")),
        }
    }
    Ok(dirs)
}

/// OR of chr.flags across all still-alive planes in the flight — report
/// detail at each sector is a live function of this, recomputed every
/// step, matching ac_encounter()'s per-step pln_caps() recompute (a spy
/// plane shot down mid-route silently downgrades the rest of the report).
fn combined_caps(spy: &[Plane], escort: &[Plane], chrs: &[PlaneChr]) -> PlaneChrFlags {
    let mut flags = PlaneChrFlags::empty();
    for p in spy.iter().chain(escort.iter()) {
        if p.effic <= 0 { continue; }
        if let Some(chr) = chrs.get(p.plane_type as usize) {
            flags |= chr.flags;
        }
    }
    flags
}

async fn report_sector(
    ctx: &CmdCtx<'_>,
    x: Coord, y: Coord,
    spy: &[Plane], escort: &[Plane], chrs: &[PlaneChr],
    sweep_mode: bool, rng: &mut StdRng,
    out: &mut String,
) {
    let mut sect = match sectors::get_at(ctx.db, x, y).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            out.push_str(&format!("1 {}: nothing there\n", ctx.format_xy(x, y)));
            return;
        }
        Err(e) => {
            out.push_str(&format!("1 DB error: {e}\n"));
            return;
        }
    };

    if sect.sector_type == SectorType::Sea {
        out.push_str(&format!("1 flying over water at {}\n", ctx.format_xy(x, y)));
        if sweep_mode && sect.mines > 0 {
            sweep_mines(ctx, &mut sect, spy, chrs, rng, out).await;
        }
        return;
    }

    let flags = combined_caps(spy, escort, chrs);
    if flags.contains(PlaneChrFlags::SPY) {
        let acc = if flags.contains(PlaneChrFlags::IMAGE) { 10 } else { 50 };
        out.push_str("1 SPY Plane report\n");
        out.push_str(&format_sect_row(&sect, acc, ctx));

        let all_ships = ships::get_all(ctx.db).await.unwrap_or_default();
        for sh in all_ships.iter().filter(|s| s.own != 0 && s.x == x && s.y == y) {
            let type_name = ShipChr::for_type(sh.ship_type as usize).map(|c| c.name).unwrap_or("??");
            out.push_str(&format!(
                "1 {:4} {:4} {:-16.16} {:-25.25} {} {:3}%\n",
                sh.own, sh.uid, type_name, sh.name, ctx.format_xy(sh.x, sh.y), sh.effic
            ));
        }
        let all_units = land_units::get_all(ctx.db).await.unwrap_or_default();
        for u in all_units.iter().filter(|u| u.own != 0 && u.x == x && u.y == y) {
            if let Some(lchr) = LandChr::for_type(u.land_type as usize) {
                if lchr.flags.contains(LandChrFlags::SPY) { continue; }
                out.push_str(&format!(
                    "1 {:4} {:4} {:-16.16} {} {:3}%\n",
                    u.own, u.uid, lchr.name, ctx.format_xy(u.x, u.y), u.effic
                ));
            }
        }
    } else {
        out.push_str(&format!(
            "1 {}: {} effic ~{}%\n",
            ctx.format_xy(x, y), sect.sector_type.mnemonic(),
            crate::subs::satsub::round_int_by(sect.effic as i32, 25)
        ));
        let all_ships = ships::get_all(ctx.db).await.unwrap_or_default();
        let all_units = land_units::get_all(ctx.db).await.unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for sh in all_ships.iter().filter(|s| s.own != 0 && s.own != ctx.cnum && s.x == x && s.y == y) {
            if seen.insert(("ship", sh.own)) {
                out.push_str(&format!("1 Flying over nation #{}'s ships in {}\n", sh.own, ctx.format_xy(x, y)));
            }
        }
        for u in all_units.iter().filter(|u| u.own != 0 && u.own != ctx.cnum && u.x == x && u.y == y) {
            if seen.insert(("land", u.own)) {
                out.push_str(&format!("1 Flying over nation #{}'s land units in {}\n", u.own, ctx.format_xy(x, y)));
            }
        }
    }
}

/// Ported from plane_sweep() in aircombat.c: each still-alive
/// SWEEP-flagged plane in the mission list gets one roll to clear a
/// mine (probability (100-acc)/100 -- a plane's own accuracy stat,
/// tech-scaled the same way pl_acc()/PLN_ACC() do in 4.4.1, so a more
/// advanced sweeper is *less* likely to trigger one per pass). Stops
/// as soon as the sector runs out of mines, matching the C loop's
/// `mines_there` guard. Only sect.mines changes here -- persisted by
/// the caller only if at least one mine was actually cleared.
async fn sweep_mines(
    ctx: &CmdCtx<'_>,
    sect: &mut empire_types::sector::Sector,
    spy: &[Plane], chrs: &[PlaneChr],
    rng: &mut StdRng,
    out: &mut String,
) {
    let mut cleared = false;
    for p in spy {
        if sect.mines <= 0 {
            break;
        }
        if p.effic <= 0 {
            continue;
        }
        let Some(chr) = chrs.get(p.plane_type as usize) else { continue };
        if !chr.flags.contains(PlaneChrFlags::SWEEP) {
            continue;
        }
        let acc = pln_acc(p, chr);
        let sweep_chance = ((100.0 - acc) / 100.0).clamp(0.0, 1.0);
        if rng.gen_bool(sweep_chance) {
            sect.mines -= 1;
            cleared = true;
            out.push_str(&format!("1 Sweep! in {}\n", ctx.format_xy(sect.x, sect.y)));
        }
    }
    if cleared {
        if let Err(e) = sectors::put(ctx.db, sect).await {
            out.push_str(&format!("1 Warning: mine count save error: {e}\n"));
        }
    }
}

/// Ported from pl_acc()/PLN_ACC() in global/plane.c: bombing/sweep
/// accuracy scaled down by how far the plane's build-time tech level
/// (`plane.tech`) exceeds the type's minimum required tech
/// (`chr.tech`) -- more tech surplus means a lower effective "acc",
/// which for sweeping specifically means a *higher* chance to trigger
/// a mine (see sweep_mines' probability formula).
fn pln_acc(plane: &Plane, chr: &PlaneChr) -> f64 {
    let surplus = ((plane.tech as i32) - chr.tech).max(0) as f64;
    chr.acc as f64 * (1.0 - (surplus.sqrt() / 50.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::plane::PlaneFlags;

    fn make_plane(plane_type: i8, tech: i16) -> Plane {
        Plane {
            uid: 0, own: 1, x: 0, y: 0, plane_type,
            effic: 100, mobil: 30, off: false, tech,
            wing: ' ', opx: 0, opy: 0, mission: 0, mission_radius: 0,
            range: 10, harden: 0, ship: -1, land: -1,
            flags: PlaneFlags::empty(), access: 0, theta: 0.0,
        }
    }

    #[test]
    fn pln_acc_no_tech_surplus_keeps_base_acc() {
        // anti-sub plane: acc=85, min tech=100
        let chrs = PlaneChr::all();
        let chr = &chrs[14];
        let plane = make_plane(14, 100); // built at exactly the minimum tech
        assert_eq!(pln_acc(&plane, chr), 85.0);
    }

    #[test]
    fn pln_acc_drops_with_tech_surplus() {
        let chrs = PlaneChr::all();
        let chr = &chrs[14];
        // 400 tech surplus: acc * (1 - sqrt(400)/50) = 85 * (1 - 20/50) = 85 * 0.6
        let plane = make_plane(14, 500);
        assert!((pln_acc(&plane, chr) - 51.0).abs() < 1e-9);
    }

    #[test]
    fn pln_acc_ignores_tech_deficit() {
        // Built below the type's minimum tech (shouldn't normally happen,
        // but the C formula clamps the surplus at 0 either way).
        let chrs = PlaneChr::all();
        let chr = &chrs[14];
        let plane = make_plane(14, 0);
        assert_eq!(pln_acc(&plane, chr), 85.0);
    }
}
