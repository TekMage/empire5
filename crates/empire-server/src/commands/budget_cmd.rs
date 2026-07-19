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
// Ported from: src/lib/commands/budg.c

// "budget" command — simulate the entire next update for this nation and
// report a cost/income rollup, plus a shortfall breakdown of which of the
// player's producing sectors are capacity-limited and why. This is a
// preview: the real update may differ if bots, other players, or the
// market act between now and then.

use empire_db::{sectors, ships, planes, land_units, nations};
use empire_types::commodity::Item;
use empire_types::nation::NatStatus;
use empire_types::product_chr::{ProductChr, NatLevel};
use empire_types::sector_chr::{SectorChr, PRD_NONE};
use empire_types::MAX_NATIONS;
use super::ctx::CmdCtx;
use crate::update::{
    Budget, bm_idx,
    prepare_sects, pay_reserve, produce_sects, finish_sects,
    prod_ships, ship_produce_ocean, prod_planes, prod_land, prod_nat,
    prod_eff, prod_resource_limit, resource_val,
};

pub async fn run(_args: &str, ctx: &CmdCtx<'_>) -> String {
    let rates = &ctx.config.rates;
    let etu   = ctx.etu;

    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let mut sim_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let mut sim_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let mut sim_planes = match planes::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };
    let mut sim_land = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    // Seed a fresh budget array exactly like update_main does, then run the
    // same (corrected) function sequence on these function-local copies.
    // Nothing here is ever put() back — the whole point is to preview.
    let mut budgets: Vec<Budget> = vec![Budget::default(); MAX_NATIONS + 1];
    for nat in &all_nations {
        if nat.status < NatStatus::Active { continue; }
        let b = &mut budgets[nat.cnum as usize];
        b.start_money = nat.money as f64;
        b.money       = nat.money as f64;
    }

    prepare_sects(&mut sim_sectors, &mut budgets, &all_nations, etu, rates, false);
    for nat in &all_nations {
        if nat.status < NatStatus::Active { continue; }
        pay_reserve(nat, &mut budgets[nat.cnum as usize], etu, rates);
    }
    produce_sects(&mut sim_sectors, &mut budgets, &all_nations, etu, rates, false);
    finish_sects(&mut sim_sectors, rates);
    prod_ships(&mut sim_ships, &mut sim_sectors, &mut budgets, etu, rates);
    ship_produce_ocean(&mut sim_ships, &mut sim_sectors, &budgets, &all_nations, etu);
    prod_planes(&mut sim_planes, &mut sim_sectors, &mut budgets, etu, rates);
    prod_land(&mut sim_land, &mut sim_sectors, &mut budgets, etu, rates);

    let mut sim_nations = all_nations.clone();
    prod_nat(&mut sim_nations, &mut budgets, etu, rates);

    let own = ctx.cnum as usize;
    let b   = &budgets[own];

    let mut out = String::new();
    out.push_str("1 BUDGET PREVIEW — next update, based on current state\n");
    out.push_str("1 (this is a preview, not a guarantee — bot activity, market\n");
    out.push_str("1 changes, or another player's actions before the real update\n");
    out.push_str("1 can change the outcome)\n");

    let mut expenses = 0.0f64;
    let mut income   = 0.0f64;

    // Per-sector-type production
    out.push_str("1 \n");
    out.push_str("1 Sector Type              Production                     Cost\n");
    for (idx, item) in b.prod.iter().enumerate() {
        if item.money == 0.0 { continue; }
        let dchr = SectorChr::for_index(idx);
        let label = if dchr.prd != PRD_NONE {
            if let Some(prd) = ProductChr::get(dchr.prd) {
                format!("{} {}", item.count, prd.sname)
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            "1 {:<25} {:<25}   ${:>10.0}\n",
            dchr.name, label, -item.money
        ));
        expenses -= item.money;
    }

    // Ship/plane/unit/sector building & maintenance
    const BM_LABELS: [(&str, &str); bm_idx::COUNT] = [
        ("Ship building",     "ship"),
        ("Ship maintenance",  "ship"),
        ("Plane building",    "plane"),
        ("Plane maintenance", "plane"),
        ("Unit building",     "unit"),
        ("Unit maintenance",  "unit"),
        ("Sector building",   "sector"),
        ("Sector maintenance","sector"),
    ];
    for (i, (activity, object)) in BM_LABELS.iter().enumerate() {
        let bm = &b.bm[i];
        if bm.money == 0.0 { continue; }
        let plural = if bm.count == 1 { "" } else { "s" };
        out.push_str(&format!(
            "1 {:<25} {:<25}   ${:>10.0}\n",
            activity, format!("{} {}{}", bm.count, object, plural), -bm.money
        ));
        expenses -= bm.money;
    }

    // Military payroll
    if b.mil.money != 0.0 {
        out.push_str(&format!(
            "1 {:<25} {:<25}   ${:>10.0}\n",
            "Military payroll",
            format!("{} mil, {} res", b.mil.count, ctx.nat.reserve),
            -b.mil.money
        ));
        expenses -= b.mil.money;
    }

    out.push_str(&format!("1 Total expenses{:>45}\n", format!("${:.0}", expenses)));

    // Income
    let taxes = b.civ.money + b.uw.money;
    if taxes != 0.0 {
        out.push_str(&format!(
            "1 {:<25} {:<25}   ${:>+10.0}\n",
            "Income from taxes",
            format!("{} civ, {} uw", b.civ.count, b.uw.count),
            taxes
        ));
        income += taxes;
    }
    if b.bars.money != 0.0 {
        out.push_str(&format!(
            "1 {:<25} {:<25}   ${:>+10.0}\n",
            "Income from bars", format!("{} bars", b.bars.count), b.bars.money
        ));
        income += b.bars.money;
    }
    out.push_str(&format!("1 Total income{:>47}\n", format!("${:+.0}", income)));

    let balance = ctx.nat.money as f64;
    let delta   = income - expenses;
    let new_treasury = balance + delta;

    out.push_str(&format!("1 Balance forward{:>44}\n", format!("${:.0}", balance)));
    out.push_str(&format!("1 Estimated delta{:>44}\n", format!("${:+.0}", delta)));
    out.push_str(&format!("1 Estimated new treasury{:>37}\n", format!("${:.0}", new_treasury)));

    if new_treasury < 0.0 {
        out.push_str("1 \n");
        out.push_str("1 After processing sectors, you will be broke!\n");
        out.push_str("1 Sectors will not produce, distribute, or deliver!\n");
    }

    // Idle-capacity / shortfall section — not present in real budg.c, added
    // per request: which of the player's own producing sectors are
    // capacity-limited, and by what.
    out.push_str("1 \n");
    out.push_str("1 CAPACITY SHORTFALLS (this nation's producing sectors)\n");

    let mut nat_tech = 0.0f64;
    let mut nat_edu  = 0.0f64;
    for n in &all_nations {
        if n.cnum as usize == own {
            nat_tech = n.tech;
            nat_edu  = n.education;
        }
    }

    let mut shortfalls: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut limited_sectors = 0u32;
    let mut producing_sectors = 0u32;

    for s in &sim_sectors {
        if s.own as usize != own { continue; }
        let dchr = SectorChr::for_type(s.sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }
        if s.effic < 60 || dchr.prd == PRD_NONE { continue; }
        let prd = match ProductChr::get(dchr.prd) {
            Some(p) => p,
            None => continue,
        };
        producing_sectors += 1;

        let level = match prd.nlndx {
            Some(NatLevel::Tech)      => nat_tech,
            Some(NatLevel::Education) => nat_edu,
            _                          => 0.0,
        };
        let p_e = prod_eff(prd, level, dchr.peff);
        if p_e <= 0.0 {
            limited_sectors += 1;
            *shortfalls.entry("nation level too low (tech/education)".to_string()).or_insert(0) += 1;
            continue;
        }

        // Which single input (if any) is the tightest?
        let mut material_limit = 9999.0f64;
        let mut limiting_item: Option<Item> = None;
        for input in prd.inputs.iter().flatten() {
            let available = s.items.get(input.item) as f64;
            let n = available / input.amount as f64;
            if n < material_limit {
                material_limit = n;
                limiting_item = Some(input.item);
            }
        }
        let res_limit = prod_resource_limit(s, prd);
        let unit_work = prd.bwork.max(1) as f64;
        let res_factor = resource_val(s, &prd.resource) as f64 / 100.0;
        let worker_limit = s.avail as f64 * (s.effic as f64 / 100.0) * res_factor / unit_work;

        let take = material_limit.min(worker_limit).min(res_limit);
        if take > 0.01 { continue; } // producing fine, not a shortfall

        limited_sectors += 1;
        let cause = if take == material_limit && limiting_item.is_some() {
            format!("missing {}", limiting_item.unwrap().mnemonic())
        } else if take == res_limit {
            format!("low natural resource ({:?})", prd.resource)
        } else {
            "insufficient workforce (avail)".to_string()
        };
        *shortfalls.entry(cause).or_insert(0) += 1;
    }

    if producing_sectors == 0 {
        out.push_str("1 No producing sectors.\n");
    } else if limited_sectors == 0 {
        out.push_str(&format!("1 All {producing_sectors} producing sectors are running at capacity.\n"));
    } else {
        let mut causes: Vec<_> = shortfalls.into_iter().collect();
        causes.sort_by(|a, b| b.1.cmp(&a.1));
        for (cause, count) in causes {
            out.push_str(&format!("1 {count} sector{} — {cause}\n", if count == 1 { "" } else { "s" }));
        }
        out.push_str(&format!(
            "1 ({limited_sectors} of {producing_sectors} producing sectors capacity-limited)\n"
        ));
    }

    out.push_str("0 budget\n");
    out
}
