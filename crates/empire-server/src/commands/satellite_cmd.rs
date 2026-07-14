// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/sate.c, src/lib/subs/satmap.c

// "satellite" / "sat" command — generate a report from an orbiting
// satellite: a spy report (sector/ship/land unit intel, if the plane
// has SPY capability), an imaging map (real terrain regardless of
// range, if the plane has IMAGE capability), or both.
//
// Usage: satellite PLANE-UID [sect|ship|land]
//
// The satellite must actually be in orbit (launched via 'launch') and
// have fully regenerated its mobility since launch — see 'info launch'
// and 'info satellite'.
//
// v1 gap: anti-satellite missiles (shooting down an enemy's satellite)
// are not implemented — see 'info satellite'.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use empire_db::{bmap, land_units, planes, sectors, ships};
use empire_types::commodity::Item;
use empire_types::land_chr::{LandChr, LandChrFlags};
use empire_types::plane_chr::{PlaneChr, PlaneChrFlags};
use empire_types::sector::{Sector, SectorType};
use empire_types::ship_chr::{ShipChr, ShipChrFlags};

use super::ctx::CmdCtx;
use super::radar_cmd::{build_coord_map, render_radar_border, seed_bmap_if_blank};
use crate::subs::geo::{map_dist, x_norm, xy_in_range, xydist_range, y_norm};
use crate::subs::satsub::{is_noisy_slot, round_int_by, sat_is_in_orbit, sat_is_ready, sat_range};

#[derive(PartialEq)]
enum ReportType { All, Sect, Ship, Land }

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return "10 Usage: satellite PLANE-UID [sect|ship|land]\n".to_string();
    }
    let Ok(pln_uid) = parts[0].parse::<i32>() else {
        return format!("10 Bad plane uid '{}'\n", parts[0]);
    };
    let report_type = match parts.get(1).copied() {
        None => ReportType::All,
        Some("sect") => ReportType::Sect,
        Some("ship") => ReportType::Ship,
        Some("land") => ReportType::Land,
        Some(other) => return format!("10 Unknown report type '{other}' (want sect|ship|land)\n"),
    };

    let plane = match planes::get(ctx.db, pln_uid).await {
        Ok(Some(p)) => p,
        Ok(None) => return "10 No such plane\n".to_string(),
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    if plane.own != ctx.cnum && !ctx.is_deity {
        return format!("10 You don't own plane #{pln_uid}\n");
    }
    let Some(chr) = PlaneChr::for_type(plane.plane_type as usize) else {
        return "10 Unknown plane type\n".to_string();
    };
    if !sat_is_in_orbit(&plane, chr) {
        return format!("10 {} isn't in orbit\n", chr.name);
    }
    let plane_mob_max = ctx.config.rates.plane_mob_max;
    if !sat_is_ready(&plane, plane_mob_max) {
        return format!(
            "10 {} doesn't have enough mobility (needs {plane_mob_max})\n", chr.name
        );
    }

    let spy = chr.flags.contains(PlaneChrFlags::SPY);
    let image = chr.flags.contains(PlaneChrFlags::IMAGE);

    let mut out = String::new();
    out.push_str(if spy {
        "1 Satellite Spy Report:\n"
    } else {
        "1 Satellite Map Report:\n"
    });

    let range = sat_range(plane.tech as f64, plane.effic);
    out.push_str(&format!(
        "1 {} at {} efficiency {}%, max range {range}\n",
        chr.name, ctx.format_xy(plane.x, plane.y), plane.effic
    ));
    if plane.effic < 100 {
        out.push_str("1 Some noise on the transmission...\n");
    }

    let wx = ctx.world_x;
    let wy = ctx.world_y;
    let cx = plane.x;
    let cy = plane.y;
    let scan_range = xydist_range(cx, cy, range, wx, wy);

    // Grid for the combined map render (only built/printed in ReportType::All).
    let display_range = range;
    let disp_lx = x_norm(cx - (2 * display_range) as i16, wx);
    let disp_ly = y_norm(cy - display_range as i16, wy);
    let disp_w  = (4 * display_range + 1).min(wx) as usize;
    let disp_h  = (2 * display_range + 1).min(wy) as usize;
    let mut grid: Vec<Vec<char>> = vec![vec![' '; disp_w]; disp_h];

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let coord_map = build_coord_map(&all_sectors);

    let mut bm = match bmap::get_bmap(ctx.db, ctx.cnum, wx as usize, wy as usize).await {
        Ok(b) => b,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    seed_bmap_if_blank(&mut bm, &all_sectors, ctx.cnum);

    // ── Sector pass: fills both the text report and the map grid ──────────
    if matches!(report_type, ReportType::All | ReportType::Sect) {
        let acc = if image { 5 } else { 50 };
        let mut crackle = 0usize;
        let mut count = 0u32;
        if spy {
            out.push_str("1 Satellite sector report\n");
            out.push_str("1                     sct rd  rl  def\n");
            out.push_str("1    sect   type own  eff eff eff eff  civ  mil  shl  gun iron  pet  food\n");
        }

        for s in &all_sectors {
            let dist = map_dist(cx, cy, s.x, s.y, wx, wy);
            if dist > range || !xy_in_range(s.x, s.y, &scan_range) { continue; }

            let noisy = { let c = crackle; crackle = (crackle + 1) % 100; is_noisy_slot(c, plane.effic) };

            if spy && s.own != 0 && s.own != ctx.cnum && !noisy {
                out.push_str(&format_sect_row(s, acc, ctx));
                count += 1;
            }

            if matches!(report_type, ReportType::All) {
                let row = y_norm(s.y - disp_ly, wy) as usize;
                let col = x_norm(s.x - disp_lx, wx) as usize;
                if row < disp_h && col < disp_w {
                    let ch = if image || s.sector_type == SectorType::Sea || s.sector_type == SectorType::Mountain {
                        s.sector_type.mnemonic()
                    } else {
                        '?'
                    };
                    grid[row][col] = ch;
                    bm.set(s.x, s.y, ch as u8);
                }
            }
        }
        if spy {
            out.push_str(&format!("1   {count} sectors\n\n"));
        }
    }

    // ── Ship pass ───────────────────────────────────────────────────────
    if matches!(report_type, ReportType::All | ReportType::Ship) && (spy || image) {
        let all_ships = match ships::get_all(ctx.db).await {
            Ok(v) => v,
            Err(e) => return format!("10 DB error: {e}\n"),
        };
        let mut crackle = 0usize;
        let mut count = 0u32;
        if spy {
            out.push_str("1 Satellite ship report\n");
            out.push_str("1  own shp# ship type                                   sector   eff\n");
        }
        for sh in &all_ships {
            if sh.own == 0 { continue; }
            let dist = map_dist(cx, cy, sh.x, sh.y, wx, wy);
            if dist > range || !xy_in_range(sh.x, sh.y, &scan_range) { continue; }

            let is_sub = ShipChr::for_type(sh.ship_type as usize)
                .map(|c| c.flags.contains(ShipChrFlags::SUBMARINE))
                .unwrap_or(false);
            if is_sub && !(spy && image) { continue; }

            let noisy = { let c = crackle; crackle = (crackle + 1) % 100; is_noisy_slot(c, plane.effic) };
            if noisy { continue; }

            if spy {
                let type_name = ShipChr::for_type(sh.ship_type as usize).map(|c| c.name).unwrap_or("??");
                out.push_str(&format!(
                    "1 {:4} {:4} {:-16.16} {:-25.25} {} {:3}%\n",
                    sh.own, sh.uid, type_name, sh.name, ctx.format_xy(sh.x, sh.y), sh.effic
                ));
                count += 1;
            }
            if matches!(report_type, ReportType::All) && image {
                let row = y_norm(sh.y - disp_ly, wy) as usize;
                let col = x_norm(sh.x - disp_lx, wx) as usize;
                if row < disp_h && col < disp_w {
                    let type_name = ShipChr::for_type(sh.ship_type as usize).map(|c| c.sname).unwrap_or("?");
                    let blip = type_name.chars().next().unwrap_or('?').to_ascii_uppercase();
                    grid[row][col] = blip;
                    bm.set(sh.x, sh.y, blip as u8);
                }
            }
        }
        if spy {
            out.push_str(&format!("1   {count} ships\n\n"));
        }
    }

    // ── Land unit pass ──────────────────────────────────────────────────
    if matches!(report_type, ReportType::All | ReportType::Land) && (spy || image) {
        let all_units = match land_units::get_all(ctx.db).await {
            Ok(v) => v,
            Err(e) => return format!("10 DB error: {e}\n"),
        };
        let mut rng = StdRng::from_entropy();
        let mut crackle = 0usize;
        let mut count = 0u32;
        if spy {
            out.push_str("1 Satellite unit report\n");
            out.push_str("1  own lnd# unit type         sector   eff\n");
        }
        for u in &all_units {
            if u.own == 0 { continue; }
            let dist = map_dist(cx, cy, u.x, u.y, wx, wy);
            if dist > range || !xy_in_range(u.x, u.y, &scan_range) { continue; }

            let Some(lchr) = LandChr::for_type(u.land_type as usize) else { continue };
            if lchr.flags.contains(LandChrFlags::SPY) { continue; }
            if !rng.gen_bool((u.effic as f64 / 20.0).clamp(0.0, 1.0)) { continue; }

            let noisy = { let c = crackle; crackle = (crackle + 1) % 100; is_noisy_slot(c, plane.effic) };
            if noisy { continue; }

            if spy {
                out.push_str(&format!(
                    "1 {:4} {:4} {:-16.16} {} {:3}%\n",
                    u.own, u.uid, lchr.name, ctx.format_xy(u.x, u.y), u.effic
                ));
                count += 1;
            }
            if matches!(report_type, ReportType::All) && image {
                let row = y_norm(u.y - disp_ly, wy) as usize;
                let col = x_norm(u.x - disp_lx, wx) as usize;
                if row < disp_h && col < disp_w {
                    let blip = lchr.sname.chars().next().unwrap_or('?').to_ascii_uppercase();
                    grid[row][col] = blip;
                    bm.set(u.x, u.y, blip as u8);
                }
            }
        }
        if spy {
            out.push_str(&format!("1   {count} units\n\n"));
        }
    }

    // ── Combined map render ─────────────────────────────────────────────
    if matches!(report_type, ReportType::All) {
        let center_row = y_norm(cy - disp_ly, wy) as usize;
        let center_col = x_norm(cx - disp_lx, wx) as usize;
        if center_row < disp_h && center_col < disp_w {
            grid[center_row][center_col] = '0';
            bm.set(cx, cy, b'0');
        }

        out.push_str("1 Satellite radar report\n");
        let rel_lx = ctx.x_rel(disp_lx) as i32;
        render_radar_border(&mut out, rel_lx, disp_w, wx);
        for row in 0..disp_h {
            let abs_y = y_norm(disp_ly + row as i16, wy);
            let rel_y = ctx.y_rel(abs_y);
            let row_str: String = grid[row].iter().collect();
            out.push_str(&format!("1 {:4} {} {}\n", rel_y, row_str, rel_y));
        }
        render_radar_border(&mut out, rel_lx, disp_w, wx);
        out.push_str("1\n1 (c) 1989 Imaginative Images Inc.\n");
    }

    if let Err(e) = bmap::put_bmap(ctx.db, ctx.cnum, &bm).await {
        out.push_str(&format!("1 Warning: could not save bmap: {e}\n"));
    }

    out.push_str("0 satellite\n");
    out
}

pub(crate) fn format_sect_row(s: &Sector, acc: i32, ctx: &CmdCtx<'_>) -> String {
    let half = (acc / 2).max(1);
    let civ = s.items.get(Item::Civil) as i32;
    let mil = s.items.get(Item::Milit) as i32;
    let shell = s.items.get(Item::Shell) as i32;
    let gun = s.items.get(Item::Gun) as i32;
    let iron = s.items.get(Item::Iron) as i32;
    let pet = s.items.get(Item::Petrol) as i32;
    let food = s.items.get(Item::Food) as i32;
    // road/rail/defense aren't modeled on Sector yet (same simplification
    // dump.rs's sector dump already makes) — always report 0.
    format!(
        "1 {}   {}  {:3}  {:3} {:3} {:3} {:3} {:4} {:4} {:4} {:4} {:4} {:4} {:5}\n",
        ctx.format_xy(s.x, s.y), s.sector_type.mnemonic(), s.own,
        round_int_by(s.effic as i32, half), round_int_by(0, half),
        round_int_by(0, half), round_int_by(0, half),
        round_int_by(civ, acc), round_int_by(mil, acc), round_int_by(shell, acc),
        round_int_by(gun, acc), round_int_by(iron, acc), round_int_by(pet, acc),
        round_int_by(food, acc),
    )
}
