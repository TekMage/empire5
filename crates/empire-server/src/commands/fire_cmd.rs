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
// Ported from: src/lib/commands/mfir.c (c_fire)

// "fire" command — gunnery. Ships, artillery land units, and fortress
// sectors can all shoot at a ship or a sector.
//
// Usage: fire <ship|land|fort> <firer-id> <ship|sect> <target-id>
//   fire ship <uid-spec> ship <target-uid>
//   fire ship <uid-spec> sect <x,y>
//   fire land <uid-spec> ship <target-uid>
//   fire land <uid-spec> sect <x,y>
//   fire fort <x,y>       ship <target-uid>
//   fire fort <x,y>       sect <x,y>
//
// <firer-id> for ship/land also accepts a fleet/army letter, "~"
// (unassigned units), a uid range, or a comma list — see 'info fleetadd'
// and 'info army'.
//
// Unlike bombing/torpedoes, gunnery has no miss chance — damage lands
// unconditionally once the firer is in range and eligible. A ship target
// that survives and still has usable guns fires back once (a simplified
// single-reprisal stand-in for 4.4.1's full pooled multi-defender return
// fire — see info/fire for the documented gaps).

use rand::SeedableRng;
use rand::rngs::StdRng;

use empire_db::{land_units, nations, news, sectors, ships};
use empire_types::commodity::Item;
use empire_types::land_chr::LandChr;
use empire_types::news::NewsVerb;
use empire_types::sector::Sector;
use empire_types::ship::Ship;
use empire_types::ship_chr::ShipChr;

use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;
use crate::subs::damage;
use crate::subs::fortsub::{fort_can_fire, fort_gun_range, fortgun_damage};
use crate::subs::geo::map_dist;
use crate::subs::lndsub::{land_spec_matches, lnd_can_fire, lnd_fire_shot, lnd_gun_range};
use crate::subs::shpsub::{
    seagun_damage, ship_spec_matches, shp_can_fire, shp_damage, shp_fire_at_ship,
    shp_gun_range, shp_guns_after_ammo, shp_guns_fired, shp_in_range, shp_shells_needed,
};

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 4 {
        return "10 Usage: fire <ship|land|fort> <firer-id> <ship|sect> <target-id>\n".to_string();
    }
    let firer_kind = parts[0];
    let firer_id = parts[1];
    let target_kind = parts[2];
    let target_id = parts[3];

    if target_kind != "ship" && target_kind != "sect" {
        return format!("10 Unknown target type '{target_kind}' (want ship|sect)\n");
    }

    match firer_kind {
        "ship" => fire_from_ships(firer_id, target_kind, target_id, ctx).await,
        "land" => fire_from_land(firer_id, target_kind, target_id, ctx).await,
        "fort" => fire_from_fort(firer_id, target_kind, target_id, ctx).await,
        _ => format!("10 Unknown firer type '{firer_kind}' (want ship|land|fort)\n"),
    }
}

// ── Target resolution ─────────────────────────────────────────────────────────

enum Target {
    Ship(Ship),
    Sect(Sector),
}

async fn resolve_target(target_kind: &str, target_id: &str, ctx: &CmdCtx<'_>) -> Result<Target, String> {
    match target_kind {
        "ship" => {
            let uid: i32 = target_id.parse().map_err(|_| format!("10 Bad ship uid '{target_id}'\n"))?;
            match ships::get(ctx.db, uid).await {
                Ok(Some(s)) => Ok(Target::Ship(s)),
                Ok(None) => Err(format!("10 No ship #{uid}\n")),
                Err(e) => Err(format!("10 DB error: {e}\n")),
            }
        }
        "sect" => {
            let Some((rx, ry)) = parse_rel_xy(target_id) else {
                return Err(format!("10 Bad sector specification '{target_id}'\n"));
            };
            let (tx, ty) = (ctx.x_abs(rx), ctx.y_abs(ry));
            match sectors::get_at(ctx.db, tx, ty).await {
                Ok(Some(s)) => Ok(Target::Sect(s)),
                Ok(None) => Err(format!("10 Sector {} doesn't exist\n", ctx.format_xy(tx, ty))),
                Err(e) => Err(format!("10 DB error: {e}\n")),
            }
        }
        _ => unreachable!(),
    }
}

impl Target {
    fn xy(&self) -> (i16, i16) {
        match self {
            Target::Ship(s) => (s.x, s.y),
            Target::Sect(s) => (s.x, s.y),
        }
    }
}

async fn file_news(ctx: &CmdCtx<'_>, verb: NewsVerb, victim: u8) {
    if victim == ctx.cnum || victim == 0 { return; }
    let _ = news::add_news(ctx.db, ctx.cnum, verb as u8, victim, 1).await;
}

// ── Ship firer ────────────────────────────────────────────────────────────────

async fn fire_from_ships(firer_spec: &str, target_kind: &str, target_id: &str, ctx: &CmdCtx<'_>) -> String {
    let mut all_ships = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let firer_uids: Vec<i32> = all_ships.iter()
        .filter(|s| (s.own == ctx.cnum || ctx.is_deity) && ship_spec_matches(firer_spec, s))
        .map(|s| s.uid)
        .collect();
    if firer_uids.is_empty() {
        return "10 No ships match that specification.\n".to_string();
    }

    let target = match resolve_target(target_kind, target_id, ctx).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    let (tx, ty) = target.xy();
    // Target ship, if any, is a self-owned uid not present in `firer_uids`
    // (self-fire is rejected below), so it's fine to skip it when scanning
    // `all_ships` for the firer that matches `target_uid` by coincidence —
    // there is none, since a ship can't be both.
    let mut target_ship: Option<Ship> = if let Target::Ship(s) = &target { Some(s.clone()) } else { None };
    let mut target_sect: Option<Sector> = if let Target::Sect(s) = &target { Some(s.clone()) } else { None };

    let mut out = String::new();
    let mut rng = StdRng::from_entropy();

    for firer_uid in firer_uids.iter().copied() {
        if let Some(ship) = &target_ship {
            if ship.uid == firer_uid {
                out.push_str("1 A ship can't fire on itself\n");
                continue;
            }
        }

        let Some(idx) = all_ships.iter().position(|s| s.uid == firer_uid) else { continue };
        let Some(mchr) = ShipChr::for_type(all_ships[idx].ship_type as usize) else {
            out.push_str(&format!("1 Ship #{firer_uid}: unknown type — skipped\n"));
            continue;
        };

        if !shp_can_fire(&all_ships[idx], mchr) {
            out.push_str(&format!(
                "1 Ship #{firer_uid}: not eligible to fire (effic, guns, shells, or crew)\n"
            ));
            continue;
        }
        if !shp_in_range(&all_ships[idx], mchr, tx, ty, ctx.world_x, ctx.world_y) {
            out.push_str(&format!("1 Ship #{firer_uid}: target out of range\n"));
            continue;
        }

        match (&mut target_ship, &mut target_sect) {
            (Some(defender), _) => {
                let Some(def_mchr) = ShipChr::for_type(defender.ship_type as usize) else { continue };
                let attacker = &mut all_ships[idx];

                let result = shp_fire_at_ship(attacker, mchr, defender, def_mchr, &mut rng);
                out.push_str(&format!(
                    "1 Ship #{firer_uid} fires {} gun(s): {} damage to ship #{} (effic now {}%)\n",
                    result.guns, result.damage_dealt, defender.uid, defender.effic
                ));
                file_news(ctx, NewsVerb::ShpShell, defender.own).await;

                if !result.target_sunk && shp_can_fire(defender, def_mchr) {
                    let counter = shp_fire_at_ship(defender, def_mchr, attacker, mchr, &mut rng);
                    out.push_str(&format!(
                        "1 Ship #{} returns fire: {} damage to ship #{firer_uid} (effic now {}%)\n",
                        defender.uid, counter.damage_dealt, attacker.effic
                    ));
                } else if result.target_sunk {
                    out.push_str(&format!("1 Ship #{} sinks!\n", defender.uid));
                }
            }
            (_, Some(sector)) => {
                let guns = shp_guns_fired(
                    all_ships[idx].items.get(Item::Milit) as i32,
                    all_ships[idx].items.get(Item::Gun) as i32,
                    mchr.glim,
                );
                let guns = shp_guns_after_ammo(
                    guns, all_ships[idx].items.get(Item::Shell) as i32,
                );
                if guns <= 0 {
                    out.push_str(&format!("1 Ship #{firer_uid}: out of ammo\n"));
                    continue;
                }
                let shells_used = shp_shells_needed(guns);
                all_ships[idx].items.add(Item::Shell, -(shells_used as i16));
                all_ships[idx].mobil = (all_ships[idx].mobil as i32 - 15).max(-100) as i8;

                let dam = seagun_damage(all_ships[idx].effic, guns, &mut rng);
                let old_effic = sector.effic;
                sector.effic = damage::damage(sector.effic as i32, dam.clamp(0, 100)) as i8;
                out.push_str(&format!(
                    "1 Ship #{firer_uid} fires {guns} gun(s) at {}: {dam} damage (effic {old_effic}% -> {}%)\n",
                    ctx.format_xy(sector.x, sector.y), sector.effic
                ));
                file_news(ctx, NewsVerb::SctShell, sector.own).await;
            }
            _ => {}
        }
    }

    for uid in firer_uids.iter().copied() {
        if let Some(s) = all_ships.iter().find(|s| s.uid == uid) {
            let _ = ships::put(ctx.db, s).await;
        }
    }
    if let Some(ship) = &target_ship {
        let _ = ships::put(ctx.db, ship).await;
    }
    if let Some(sector) = &target_sect {
        if let Err(e) = sectors::put(ctx.db, sector).await {
            out.push_str(&format!("1 Warning: error saving sector: {e}\n"));
        }
    }

    if out.is_empty() {
        out.push_str("1 Nothing fired.\n");
    }
    out.push_str("0 fire\n");
    out
}

// ── Land-unit (artillery) firer ───────────────────────────────────────────────

async fn fire_from_land(firer_spec: &str, target_kind: &str, target_id: &str, ctx: &CmdCtx<'_>) -> String {
    let mut all_units = match land_units::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    let firer_uids: Vec<i32> = all_units.iter()
        .filter(|u| (u.own == ctx.cnum || ctx.is_deity) && land_spec_matches(firer_spec, u))
        .map(|u| u.uid)
        .collect();
    if firer_uids.is_empty() {
        return "10 No land units match that specification.\n".to_string();
    }

    let target = match resolve_target(target_kind, target_id, ctx).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    let (tx, ty) = target.xy();

    let mut out = String::new();
    let mut rng = StdRng::from_entropy();
    let mut target_ship: Option<Ship> = if let Target::Ship(s) = &target { Some(s.clone()) } else { None };
    let mut target_sect: Option<Sector> = if let Target::Sect(s) = &target { Some(s.clone()) } else { None };

    for firer_uid in firer_uids.iter().copied() {
        let Some(idx) = all_units.iter().position(|u| u.uid == firer_uid) else { continue };
        let Some(lchr) = LandChr::for_type(all_units[idx].land_type as usize) else {
            out.push_str(&format!("1 Land unit #{firer_uid}: unknown type — skipped\n"));
            continue;
        };

        if !lnd_can_fire(&all_units[idx], lchr) {
            out.push_str(&format!(
                "1 Land unit #{firer_uid}: not eligible to fire (effic, guns, shells, or crew)\n"
            ));
            continue;
        }
        let range = lnd_gun_range(&all_units[idx], lchr);
        let dist = map_dist(all_units[idx].x, all_units[idx].y, tx, ty, ctx.world_x, ctx.world_y);
        if dist > range {
            out.push_str(&format!("1 Land unit #{firer_uid}: target out of range\n"));
            continue;
        }

        let dam = lnd_fire_shot(&mut all_units[idx], lchr, &mut rng);
        if dam <= 0 {
            out.push_str(&format!("1 Land unit #{firer_uid}: out of ammo\n"));
            continue;
        }

        match (&mut target_ship, &mut target_sect) {
            (Some(ship), _) => {
                let Some(def_mchr) = ShipChr::for_type(ship.ship_type as usize) else { continue };
                let sunk = shp_damage(ship, dam, def_mchr.armor);
                out.push_str(&format!(
                    "1 Land unit #{firer_uid} fires: {dam} damage to ship #{} (effic now {}%)\n",
                    ship.uid, ship.effic
                ));
                let target_own = ship.own;
                file_news(ctx, NewsVerb::ShpShell, target_own).await;
                if sunk { out.push_str(&format!("1 Ship #{} sinks!\n", ship.uid)); }
            }
            (_, Some(sector)) => {
                let old_effic = sector.effic;
                sector.effic = damage::damage(sector.effic as i32, dam.clamp(0, 100)) as i8;
                out.push_str(&format!(
                    "1 Land unit #{firer_uid} fires at {}: {dam} damage (effic {old_effic}% -> {}%)\n",
                    ctx.format_xy(sector.x, sector.y), sector.effic
                ));
                let sect_own = sector.own;
                file_news(ctx, NewsVerb::SctShell, sect_own).await;
            }
            _ => {}
        }
    }

    for uid in firer_uids {
        if let Some(u) = all_units.iter().find(|u| u.uid == uid) {
            let _ = land_units::put(ctx.db, u).await;
        }
    }
    if let Some(ship) = &target_ship {
        let _ = ships::put(ctx.db, ship).await;
    }
    if let Some(sector) = &target_sect {
        if let Err(e) = sectors::put(ctx.db, sector).await {
            out.push_str(&format!("1 Warning: error saving sector: {e}\n"));
        }
    }

    if out.is_empty() {
        out.push_str("1 Nothing fired.\n");
    }
    out.push_str("0 fire\n");
    out
}

// ── Fort firer ────────────────────────────────────────────────────────────────

async fn fire_from_fort(firer_xy: &str, target_kind: &str, target_id: &str, ctx: &CmdCtx<'_>) -> String {
    let Some((rx, ry)) = parse_rel_xy(firer_xy) else {
        return format!("10 Bad sector specification '{firer_xy}'\n");
    };
    let (fx, fy) = (ctx.x_abs(rx), ctx.y_abs(ry));
    let mut fort = match sectors::get_at(ctx.db, fx, fy).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(fx, fy)),
        Err(e) => return format!("10 DB error: {e}\n"),
    };
    if fort.own != ctx.cnum && !ctx.is_deity {
        return "10 That sector isn't yours\n".to_string();
    }
    if !fort_can_fire(&fort) {
        return "10 That fortress can't fire (effic, guns, shells, or militia)\n".to_string();
    }

    let target = match resolve_target(target_kind, target_id, ctx).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    let (tx, ty) = target.xy();

    let owner_tech = match nations::get_by_cnum(ctx.db, fort.own).await {
        Ok(Some(n)) => n.tech,
        _ => 0.0,
    };
    let range = fort_gun_range(fort.effic, owner_tech);
    let dist = map_dist(fort.x, fort.y, tx, ty, ctx.world_x, ctx.world_y);
    if dist > range {
        return "10 Target out of range\n".to_string();
    }

    let mut rng = StdRng::from_entropy();
    let guns = fort.items.get(Item::Gun) as i32;
    let dam = fortgun_damage(fort.effic, guns, &mut rng);
    fort.items.add(Item::Shell, -1);

    let mut out = String::new();
    match target {
        Target::Ship(mut ship) => {
            let Some(def_mchr) = ShipChr::for_type(ship.ship_type as usize) else {
                return "10 Unknown target ship type\n".to_string();
            };
            let sunk = shp_damage(&mut ship, dam, def_mchr.armor);
            out.push_str(&format!(
                "1 Fortress fires: {dam} damage to ship #{} (effic now {}%)\n", ship.uid, ship.effic
            ));
            file_news(ctx, NewsVerb::ShpShell, ship.own).await;
            if sunk { out.push_str(&format!("1 Ship #{} sinks!\n", ship.uid)); }
            let _ = ships::put(ctx.db, &ship).await;
        }
        Target::Sect(mut sector) => {
            let old_effic = sector.effic;
            sector.effic = damage::damage(sector.effic as i32, dam.clamp(0, 100)) as i8;
            out.push_str(&format!(
                "1 Fortress fires at {}: {dam} damage (effic {old_effic}% -> {}%)\n",
                ctx.format_xy(sector.x, sector.y), sector.effic
            ));
            file_news(ctx, NewsVerb::SctShell, sector.own).await;
            if let Err(e) = sectors::put(ctx.db, &sector).await {
                out.push_str(&format!("1 Warning: error saving sector: {e}\n"));
            }
        }
    }

    if let Err(e) = sectors::put(ctx.db, &fort).await {
        out.push_str(&format!("1 Warning: error saving fort sector: {e}\n"));
    }
    out.push_str("0 fire\n");
    out
}
