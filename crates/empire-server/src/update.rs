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
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/server/update.c, src/lib/update/main.c,
//              src/lib/update/mobility.c, src/lib/update/populace.c,
//              src/lib/update/prepare.c, src/lib/update/produce.c,
//              src/lib/update/nat.c, src/lib/update/age.c
// Known contributors to the original:
//    Dave Pare, 1986, 1994
//    Steve McClure, 1996-1999
//    Doug Hay, 1998
//    Ron Koenderink, 2005
//    Markus Armbruster, 2004-2021

// Update engine — runs the full ETU tick under the exclusive write lock.
//
// Sequence (mirrors update_main() in src/lib/update/main.c):
//
//   1. journal_update
//   2. init per-nation budget (money, level counters, civ counts)
//   3. prepare_sects  (tax, bank income, pay reserve, feed, populace)
//   4. prod_ship/plane/land (0 = maintenance pass)
//   5. produce_sect   (sector production)
//   6. prod_ship/plane/land (1 = build pass)
//   7. finish_sects   (avail rollover)
//   8. prod_nat       (tech/res/edu/hap accumulation)
//   9. age_levels     (tech/res decay, best-tech floor)
//  10. mob_inc_all    (mobility accrual for all game objects)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{info, warn};

use chrono::{Local, Duration as ChronoDuration};
use empire_config::{Config, UpdateConfig, UpdateRates};
use empire_types::commodity::Item;
use empire_types::nation::NatStatus;
use empire_types::product_chr::{NatLevel, ProductChr, Resource};
use empire_types::sector::SectorType;
use empire_types::sector_chr::{SectorChr, PRD_NONE};
use empire_types::MAX_NATIONS;

use crate::journal::Journal;
use crate::state::GameState;

// ── Budget (per-nation accounting for one tick) ───────────────────────────────
//
// Mirrors `struct budget` in include/update.h

#[derive(Debug, Default, Clone)]
pub struct BudgetItem {
    pub count: i64,
    pub money: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Budget {
    /// Money at start of tick (for delta reporting).
    pub start_money: f64,
    /// Running money balance (taxes in, maintenance out).
    pub money: f64,
    /// Total old-owner civilians (used for hap/edu denominator).
    pub oldowned_civs: i64,
    /// Per-level production accumulator (indexed by NatLevel as usize).
    pub level: [f64; 4],
    // Budget line items
    pub civ:   BudgetItem,
    pub mil:   BudgetItem,
    pub uw:    BudgetItem,
    pub bars:  BudgetItem,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the update loop indefinitely.  Called as a Tokio task from main.
///
/// If `config.server.schedule_file` exists, update times are read from it
/// (port of `rdsched.c` via `empire_config::rdsched`).  Otherwise the loop
/// falls back to the fixed `update_interval_secs` interval.
pub async fn run_update_loop(
    state: Arc<RwLock<GameState>>,
    cfg: UpdateConfig,
    journal: Arc<Journal>,
    config: Arc<Config>,
) {
    let fallback_secs = cfg.update_interval_secs.max(60);
    info!(fallback_secs, "Update engine started");

    loop {
        // ── Determine how long to sleep until the next update ────────────────
        let sleep_dur = next_update_sleep(&config, fallback_secs);
        info!(sleep_secs = sleep_dur.as_secs(), "Next update scheduled");
        time::sleep(sleep_dur).await;

        // ── Run the update under the exclusive write lock ────────────────────
        let mut gs = state.write().await;
        gs.update_number += 1;
        let tick = gs.update_number;
        let etu = config.game.etu_per_update;

        info!(tick, etu, "Update tick starting");
        journal.update(tick);

        if let Err(e) = update_main(&mut gs, etu, &config.rates).await {
            warn!(tick, error = %e, "Update tick error");
        }

        info!(tick, "Update tick complete");
        drop(gs);
    }
}

/// Return how long to sleep before the next update.
///
/// Reads the schedule file if it exists and is parseable.  Falls back to
/// `fallback_secs` when the file is absent, empty, or cannot be parsed.
fn next_update_sleep(config: &Config, fallback_secs: u64) -> Duration {
    let sched_path = &config.server.schedule_file;
    if sched_path.as_os_str().is_empty() || !sched_path.exists() {
        return Duration::from_secs(fallback_secs);
    }

    // Need at least 30 s lead time so we don't race the previous update tick.
    let now   = Local::now();
    let after = now + ChronoDuration::seconds(30);
    // anchor: current time rounded up to the next minute (C convention)
    let anchor = {
        let secs = now.timestamp();
        let rounded = (secs + 59) / 60 * 60;
        match chrono::DateTime::from_timestamp(rounded, 0) {
            Some(utc) => utc.with_timezone(&Local),
            None => now,
        }
    };

    match empire_config::rdsched::read_schedule(sched_path, after, anchor, 16) {
        Ok(sched) if !sched.is_empty() => {
            let next = sched[0];
            let remaining = (next - now).to_std().unwrap_or(Duration::from_secs(fallback_secs));
            info!(
                next_update = %next.format("%Y-%m-%d %H:%M:%S"),
                secs = remaining.as_secs(),
                "Schedule file: next update"
            );
            remaining
        }
        Ok(_) => {
            warn!(path = %sched_path.display(), "Schedule file has no upcoming updates — using fallback interval");
            Duration::from_secs(fallback_secs)
        }
        Err(e) => {
            warn!(path = %sched_path.display(), error = %e, "Cannot read schedule file — using fallback interval");
            Duration::from_secs(fallback_secs)
        }
    }
}

async fn update_main(
    gs: &mut GameState,
    etu: i32,
    rates: &UpdateRates,
) -> Result<(), empire_db::DbError> {
    // 1. Load all active nations into local budget array
    let nations = empire_db::nations::get_all(&gs.db).await?;
    let mut budgets: Vec<Budget> = vec![Budget::default(); MAX_NATIONS + 1];

    for nat in &nations {
        if nat.status < NatStatus::Active { continue; }
        let b = &mut budgets[nat.cnum as usize];
        b.start_money = nat.money as f64;
        b.money       = nat.money as f64;
    }

    // 2. prepare_sects — tax, bank income, pay reserve, populace
    let mut sectors = empire_db::sectors::get_all(&gs.db).await?;
    prepare_sects(&mut sectors, &mut budgets, &nations, etu, rates);
    for nat in &nations {
        if nat.status < NatStatus::Active { continue; }
        pay_reserve(nat, &mut budgets[nat.cnum as usize], etu, rates);
    }

    // 3. produce_sect — sector production cycle
    produce_sects(&mut sectors, &mut budgets, &nations, etu, rates);

    // 4. finish_sects — avail rollover clamp
    finish_sects(&mut sectors, rates);

    // 5. prod_nat — accumulate tech/res/edu/hap levels
    let mut nations_mut = nations.clone();
    prod_nat(&mut nations_mut, &mut budgets, etu, rates);

    // 6. age_levels — tech/res decay + best-tech floor
    age_levels(&mut nations_mut, etu, rates);

    // 7. mob_inc_all — mobility accrual
    let mut ships      = empire_db::ships::get_all(&gs.db).await?;
    let mut planes     = empire_db::planes::get_all(&gs.db).await?;
    let mut land_units = empire_db::land_units::get_all(&gs.db).await?;
    mob_inc_all(&mut sectors, &mut ships, &mut planes, &mut land_units, etu, rates);

    // 8. Persist everything back to DB
    for nat in &nations_mut {
        empire_db::nations::put(&gs.db, nat).await?;
    }
    for sec in &sectors {
        empire_db::sectors::put(&gs.db, sec).await?;
    }
    for shp in &ships {
        empire_db::ships::put(&gs.db, shp).await?;
    }
    for pln in &planes {
        empire_db::planes::put(&gs.db, pln).await?;
    }
    for lnd in &land_units {
        empire_db::land_units::put(&gs.db, lnd).await?;
    }

    Ok(())
}

// ── Mobility (mobility.c) ─────────────────────────────────────────────────────

fn mob_inc_all(
    sectors:    &mut [empire_types::sector::Sector],
    ships:      &mut [empire_types::ship::Ship],
    planes:     &mut [empire_types::plane::Plane],
    land_units: &mut [empire_types::land::LandUnit],
    etu: i32,
    rates: &UpdateRates,
) {
    for s in sectors.iter_mut()    { mob_inc_sect(s, etu, rates); }
    for s in ships.iter_mut()      { mob_inc_ship(s, etu, rates); }
    for p in planes.iter_mut()     { mob_inc_plane(p, etu, rates); }
    for l in land_units.iter_mut() { mob_inc_land(l,  etu, rates); }
}

fn mob_inc_sect(s: &mut empire_types::sector::Sector, etu: i32, rates: &UpdateRates) {
    if s.own == 0 { return; }
    let dchr = SectorChr::for_type(s.sector_type);
    if dchr.is_water || dchr.is_sanct { return; }

    let value = s.mobil as i32 + (etu as f32 * rates.sect_mob_scale) as i32;
    s.mobil = value.min(rates.sect_mob_max).min(127) as i8;
}

fn mob_inc_ship(s: &mut empire_types::ship::Ship, etu: i32, rates: &UpdateRates) {
    if s.own == 0 { return; }
    let value = s.mobil as i32 + (etu as f32 * rates.ship_mob_scale) as i32;
    s.mobil = value.min(rates.ship_mob_max).min(127) as i8;
}

fn mob_inc_plane(p: &mut empire_types::plane::Plane, etu: i32, rates: &UpdateRates) {
    if p.own == 0 { return; }
    let value = p.mobil as i32 + (etu as f32 * rates.plane_mob_scale) as i32;
    p.mobil = value.min(rates.plane_mob_max).min(127) as i8;
}

fn mob_inc_land(l: &mut empire_types::land::LandUnit, etu: i32, rates: &UpdateRates) {
    if l.own == 0 { return; }
    let value = l.mobil as i32 + (etu as f32 * rates.land_mob_scale) as i32;
    l.mobil = value.min(rates.land_mob_max).min(127) as i8;
}

// ── Populace (populace.c) ─────────────────────────────────────────────────────

/// Remove unowned sectors with no population.
/// Mirrors check_pop_loss() in src/lib/update/populace.c
fn check_pop_loss(s: &mut empire_types::sector::Sector) {
    let civ = s.items.get(Item::Civil);
    let mil = s.items.get(Item::Milit);
    if civ == 0 {
        s.work = 100;
        s.loyal = 0;
        s.old_own = s.own;
    }
    if s.own != 0 && civ == 0 && mil == 0 {
        s.own = 0;
        s.old_own = 0;
        s.mobil = 0;
    }
}

/// Compute available worker-ETUs for a sector.
/// Mirrors total_work() in src/lib/update/populace.c
fn total_work(sctwork: u8, etu: i32, civil: i16, milit: i16, uw: i16, maxpop: i32) -> i32 {
    let c = (civil as i32).min(maxpop) as f64;
    let m = (milit as i32).min(maxpop) as f64;
    let u = (uw as i32).min(maxpop) as f64;
    round_avg((c * sctwork as f64 / 100.0 + m / 2.5 + u) * etu as f64 / 100.0)
}

// ── Prepare sects (prepare.c) ─────────────────────────────────────────────────

fn prepare_sects(
    sectors: &mut [empire_types::sector::Sector],
    budgets: &mut [Budget],
    nations: &[empire_types::nation::Nation],
    etu: i32,
    rates: &UpdateRates,
) {
    for s in sectors.iter_mut() {
        let dchr = SectorChr::for_type(s.sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }

        let own = s.own as usize;

        // Compute available workforce for this sector
        let maxpop = dchr.maxpop.max(1);
        let civ  = s.items.get(Item::Civil);
        let mil  = s.items.get(Item::Milit);
        let uw   = s.items.get(Item::Uw);
        let work = total_work(s.work, etu, civ, mil, uw, maxpop);
        // Clamp avail: existing rollover (capped) + new work
        s.avail = (s.avail / 2 + work as i16).min(10000);

        if own == 0 { continue; }

        // Tax civilians
        let civ_tax = if s.old_own == s.own {
            civ as f64 * etu as f64 * rates.money_civ * s.effic as f64 / 100.0
        } else {
            civ as f64 * etu as f64 * rates.money_civ * s.effic as f64 / 100.0 / 4.0
        };
        if s.old_own == s.own {
            budgets[own].oldowned_civs += civ as i64;
        }
        budgets[own].civ.count += civ as i64;
        budgets[own].civ.money += civ_tax;
        budgets[own].money     += civ_tax;

        // Tax uncompensated workers
        let uw_tax = uw as f64 * etu as f64 * rates.money_uw * s.effic as f64 / 100.0;
        budgets[own].uw.count += uw as i64;
        budgets[own].uw.money += uw_tax;
        budgets[own].money    += uw_tax;

        // Pay military (negative — costs money)
        let mil_pay = mil as f64 * etu as f64 * rates.money_mil;
        budgets[own].mil.count += mil as i64;
        budgets[own].mil.money += mil_pay;
        budgets[own].money     += mil_pay;

        // Bank income
        if s.sector_type == SectorType::Bank {
            let bars   = s.items.get(Item::Bar);
            let income = bars as f64 * etu as f64 * rates.bankint * s.effic as f64 / 100.0;
            budgets[own].bars.count += bars as i64;
            budgets[own].bars.money += income;
            budgets[own].money      += income;
        }

        // Feed the sector (simplified: no starvation; Phase 4 subs will add this)
        // Full do_feed() port deferred to Phase 4.

        check_pop_loss(s);
    }
    // Nations parameter currently unused (future: guerrilla, plague)
    let _ = nations;
}

fn pay_reserve(
    nat: &empire_types::nation::Nation,
    budget: &mut Budget,
    etu: i32,
    rates: &UpdateRates,
) {
    let pay = nat.reserve as f64 * rates.money_res * etu as f64;
    budget.mil.money += pay;
    budget.money     += pay;
}

// ── Sector production (produce.c + sect.c) ───────────────────────────────────

fn produce_sects(
    sectors: &mut [empire_types::sector::Sector],
    budgets: &mut [Budget],
    nations:  &[empire_types::nation::Nation],
    etu: i32,
    rates: &UpdateRates,
) {
    // Build a nation-level lookup (cnum → tech level for produce())
    let mut nat_tech  = [0.0f64; MAX_NATIONS + 1];
    let mut nat_edu   = [0.0f64; MAX_NATIONS + 1];
    for n in nations {
        nat_tech[n.cnum as usize]  = n.tech;
        nat_edu[n.cnum as usize]   = n.education;
    }

    for s in sectors.iter_mut() {
        let own = s.own as usize;
        if own == 0 { continue; }
        let dchr = SectorChr::for_type(s.sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }

        // Sector maintenance cost
        if dchr.maint > 0 {
            let cost = etu as f64 * dchr.maint as f64;
            budgets[own].money -= cost;
        }

        if s.off || budgets[own].money < 0.0 {
            s.avail = 0;
            continue;
        }

        // Efficiency build towards 100% (or towards new_type if redesignated)
        if s.effic < 100 || s.sector_type != s.new_type {
            let build_cost = build_eff(s, dchr);
            budgets[own].money -= build_cost;
        }

        // Enlistment: convert civs → mil
        if dchr.is_enlist && s.effic >= 60 && s.old_own == s.own {
            enlist(s, own, budgets, etu);
        }

        // Production (sector must be >= 60% efficient)
        if s.effic >= 60 && dchr.prd != PRD_NONE {
            if let Some(prd) = ProductChr::get(dchr.prd) {
                let level = match prd.nlndx {
                    Some(NatLevel::Tech)      => nat_tech[own],
                    Some(NatLevel::Education) => nat_edu[own],
                    _                          => 0.0,
                };
                produce(s, own, prd, level, budgets);
            }
        }
    }
}

/// Build up sector efficiency by 1 unit; return cash cost.
/// Mirrors buildeff() in src/lib/update/sect.c
fn build_eff(
    s: &mut empire_types::sector::Sector,
    dchr: &SectorChr,
) -> f64 {
    let avail = s.avail as i32 / 2 * 100;
    let mut cost = 0.0f64;

    if s.sector_type != s.new_type {
        // Tear down: easier than building
        let bwork = if dchr.bwork > 0 { dchr.bwork } else { 1 };
        let build = (4 * avail / bwork).min(s.effic as i32);
        s.effic -= build as i8;
        if s.effic <= 0 {
            s.effic = 0;
            s.sector_type = s.new_type;
        }
        cost += build as f64 / 4.0;
    }

    if s.sector_type == s.new_type {
        let new_dchr = SectorChr::for_type(s.sector_type);
        let bwork = if new_dchr.bwork > 0 { new_dchr.bwork } else { 1 };
        let delta = (avail / bwork).min(100 - s.effic as i32);
        if delta > 0 {
            s.effic += delta as i8;
            cost += delta as f64 * new_dchr.cost as f64 / 100.0;
        }
    }

    let new_avail = (s.avail + 1) / 2 + (avail / 100) as i16;
    s.avail = new_avail;
    cost
}

fn enlist(
    s: &mut empire_types::sector::Sector,
    own: usize,
    budgets: &mut [Budget],
    etu: i32,
) {
    let civ = s.items.get(Item::Civil);
    let mil = s.items.get(Item::Milit);
    let max_mil = civ / 2 - mil;
    if max_mil <= 0 { return; }

    let enlisted = ((etu as f64 * (10.0 + mil as f64) * 0.05) as i16).min(max_mil);
    if enlisted <= 0 { return; }

    s.items.add(Item::Civil, -enlisted);
    s.items.add(Item::Milit,  enlisted);
    budgets[own].money -= enlisted as f64 * 3.0;
}

/// Run production for one sector.
/// Mirrors produce() in src/lib/update/produce.c
fn produce(
    s: &mut empire_types::sector::Sector,
    own: usize,
    prd: &ProductChr,
    level: f64,
    budgets: &mut [Budget],
) {
    let p_e = prod_eff(prd, level);
    if p_e <= 0.0 { return; }

    let output = prod_output(s, prd, p_e);
    if output <= 0.0 { return; }

    let cost = prd.cost as f64 * output / p_e;
    budgets[own].money -= cost;

    if let Some(nat_lev) = prd.level {
        budgets[own].level[nat_lev as usize] += output;
    }
}

/// Return production efficiency for `prd` at `level`.
/// Zero means level is too low.  Mirrors prod_eff() in produce.c
fn prod_eff(prd: &ProductChr, level: f64) -> f64 {
    let level_pe = match prd.nlndx {
        None => 1.0,
        Some(_) => {
            let delta = level - prd.nlmin as f64;
            if delta < 0.0 { return 0.0; }
            let lag = (prd.nllag as f64 + delta).max(1.0);
            delta / lag
        }
    };
    // p_eff not in pchr; products produce at 100% of level_pe
    // (dchr.peff / 100.0 would normally be multiplied in, defaulting to 1.0)
    level_pe
}

/// Compute how much a sector produces in one tick.
/// Mirrors prod_output() in produce.c (simplified: no resource depletion yet)
fn prod_output(
    s: &mut empire_types::sector::Sector,
    prd: &ProductChr,
    p_e: f64,
) -> f64 {
    if s.avail <= 0 { return 0.0; }

    // Material limit: how many units we can make from available inputs
    let material_limit = prod_materials_limit(s, prd);
    let unit_work = prd.bwork.max(1) as f64;
    let worker_limit = s.avail as f64 * (s.effic as f64 / 100.0) / unit_work;
    // Resource limit (natural resource depletion — simplified: use sector field)
    let res_limit = prod_resource_limit(s, prd);

    let material_consume = material_limit.min(worker_limit).min(res_limit);
    if material_consume <= 0.0 { return 0.0; }

    let mut output = material_consume * p_e;

    // If producing an item, floor and clamp to ITEM_MAX - current
    if let Some(item) = prd.item {
        output = output.floor();
        let current = s.items.get(item) as f64;
        let item_max = 9999.0;
        if current + output > item_max {
            output = (item_max - current).max(0.0);
        }
        if output > 0.0 {
            s.items.add(item, output as i16);
            // Consume materials
            consume_materials(s, prd, output / p_e);
        }
    } else {
        // Level production: consume materials
        consume_materials(s, prd, material_consume);
    }

    // Deplete natural resource if applicable
    deplete_resource(s, prd, material_consume);

    // Deduct worker-ETUs used
    let work_used = (unit_work * material_consume / (s.effic as f64 / 100.0).max(0.01)) as i16;
    s.avail = (s.avail - work_used).max(0);

    output
}

fn prod_materials_limit(s: &empire_types::sector::Sector, prd: &ProductChr) -> f64 {
    let mut count = 9999.0f64;
    for input in prd.inputs.iter().flatten() {
        let available = s.items.get(input.item) as f64;
        let n = available / input.amount as f64;
        if n < count { count = n; }
    }
    count
}

fn prod_resource_limit(s: &empire_types::sector::Sector, prd: &ProductChr) -> f64 {
    if prd.nrdep == 0 { return 9999.0; }
    let res_val = resource_val(s, &prd.resource);
    res_val as f64 * 100.0 / prd.nrdep as f64
}

fn resource_val(s: &empire_types::sector::Sector, resource: &Resource) -> u8 {
    match resource {
        Resource::None   => 100,
        Resource::Min    => s.min,
        Resource::Gold   => s.gmin,
        Resource::Fert   => s.fertil,
        Resource::OilRes => s.oil,
        Resource::Uran   => s.uran,
    }
}

fn consume_materials(
    s: &mut empire_types::sector::Sector,
    prd: &ProductChr,
    count: f64,
) {
    for input in prd.inputs.iter().flatten() {
        let consumed = (input.amount as f64 * count).round() as i16;
        s.items.add(input.item, -consumed);
    }
}

fn deplete_resource(
    s: &mut empire_types::sector::Sector,
    prd: &ProductChr,
    count: f64,
) {
    if prd.nrdep == 0 { return; }
    let depletion = round_avg(prd.nrdep as f64 * count / 100.0) as i32;
    match prd.resource {
        Resource::None   => {}
        Resource::Min    => s.min    = (s.min    as i32 - depletion).max(0) as u8,
        Resource::Gold   => s.gmin   = (s.gmin   as i32 - depletion).max(0) as u8,
        Resource::Fert   => s.fertil = (s.fertil as i32 - depletion).max(0) as u8,
        Resource::OilRes => s.oil    = (s.oil    as i32 - depletion).max(0) as u8,
        Resource::Uran   => s.uran   = (s.uran   as i32 - depletion).max(0) as u8,
    }
}

// ── Finish sects (finish.c) ───────────────────────────────────────────────────

fn finish_sects(
    sectors: &mut [empire_types::sector::Sector],
    _rates: &UpdateRates,
) {
    for s in sectors.iter_mut() {
        // Clamp avail rollover to avoid unbounded growth
        if s.avail > 1000 { s.avail = 1000; }
    }
}

// ── Nation level production (nat.c) ──────────────────────────────────────────

/// Accumulate tech/res/edu/hap for all active nations.
/// Mirrors prod_nat() in src/lib/update/nat.c
fn prod_nat(
    nations: &mut [empire_types::nation::Nation],
    budgets: &mut [Budget],
    etu: i32,
    rates: &UpdateRates,
) {
    for nat in nations.iter_mut() {
        if nat.status < NatStatus::Active { continue; }
        let own = nat.cnum as usize;
        let b = &budgets[own];
        let pop = (b.oldowned_civs + 1) as f64;

        // hap_edu: more educated people want more happiness
        let hap_edu = {
            let e = nat.education;
            1.5 - (e + 10.0) / (e + 20.0)
        };

        // Per-ETU production of happiness and education from sector budgets
        let hap_produced = b.level[NatLevel::Happiness as usize]
            * hap_edu * rates.hap_cons as f64 / (pop * etu as f64);
        let edu_produced = b.level[NatLevel::Education as usize]
            * rates.edu_cons as f64 / (pop * etu as f64);

        let hap_produced = limit_level(hap_produced, NatLevel::Happiness, rates, true);
        let edu_produced = limit_level(edu_produced, NatLevel::Education, rates, true);

        // Moving average: weight old value by hap_avg/edu_avg ETUs
        nat.happiness  = (nat.happiness  * rates.hap_avg as f64 + hap_produced * etu as f64)
                       / (rates.hap_avg as f64 + etu as f64);
        nat.education  = (nat.education  * rates.edu_avg as f64 + edu_produced * etu as f64)
                       / (rates.edu_avg as f64 + etu as f64);

        // Tech and research accumulate (logarithm-limited above easy_tech)
        let tlev = limit_level(b.level[NatLevel::Tech as usize],     NatLevel::Tech,     rates, false);
        let rlev = limit_level(b.level[NatLevel::Research as usize], NatLevel::Research, rates, false);
        nat.tech     += tlev;
        nat.research += rlev;

        // Apply money delta
        nat.money = budgets[own].money.round() as i32;
    }
}

/// Apply a logarithmic penalty to levels above the "easy" threshold.
/// Mirrors limit_level() in src/lib/update/nat.c
fn limit_level(level: f64, lev_type: NatLevel, rates: &UpdateRates, is_avg: bool) -> f64 {
    let (easy, log_base) = match lev_type {
        NatLevel::Tech      => (rates.easy_tech as f64, rates.tech_log_base as f64),
        NatLevel::Research  => (0.75f64, 2.0),
        NatLevel::Education => (5.0f64, 4.0),
        NatLevel::Happiness => (5.0f64, 6.0),
    };

    if level <= easy { return level; }

    let above_easy = level - easy;
    let above = if is_avg {
        above_easy / logx(log_base + above_easy, log_base)
    } else {
        logx(above_easy + 1.0, log_base)
    };
    let above = above.min(250.0);
    easy + above.max(0.0)
}

fn logx(d: f64, base: f64) -> f64 {
    if base == 1.0 { d } else { d.log10() / base.log10() }
}

// ── Age levels (age.c) ────────────────────────────────────────────────────────

/// Age tech/research by level_age_rate, and pull weak nations toward the best.
/// Mirrors age_levels() in src/lib/update/age.c
fn age_levels(
    nations: &mut [empire_types::nation::Nation],
    etu: i32,
    rates: &UpdateRates,
) {
    let mut best_tech = 0.0f64;
    let mut best_res  = 0.0f64;

    for nat in nations.iter_mut() {
        if nat.status != NatStatus::Active { continue; }
        if nat.tech     > best_tech { best_tech = nat.tech; }
        if nat.research > best_res  { best_res  = nat.research; }

        if rates.level_age_rate != 0.0 {
            let decay = |level: f64| level * etu as f64 / (100.0 * rates.level_age_rate as f64);
            nat.research -= decay(nat.research);
            nat.tech     -= decay(nat.tech);
        }

        // Age the reserve (1% per 24 ETUs)
        nat.reserve = round_avg(nat.reserve as f64 * (1.0 - etu as f64 / 2400.0)) as i32;
    }

    // Pull nations far below best up slightly
    let floor_tech = best_tech / 5.0;
    let floor_res  = best_res  / 5.0;
    for nat in nations.iter_mut() {
        if nat.status < NatStatus::Sanct || nat.status == NatStatus::Deity { continue; }
        if nat.tech     < floor_tech && rand_chance(0.2) {
            nat.tech     += (floor_tech - nat.tech) / 3.0;
        }
        if nat.research < floor_res && rand_chance(0.2) {
            nat.research += (floor_res - nat.research) / 3.0;
        }
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn round_avg(v: f64) -> i32 {
    let floor = v as i32;
    let frac = v - floor as f64;
    if rand_f64() < frac { floor + 1 } else { floor }
}

fn rand_chance(p: f64) -> bool {
    rand_f64() < p
}

fn rand_f64() -> f64 {
    // Minimal LCG — good enough for non-security probabilistic events.
    // Phase 4 will wire in a seeded PRNG per-sector for reproducible tests.
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(12345);
    let s = STATE.fetch_add(6364136223846793005, Ordering::Relaxed)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    STATE.store(s, Ordering::Relaxed);
    (s >> 11) as f64 / (1u64 << 53) as f64
}

