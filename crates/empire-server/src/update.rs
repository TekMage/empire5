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

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Notify, RwLock};
use tokio::time;
use tracing::{debug, info, warn};

use chrono::{Local, Duration as ChronoDuration};
use empire_config::{Config, UpdateConfig, UpdateRates};
use empire_types::commodity::Item;
use empire_types::coords::Coord;
use empire_types::item_chr::ItemChr;
use empire_types::nation::NatStatus;
use empire_types::product_chr::{NatLevel, ProductChr, Resource};
use empire_types::sector::SectorType;
use empire_types::sector_chr::{SectorChr, PRD_NONE};
use empire_types::MAX_NATIONS;

use crate::journal::Journal;
use crate::state::GameState;

// ── Item iteration helper ─────────────────────────────────────────────────────

const ALL_ITEMS: [Item; Item::COUNT] = [
    Item::Civil, Item::Milit, Item::Shell, Item::Gun,   Item::Petrol,
    Item::Iron,  Item::Dust,  Item::Bar,   Item::Food,  Item::Oil,
    Item::Lcm,   Item::Hcm,   Item::Uw,    Item::Rad,
];

// ── Direction table & coordinate helpers ─────────────────────────────────────
//
// Mirrors diroff[] in include/dir.h.  Index = direction (0=center, 1-6=hex
// neighbors, 7=dist-center marker for delivery/distribution).

const DIROFF: [(Coord, Coord); 8] = [
    (0, 0),   // 0 — stop / no-move
    (1, -1),  // 1 — NE
    (2, 0),   // 2 — E
    (1, 1),   // 3 — SE
    (-1, 1),  // 4 — SW
    (-2, 0),  // 5 — W
    (-1, -1), // 6 — NW
    (0, 0),   // 7 — distribute-center (not a hex direction)
];

fn wrap_coord(v: i16, max: i32) -> i16 {
    ((v as i32).rem_euclid(max)) as i16
}

fn neighbor_xy(x: Coord, y: Coord, dir: u8, wx: i32, wy: i32) -> (Coord, Coord) {
    let (dx, dy) = DIROFF[dir as usize & 7];
    (wrap_coord(x + dx, wx), wrap_coord(y + dy, wy))
}

// ── Delivery/distribution movement constants ──────────────────────────────────
const DELIVER_BONUS: f64 = 4.0;
const DIST_BONUS: f64    = 10.0;
const ITEM_MAX: i16      = 9999;

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
    updates_enabled: Arc<AtomicBool>,
    force_update: Arc<Notify>,
    next_update_at: Arc<AtomicU64>,
) {
    let fallback_secs = cfg.update_interval_secs.max(60);
    info!(fallback_secs, "Update engine started");

    loop {
        // ── Determine how long to sleep until the next update ────────────────
        let sleep_dur = next_update_sleep(&config, fallback_secs);
        let wake_at = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap_or_default()
            .as_secs() + sleep_dur.as_secs();
        next_update_at.store(wake_at, Ordering::Relaxed);
        info!(sleep_secs = sleep_dur.as_secs(), "Next update scheduled");

        // Sleep until the scheduled time OR a force-update signal arrives.
        tokio::select! {
            _ = time::sleep(sleep_dur) => {}
            _ = force_update.notified() => {
                info!("Force-update requested — running tick now");
            }
        }

        // ── Check if updates are enabled ─────────────────────────────────────
        if !updates_enabled.load(Ordering::Relaxed) {
            info!("Updates disabled — skipping tick");
            continue;
        }

        // ── Run the update under the exclusive write lock ────────────────────
        let mut gs = state.write().await;
        gs.update_number += 1;
        let tick = gs.update_number;
        let etu = config.game.etu_per_update;

        info!(tick, etu, "Update tick starting");
        journal.update(tick);

        if let Err(e) = update_main(&mut gs, etu, &config).await {
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
    config: &Config,
) -> Result<(), empire_db::DbError> {
    let rates   = &config.rates;
    let world_x = config.game.world_x;
    let world_y = config.game.world_y;
    let verbose = config.update.verbose_update;

    // 1. Load all active nations into local budget array
    let nations = empire_db::nations::get_all(&gs.db).await?;
    let mut budgets: Vec<Budget> = vec![Budget::default(); MAX_NATIONS + 1];

    for nat in &nations {
        if nat.status < NatStatus::Active { continue; }
        let b = &mut budgets[nat.cnum as usize];
        b.start_money = nat.money as f64;
        b.money       = nat.money as f64;
    }

    // 2. prepare_sects — tax, bank income, pay reserve, feed population
    let mut sectors = empire_db::sectors::get_all(&gs.db).await?;
    prepare_sects(&mut sectors, &mut budgets, &nations, etu, rates, verbose);
    for nat in &nations {
        if nat.status < NatStatus::Active { continue; }
        pay_reserve(nat, &mut budgets[nat.cnum as usize], etu, rates);
    }

    // 3. produce_sects — sector production cycle
    produce_sects(&mut sectors, &mut budgets, &nations, etu, rates, verbose);

    // 4. finish: deliver items to neighbors, then distribute via dist centers
    let coord_map: HashMap<(Coord, Coord), usize> = sectors.iter().enumerate()
        .map(|(i, s)| ((s.x, s.y), i))
        .collect();
    do_deliver(&mut sectors, &coord_map, world_x, world_y, verbose);
    do_distribute(&mut sectors, &coord_map, world_x, world_y, verbose);

    // 5. finish_sects — avail rollover clamp
    finish_sects(&mut sectors, rates);

    // 6. prod_nat — accumulate tech/res/edu/hap levels
    let mut nations_mut = nations.clone();
    prod_nat(&mut nations_mut, &mut budgets, etu, rates);

    // 7. age_levels — tech/res decay + best-tech floor
    age_levels(&mut nations_mut, etu, rates);

    // 8. mob_inc_all — mobility accrual
    let mut ships      = empire_db::ships::get_all(&gs.db).await?;
    let mut planes     = empire_db::planes::get_all(&gs.db).await?;
    let mut land_units = empire_db::land_units::get_all(&gs.db).await?;
    mob_inc_all(&mut sectors, &mut ships, &mut planes, &mut land_units, etu, rates);

    // 9. Persist everything back to DB
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
    verbose: bool,
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

        // Feed population — starvation, growth, and work improvement
        do_feed(s, etu, rates, verbose);

        check_pop_loss(s);
    }
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
    verbose: bool,
) {
    let _ = verbose;
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

// ── Feed population (populace.c — do_feed) ───────────────────────────────────
//
// Called per owned sector during prepare_sects.
// Mirrors do_feed() in src/lib/update/populace.c.

fn do_feed(
    s: &mut empire_types::sector::Sector,
    etu: i32,
    rates: &UpdateRates,
    verbose: bool,
) {
    let civ  = s.items.get(Item::Civil)  as f64;
    let mil  = s.items.get(Item::Milit)  as f64;
    let uw   = s.items.get(Item::Uw)     as f64;
    let food = s.items.get(Item::Food)   as f64;

    if (civ + mil + uw) == 0.0 { return; }

    // Births — computed before eating so babies also eat this ETU
    let new_civ = (civ * etu as f64 * rates.obrate).floor();
    let new_uw  = (uw  * etu as f64 * rates.uwbrate).floor();

    // Food needed: everyone eats + food to grow babies to maturity
    let eat_need  = (civ + new_civ + mil + uw + new_uw) * rates.eatrate * etu as f64;
    let grow_need = (new_civ + new_uw) * rates.babyeat;
    let total_need = eat_need + grow_need;

    if total_need <= 0.0 { return; }

    if food >= total_need {
        s.items.set(Item::Food, (food - total_need).floor() as i16);
        // Babies arrive
        let nc = round_avg(new_civ) as i16;
        let nu = round_avg(new_uw)  as i16;
        s.items.add(Item::Civil, nc);
        s.items.add(Item::Uw,    nu);
        // Workers improve with a full belly (cap 100)
        let bump = (etu as f64 * 0.5).round() as i32;
        s.work = (s.work as i32 + bump).min(100) as u8;
        if verbose && (nc > 0 || nu > 0) {
            debug!(x = s.x, y = s.y, ate = total_need as i32, civ = nc, uw = nu, "feed: growth");
        }
    } else {
        // Starvation — kill up to half of civs and uw proportionally
        let frac = if food > 0.0 { 1.0 - food / total_need } else { 1.0 };
        let dead_civ = round_avg(civ * frac * 0.5) as i16;
        let dead_uw  = round_avg(uw  * frac * 0.5) as i16;
        s.items.add(Item::Civil, -dead_civ);
        s.items.add(Item::Uw,   -dead_uw);
        s.items.set(Item::Food, 0);
        if verbose || dead_civ > 0 || dead_uw > 0 {
            debug!(x = s.x, y = s.y, dead_civ, dead_uw, "feed: STARVATION");
        }
    }
}

// ── Delivery (finish.c — dodeliver) ──────────────────────────────────────────
//
// Deliver items above threshold to a direct hex neighbor.
// Handles del[item].path values 1-6.  Path 7 is handled by do_distribute.
// Mobility cost = (amount * weight) / packing / DELIVER_BONUS.

fn do_deliver(
    sectors: &mut Vec<empire_types::sector::Sector>,
    coord_map: &HashMap<(Coord, Coord), usize>,
    world_x: i32,
    world_y: i32,
    verbose: bool,
) {
    for i in 0..sectors.len() {
        if sectors[i].own == 0 { continue; }
        let dchr = SectorChr::for_type(sectors[i].sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }

        let (x, y, own, st, effic) = (
            sectors[i].x, sectors[i].y, sectors[i].own,
            sectors[i].sector_type, sectors[i].effic,
        );

        for &item in &ALL_ITEMS {
            let dir = sectors[i].del[item as usize].path & 7;
            if dir == 0 || dir == 7 { continue; }

            let threshold = sectors[i].del[item as usize].threshold;
            let have = sectors[i].items.get(item);
            if have <= threshold { continue; }

            let surplus = (have - threshold) as f64;
            let ichr = ItemChr::for_item(item);
            let pack = dchr.pack_mult(&ichr.packing, effic) as f64;

            // How much can we move with current mobility?
            let mob_avail = sectors[i].mobil.max(0) as f64;
            let full_cost = surplus * ichr.weight as f64 / pack / DELIVER_BONUS;
            let (amount, mob_cost) = if full_cost <= mob_avail {
                (surplus, full_cost)
            } else {
                let a = (mob_avail * pack * DELIVER_BONUS / ichr.weight as f64).floor();
                (a, mob_avail)
            };
            let amount = amount as i16;
            if amount <= 0 { continue; }

            // Find destination neighbor
            let (nx, ny) = neighbor_xy(x, y, dir, world_x, world_y);
            let Some(&j) = coord_map.get(&(nx, ny)) else { continue };
            if sectors[j].own != own { continue; }

            let room = ITEM_MAX - sectors[j].items.get(item);
            let actual = amount.min(room.max(0));
            if actual <= 0 { continue; }

            sectors[i].items.add(item, -actual);
            sectors[i].mobil -= mob_cost as i8;
            sectors[j].items.add(item, actual);

            if verbose {
                debug!(
                    from_x = x, from_y = y, to_x = nx, to_y = ny,
                    item = %item.mnemonic(), amount = actual,
                    "deliver"
                );
            }
        }
    }
}

// ── Distribution (finish.c — dodistribute / assemble_dist_paths) ─────────────
//
// Two-pass export/import:
//   EXPORT: sectors with surplus move items toward their dist center.
//   IMPORT: sectors with deficit pull items from their dist center.
// Path cost computed by Dijkstra from each unique dist center.

/// Dijkstra from `(cx, cy)` through sectors owned by `own`.
/// Returns a map (x, y) → minimum mob cost to reach that sector from center.
fn dijkstra_dist(
    center_x: Coord,
    center_y: Coord,
    own: u8,
    sectors: &[empire_types::sector::Sector],
    coord_map: &HashMap<(Coord, Coord), usize>,
    world_x: i32,
    world_y: i32,
) -> HashMap<(Coord, Coord), f64> {
    // Priority queue: (Reverse(cost), x, y)
    let mut heap: BinaryHeap<(Reverse<u64>, Coord, Coord)> = BinaryHeap::new();
    let mut dist: HashMap<(Coord, Coord), f64> = HashMap::new();

    let cost_to_u64 = |c: f64| (c * 1_000_000.0) as u64;

    let start = coord_map.get(&(center_x, center_y)).copied();
    if let Some(start_idx) = start {
        if sectors[start_idx].own == own {
            dist.insert((center_x, center_y), 0.0);
            heap.push((Reverse(0), center_x, center_y));
        }
    }

    while let Some((Reverse(_), x, y)) = heap.pop() {
        let cur_cost = match dist.get(&(x, y)) { Some(&c) => c, None => continue };

        for dir in 1u8..=6 {
            let (nx, ny) = neighbor_xy(x, y, dir, world_x, world_y);
            let Some(&ni) = coord_map.get(&(nx, ny)) else { continue };
            let ns = &sectors[ni];
            if ns.own != own { continue; }

            let dchr = SectorChr::for_type(ns.sector_type);
            let edge = dchr.mcost(ns.effic);
            if edge < 0.0 { continue; } // impassable

            let new_cost = cur_cost + edge;
            if new_cost < *dist.get(&(nx, ny)).unwrap_or(&f64::MAX) {
                dist.insert((nx, ny), new_cost);
                heap.push((Reverse(cost_to_u64(new_cost)), nx, ny));
            }
        }
    }

    dist
}

fn do_distribute(
    sectors: &mut Vec<empire_types::sector::Sector>,
    coord_map: &HashMap<(Coord, Coord), usize>,
    world_x: i32,
    world_y: i32,
    verbose: bool,
) {
    // Collect the set of unique (own, dist_x, dist_y) groups
    let mut groups: HashMap<(u8, Coord, Coord), Vec<usize>> = HashMap::new();
    for (i, s) in sectors.iter().enumerate() {
        if s.own == 0 { continue; }
        let dchr = SectorChr::for_type(s.sector_type);
        if dchr.is_water || dchr.is_sanct { continue; }
        // Only include sectors that have at least one item set to distribute (path==7)
        let has_dist = ALL_ITEMS.iter().any(|&it| s.del[it as usize].path & 7 == 7);
        if !has_dist { continue; }
        groups.entry((s.own, s.dist_x, s.dist_y)).or_default().push(i);
    }

    // For each group, compute Dijkstra costs then run export+import
    for ((own, cx, cy), member_idxs) in &groups {
        let path_cost = dijkstra_dist(*cx, *cy, *own, sectors, coord_map, world_x, world_y);

        // Find the dist center sector index
        let Some(&center_idx) = coord_map.get(&(*cx, *cy)) else { continue };
        if sectors[center_idx].own != *own { continue; }

        // ── EXPORT pass ──────────────────────────────────────────────────────
        for &si in member_idxs {
            if si == center_idx { continue; } // center doesn't export to itself
            let cost = match path_cost.get(&(sectors[si].x, sectors[si].y)) {
                Some(&c) if c > 0.0 => c,
                _ => continue,
            };
            let (sx, sy, st, effic) = (
                sectors[si].x, sectors[si].y,
                sectors[si].sector_type, sectors[si].effic,
            );
            for &item in &ALL_ITEMS {
                if sectors[si].del[item as usize].path & 7 != 7 { continue; }
                let threshold = sectors[si].del[item as usize].threshold;
                let have = sectors[si].items.get(item);
                if have <= threshold { continue; }

                let surplus = (have - threshold) as f64;
                let ichr = ItemChr::for_item(item);
                let dchr = SectorChr::for_type(st);
                let pack = dchr.pack_mult(&ichr.packing, effic) as f64;

                let mob_avail = sectors[si].mobil.max(0) as f64;
                let full_mob  = surplus * ichr.weight as f64 * cost / pack / DIST_BONUS;
                let (amount, mob_cost) = if full_mob <= mob_avail {
                    (surplus, full_mob)
                } else {
                    let a = (mob_avail * pack * DIST_BONUS / (ichr.weight as f64 * cost)).floor();
                    (a, mob_avail)
                };
                let amount = amount as i16;
                if amount <= 0 { continue; }

                let center_room = ITEM_MAX - sectors[center_idx].items.get(item);
                let actual = amount.min(center_room.max(0));
                if actual <= 0 { continue; }

                sectors[si].items.add(item, -actual);
                sectors[si].mobil -= mob_cost as i8;
                sectors[center_idx].items.add(item, actual);

                if verbose {
                    debug!(
                        x = sx, y = sy, cx = *cx, cy = *cy,
                        item = %item.mnemonic(), amount = actual,
                        "dist EXPORT"
                    );
                }
            }
        }

        // ── IMPORT pass ──────────────────────────────────────────────────────
        for &si in member_idxs {
            if si == center_idx { continue; }
            let cost = match path_cost.get(&(sectors[si].x, sectors[si].y)) {
                Some(&c) if c > 0.0 => c,
                _ => continue,
            };
            let (sx, sy, st, effic) = (
                sectors[si].x, sectors[si].y,
                sectors[si].sector_type, sectors[si].effic,
            );
            for &item in &ALL_ITEMS {
                if sectors[si].del[item as usize].path & 7 != 7 { continue; }
                let threshold = sectors[si].del[item as usize].threshold;
                let have = sectors[si].items.get(item);
                if have >= threshold { continue; }

                let deficit = (threshold - have) as f64;
                let available = sectors[center_idx].items.get(item);
                if available <= 0 { continue; }

                let want = deficit.min(available as f64);
                let ichr = ItemChr::for_item(item);
                let dchr = SectorChr::for_type(st);
                let pack = dchr.pack_mult(&ichr.packing, effic) as f64;

                let mob_avail = sectors[si].mobil.max(0) as f64;
                let full_mob  = want * ichr.weight as f64 * cost / pack / DIST_BONUS;
                let (amount, mob_cost) = if full_mob <= mob_avail {
                    (want, full_mob)
                } else {
                    let a = (mob_avail * pack * DIST_BONUS / (ichr.weight as f64 * cost)).floor();
                    (a, mob_avail)
                };
                let amount = amount as i16;
                if amount <= 0 { continue; }

                let dest_room = ITEM_MAX - sectors[si].items.get(item);
                let actual = amount.min(dest_room.max(0));
                if actual <= 0 { continue; }

                sectors[center_idx].items.add(item, -actual);
                sectors[si].items.add(item, actual);
                sectors[si].mobil -= mob_cost as i8;

                if verbose {
                    debug!(
                        cx = *cx, cy = *cy, x = sx, y = sy,
                        item = %item.mnemonic(), amount = actual,
                        "dist IMPORT"
                    );
                }
            }
        }
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

