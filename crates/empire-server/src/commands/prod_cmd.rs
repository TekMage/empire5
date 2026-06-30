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
// Ported from: src/lib/commands/prod.c

// "production" command — simulate next update's sector output.
// Usage: production [<sector-spec>]
//   Shows projected output, PE, cost, commodity consumption, and
//   the maximum possible output if inputs were unlimited.

use empire_db::{sectors, nations};
use empire_types::commodity::Item;
use empire_types::product_chr::{ProductChr, NatLevel};
use empire_types::sector_chr::{SectorChr, PRD_NONE};
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;
use crate::update::{prod_eff, prod_materials_limit, prod_resource_limit, resource_val, do_feed, build_eff};

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

    // Build nation-level lookup for tech and education
    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(_) => vec![],
    };
    let mut nat_tech = [0.0f64; 256];
    let mut nat_edu  = [0.0f64; 256];
    for n in &all_nations {
        nat_tech[n.cnum as usize] = n.tech;
        nat_edu[n.cnum as usize]  = n.education;
    }

    let rates = &ctx.config.rates;
    let etu   = ctx.etu;

    let mut rows: Vec<String> = Vec::new();
    let mut nsect = 0u32;

    for s in all_sectors {
        if s.own == 0 { continue; }
        if !ctx.is_deity && s.own != ctx.cnum { continue; }
        if !filter.matches(&s, ctx.world_x, ctx.world_y) { continue; }

        let dchr = SectorChr::for_type(s.sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }
        if s.off { continue; }

        // Simulate what the update will do before production:
        // 1. feed population (affects civ count / avail)
        // 2. build efficiency (affects effic and avail)
        let mut sim = s.clone();
        do_feed(&mut sim, etu, rates, false);
        build_eff(&mut sim, dchr);

        if sim.effic < 60 { continue; }

        let xy = ctx.format_xy(s.x, s.y);
        let des = s.sector_type.mnemonic();

        if dchr.is_enlist && sim.old_own == sim.own {
            // Enlistment sector
            let civ    = sim.items.get(Item::Civil);
            let mil    = sim.items.get(Item::Milit);
            let max_mil = (civ / 2 - mil).max(0);
            let enlisted = (((etu as f64) * (10.0 + mil as f64) * 0.05) as i16)
                .min(max_mil)
                .max(0);

            let phys_pe  = sim.effic as f64 / 100.0;
            let prodeff  = 1.0_f64;
            let cost     = enlisted as f64 * 3.0;

            // use1 = civs consumed = enlisted; max1 = same for enlist
            let row = format_row(
                &xy, des, phys_pe, sim.avail, prodeff,
                enlisted as f64, 'm', cost,
                max_mil as f64, 'm',
                &[(enlisted as i32, 'c'), (0, '\0'), (0, '\0')],
                &[(enlisted as i32, 'c'), (0, '\0'), (0, '\0')],
            );
            rows.push(row);
            nsect += 1;
            continue;
        }

        if dchr.prd == PRD_NONE { continue; }
        let prd = match ProductChr::get(dchr.prd) {
            Some(p) => p,
            None => continue,
        };

        let own = sim.own as usize;
        let level = match prd.nlndx {
            Some(NatLevel::Tech)      => nat_tech[own],
            Some(NatLevel::Education) => nat_edu[own],
            _                         => 0.0,
        };
        let prodeff = prod_eff(prd, level, dchr.peff);

        // Physical PE = effic/100 * resource/100 (shown in 'eff' column)
        let res_val  = resource_val(&sim, &prd.resource);
        let phys_pe  = sim.effic as f64 / 100.0 * res_val as f64 / 100.0;

        // Actual output with current materials
        let (real, take) = prod_output_sim(&sim, prd, prodeff);

        // Max output with unlimited materials (output item set to 0)
        let (maxr, mtake) = {
            let mut scratch = sim.clone();
            for inp in prd.inputs.iter().flatten() {
                scratch.items.set(inp.item, 9999);
            }
            if let Some(out_item) = prd.item {
                scratch.items.set(out_item, 0);
            }
            prod_output_sim(&scratch, prd, prodeff)
        };

        let cost = take * prd.cost as f64;

        // Commodity use columns (up to 3 inputs)
        let mut cuse = [(0i32, '\0'); 3];
        let mut cmax = [(0i32, '\0'); 3];
        for (i, inp) in prd.inputs.iter().enumerate().take(3) {
            if let Some(inp) = inp {
                let mnem = inp.item.mnemonic();
                cuse[i] = ((take  * inp.amount as f64).ceil() as i32, mnem);
                cmax[i] = ((mtake * inp.amount as f64).ceil() as i32, mnem);
            }
        }

        // Output mnemonic: '.' for level production, item mnemonic otherwise
        let (make_mnem, max_mnem) = match prd.item {
            Some(item) => { let m = item.mnemonic(); (m, m) }
            None       => ('.', '.'),
        };

        let row = format_row(
            &xy, des, phys_pe, sim.avail, prodeff,
            real, make_mnem, cost,
            maxr, max_mnem,
            &cuse, &cmax,
        );
        rows.push(row);
        nsect += 1;
    }

    let mut out = String::new();
    if nsect == 0 {
        out.push_str(&format!("1 {spec_str}: No sector(s)\n"));
        out.push_str("0 production\n");
        return out;
    }

    out.push_str("1 PRODUCTION SIMULATION\n");
    out.push_str("1    sect  des eff avail  make  p.e. cost    use1 use2 use3   max1 max2 max3    max\n");
    for r in rows {
        out.push_str(&format!("1 {r}\n"));
    }
    out.push_str(&format!("0 {nsect} sector{}\n", if nsect == 1 { "" } else { "s" }));
    out
}

/// Read-only production simulation — mirrors prod_output() but never mutates.
/// Returns (output, material_consume).
fn prod_output_sim(
    s: &empire_types::sector::Sector,
    prd: &ProductChr,
    p_e: f64,
) -> (f64, f64) {
    if s.avail <= 0 || p_e <= 0.0 { return (0.0, 0.0); }

    let material_limit = prod_materials_limit(s, prd);
    let unit_work      = prd.bwork.max(1) as f64;
    let res_factor     = resource_val(s, &prd.resource) as f64 / 100.0;
    let worker_limit   = s.avail as f64 * (s.effic as f64 / 100.0) * res_factor / unit_work;
    let res_limit      = prod_resource_limit(s, prd);

    let material_consume = material_limit.min(worker_limit).min(res_limit);
    if material_consume <= 0.0 { return (0.0, 0.0); }

    let output = material_consume * p_e;

    let output = if let Some(item) = prd.item {
        let out_floor = output.floor();
        let room = (9999.0 - s.items.get(item) as f64).max(0.0);
        out_floor.min(room)
    } else {
        output
    };

    (output, material_consume)
}

#[allow(clippy::too_many_arguments)]
fn format_row(
    xy: &str,
    des: char,
    phys_pe: f64,
    avail: i16,
    prodeff: f64,
    make: f64,
    make_mnem: char,
    cost: f64,
    maxr: f64,
    max_mnem: char,
    cuse: &[(i32, char); 3],
    cmax: &[(i32, char); 3],
) -> String {
    // make column: "%4.0fX" or "%5.2f" for level (mnem='.')
    let make_col = if make_mnem == '.' {
        format!("{:5.2}", make)
    } else {
        format!("{:4.0}{}", make, make_mnem)
    };

    // max column: same
    let max_col = if max_mnem == '.' {
        format!("{:5.2}", maxr)
    } else {
        format!("{:5.0}", maxr)
    };

    // use and max columns: "%4d%c" or 5 spaces if empty
    let fmt_use = |entries: &[(i32, char); 3]| -> String {
        entries.iter().map(|&(amt, mn)| {
            if mn == '\0' { "     ".to_string() }
            else          { format!("{:4}{}", amt, mn) }
        }).collect::<Vec<_>>().join("")
    };

    format!(
        "{:9} {} {:3.0}% {:5} {:5} {:4.2} ${:<5.0}  {}  {}  {:5}",
        xy, des,
        phys_pe * 100.0,
        avail,
        make_col,
        prodeff,
        cost,
        fmt_use(cuse),
        fmt_use(cmax),
        max_col,
    )
}
