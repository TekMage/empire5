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
// Ported from: src/lib/commands/fly.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Ken Stevens, 1995

// "fly" command — move planes to a friendly destination airfield or
// harbor, or land aboard a friendly carrier sitting there.
//
// Usage: fly PLANE-SPEC DEST-SECT
//
// PLANE-SPEC: "*" for all owned planes, a single uid, a uid range
// ("0-5"), a comma list, "~" for planes with no wing, or a single
// letter naming a wing (see 'info wingadd').
// DEST-SECT: destination sector (player-relative "X,Y").
//
// Planes can only land at friendly sectors with airfield or harbor
// types, or aboard a friendly CARRIER-flagged ship sitting at that
// coordinate (>=50% efficient -- see shipcarry::SHIP_AIROPS_EFF).
// Carriers are tried first (matching 4.4.1, which offers carriers
// before falling back to the sector itself); a plane that fits no
// carrier and isn't a valid sector landing has nowhere to go and
// crashes. See 'info fly' for the v1 no-carrier-picker simplification
// (eligible carriers are filled in uid order, no interactive choice).

use empire_db::{planes, sectors, ships};
use empire_types::plane::Plane;
use empire_types::plane_chr::PlaneChr;
use empire_types::sector::SectorType;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::geo::map_dist;
use crate::subs::plnsub::{pln_capable, pln_use_fuel, plane_spec_matches};
use crate::subs::shipcarry;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: fly PLANE-SPEC DEST-SECT\n".to_string();
    }

    let plane_spec = parts[0];
    let Some((rx, ry)) = parse_rel_xy(parts[1]) else {
        return format!("10 Bad sector specification: '{}'\n", parts[1]);
    };
    let dx = ctx.x_abs(rx);
    let dy = ctx.y_abs(ry);

    let dest = match sectors::get_at(ctx.db, dx, dy).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let ships_here = match ships::get_at_xy(ctx.db, dx, dy).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    let ship_chrs = ShipChr::all();
    let carriers = shipcarry::eligible_carriers(&ships_here, ship_chrs, ctx.cnum, ctx.is_deity, dx, dy);

    let sector_ok = dest.as_ref().is_some_and(|s| {
        (s.own == ctx.cnum || ctx.is_deity)
            && matches!(s.sector_type, SectorType::Airfield | SectorType::Harbor)
    });

    if !sector_ok && carriers.is_empty() {
        return match &dest {
            None => format!("10 Sector {} doesn't exist\n", ctx.format_xy(dx, dy)),
            Some(s) if s.own != ctx.cnum && !ctx.is_deity => format!(
                "10 {} is not a friendly sector — planes cannot land there.\n",
                ctx.format_xy(dx, dy),
            ),
            Some(_) => format!(
                "10 {} is not a valid airfield/harbor — planes cannot land there.\n",
                ctx.format_xy(dx, dy),
            ),
        };
    }

    // Load planes
    let all_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let chrs = PlaneChr::all();

    let selected: Vec<_> = all_planes
        .into_iter()
        .filter(|p| p.own == ctx.cnum && plane_spec_matches(plane_spec, p))
        .collect();

    if selected.is_empty() {
        return "10 No planes match that specification.\n".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!(
        "1 Flying {} plane(s) to {}\n",
        selected.len(),
        ctx.format_xy(dx, dy),
    ));

    // Per-carrier manifest, seeded from the DB and updated in-memory as
    // planes land within this same command so capacity is enforced
    // correctly across the whole batch, not just per plane.
    let mut manifests: Vec<Vec<Plane>> = Vec::with_capacity(carriers.len());
    for c in &carriers {
        manifests.push(planes::get_on_ship(ctx.db, c.uid).await.unwrap_or_default());
    }

    let mut flew = 0u32;
    let mut grounded = 0u32;
    let mut crashed = 0u32;

    for mut plane in selected {
        let Some(chr) = chrs.get(plane.plane_type as usize) else {
            out.push_str(&format!("1 Plane #{}: unknown type — skipped\n", plane.uid));
            grounded += 1;
            continue;
        };

        // Check capability
        if !pln_capable(&plane, chr) {
            out.push_str(&format!(
                "1 Plane #{}: not capable of flying (effic {}%, mob {})\n",
                plane.uid, plane.effic, plane.mobil,
            ));
            grounded += 1;
            continue;
        }

        // Range check — use stored chr.range
        let dist = map_dist(plane.x, plane.y, dx, dy, ctx.world_x, ctx.world_y);
        let max_range = chr.range as i32;
        if dist > max_range {
            out.push_str(&format!(
                "1 Plane #{}: out of range (dist {dist}, range {max_range})\n",
                plane.uid,
            ));
            grounded += 1;
            continue;
        }

        // Deduct fuel
        pln_use_fuel(&mut plane, chr, dist);

        // Try carriers first (matches 4.4.1's landing-offer order),
        // then the sector, then the plane has nowhere to land.
        let mut landed_on = None;
        for (i, c) in carriers.iter().enumerate() {
            let Some(c_chr) = ship_chrs.get(c.ship_type as usize) else { continue };
            if shipcarry::ship_can_carry(c_chr, &manifests[i], chrs, chr.flags) {
                landed_on = Some(i);
                break;
            }
        }

        if let Some(i) = landed_on {
            let c = carriers[i];
            plane.ship = c.uid;
            plane.x = c.x;
            plane.y = c.y;
            if let Err(e) = planes::put(ctx.db, &plane).await {
                out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
            } else {
                manifests[i].push(plane.clone());
                out.push_str(&format!("1 Plane #{} landed aboard ship #{}\n", plane.uid, c.uid));
                flew += 1;
            }
        } else if sector_ok {
            plane.x = dx;
            plane.y = dy;
            if let Err(e) = planes::put(ctx.db, &plane).await {
                out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
            } else {
                flew += 1;
            }
        } else {
            plane.effic = 0;
            if let Err(e) = planes::put(ctx.db, &plane).await {
                out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
            } else {
                out.push_str(&format!(
                    "1 Plane #{}: no room to land — crashes and burns\n",
                    plane.uid
                ));
                crashed += 1;
            }
        }
    }

    if flew > 0 {
        out.push_str(&format!("1 {flew} plane(s) landed safely at {}.\n", ctx.format_xy(dx, dy)));
    }
    if grounded > 0 {
        out.push_str(&format!("1 {grounded} plane(s) could not fly.\n"));
    }
    if crashed > 0 {
        out.push_str(&format!("1 {crashed} plane(s) had nowhere to land and crashed.\n"));
    }

    out.push_str("0 fly\n");
    out
}
