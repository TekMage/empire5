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

// "dump" / "sdump" / "ldump" / "pdump" commands — dump game state in
// the classic Empire 4 format that ptkei's ParseDump parser expects.
//
// Field order matches what ptkei's CenWin expects (from its comment block):
//   x y des sdes eff mob * off min gold fert ocontent uran work avail terr
//   civ mil uw food shell gun pet iron dust bar oil lcm hcm rad
//   u_del f_del s_del g_del p_del i_del d_del b_del o_del l_del h_del r_del
//   u_cut f_cut s_cut g_cut p_cut i_cut d_cut b_cut o_cut l_cut h_cut r_cut
//   dist_x dist_y
//   c_dist m_dist u_dist f_dist s_dist g_dist p_dist i_dist d_dist b_dist
//   o_dist l_dist h_dist r_dist
//   road rail defense fallout coast
//   c_del m_del c_cut m_cut

use super::ctx::CmdCtx;
use empire_db::{sectors, ships};
use empire_types::commodity::Item;
use empire_types::ship_chr::ShipChr;

// Item enum indices used to index del[] array:
//   Civil=0, Milit=1, Shell=2, Gun=3, Petrol=4, Iron=5, Dust=6, Bar=7,
//   Food=8, Oil=9, Lcm=10, Hcm=11, Uw=12, Rad=13

fn dir_char(d: u8) -> char {
    match d {
        0 => '.', 1 => 'u', 2 => 'j', 3 => 'n',
        4 => 'b', 5 => 'g', 6 => 'y', 7 => '$',
        _ => '.',
    }
}

pub async fn run(subcmd: &str, ctx: &CmdCtx<'_>) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    match subcmd {
        "dump"  => dump_sectors(ts, ctx).await,
        "sdump" => dump_ships(ts, ctx).await,
        "ldump" => empty_dump("LAND UNITS", "units",  "ldump", ts),
        "pdump" => empty_dump("PLANES",     "planes", "pdump", ts),
        _       => "10 Unknown dump subcommand\n".to_string(),
    }
}

async fn dump_sectors(ts: i64, ctx: &CmdCtx<'_>) -> String {
    let all = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|s| s.own == ctx.cnum)
        .collect();

    let mut out = String::new();
    out.push_str(&format!("1 DUMP SECTOR {ts}\n"));
    out.push_str("1 x y des sdes eff mob * off min gold fert ocontent uran work avail terr \
civ mil uw food shell gun pet iron dust bar oil lcm hcm rad \
u_del f_del s_del g_del p_del i_del d_del b_del o_del l_del h_del r_del \
u_cut f_cut s_cut g_cut p_cut i_cut d_cut b_cut o_cut l_cut h_cut r_cut \
dist_x dist_y \
c_dist m_dist u_dist f_dist s_dist g_dist p_dist i_dist d_dist b_dist o_dist l_dist h_dist r_dist \
road rail defense fallout coast \
c_del m_del c_cut m_cut\n");

    for s in &mine {
        let des = s.sector_type.mnemonic();
        let rx  = ctx.x_rel(s.x);
        let ry  = ctx.y_rel(s.y);
        let inv = &s.items;
        let del = &s.del;

        // Delivery directions for uw,food,shell,gun,pet,iron,dust,bar,oil,lcm,hcm,rad
        let ud = dir_char(del[Item::Uw    as usize].path);
        let fd = dir_char(del[Item::Food  as usize].path);
        let sd = dir_char(del[Item::Shell as usize].path);
        let gd = dir_char(del[Item::Gun   as usize].path);
        let pd = dir_char(del[Item::Petrol as usize].path);
        let id = dir_char(del[Item::Iron  as usize].path);
        let dd = dir_char(del[Item::Dust  as usize].path);
        let bd = dir_char(del[Item::Bar   as usize].path);
        let od = dir_char(del[Item::Oil   as usize].path);
        let ld = dir_char(del[Item::Lcm   as usize].path);
        let hd = dir_char(del[Item::Hcm   as usize].path);
        let rd = dir_char(del[Item::Rad   as usize].path);
        let cd = dir_char(del[Item::Civil as usize].path);
        let md = dir_char(del[Item::Milit as usize].path);

        // Delivery cutoffs (thresholds) for same items
        let uc = del[Item::Uw     as usize].threshold;
        let fc = del[Item::Food   as usize].threshold;
        let sc = del[Item::Shell  as usize].threshold;
        let gc = del[Item::Gun    as usize].threshold;
        let pc = del[Item::Petrol as usize].threshold;
        let ic = del[Item::Iron   as usize].threshold;
        let dc = del[Item::Dust   as usize].threshold;
        let bc = del[Item::Bar    as usize].threshold;
        let oc = del[Item::Oil    as usize].threshold;
        let lc = del[Item::Lcm    as usize].threshold;
        let hc = del[Item::Hcm    as usize].threshold;
        let rc = del[Item::Rad    as usize].threshold;
        let cc = del[Item::Civil  as usize].threshold;
        let mc = del[Item::Milit  as usize].threshold;

        // Distribution center (player-relative)
        let dx = ctx.x_rel(s.dist_x);
        let dy = ctx.y_rel(s.dist_y);

        out.push_str(&format!(
            "1 {rx} {ry} {des} _ {eff} {mob} . 0 {min} 0 {fert} {ocont} {uran} {work} 0 0 \
{civ} {mil} {uw} {food} {shell} {gun} {pet} {iron} {dust} {bar} {oil} {lcm} {hcm} {rad} \
{ud} {fd} {sd} {gd} {pd} {id} {dd} {bd} {od} {ld} {hd} {rd} \
0 0 0 0 0 0 0 0 0 0 0 0 \
{dx} {dy} \
{cc} {mc} {uc} {fc} {sc} {gc} {pc} {ic} {dc} {bc} {oc} {lc} {hc} {rc} \
0 0 0 0 0 \
{cd} {md} 0 0\n",
            eff   = s.effic,
            mob   = s.mobil,
            min   = s.mines,
            fert  = s.fertil,
            ocont = s.oil,
            uran  = s.uran,
            work  = s.work,
            civ   = inv.get(Item::Civil),
            mil   = inv.get(Item::Milit),
            uw    = inv.get(Item::Uw),
            food  = inv.get(Item::Food),
            shell = inv.get(Item::Shell),
            gun   = inv.get(Item::Gun),
            pet   = inv.get(Item::Petrol),
            iron  = inv.get(Item::Iron),
            dust  = inv.get(Item::Dust),
            bar   = inv.get(Item::Bar),
            oil   = inv.get(Item::Oil),
            lcm   = inv.get(Item::Lcm),
            hcm   = inv.get(Item::Hcm),
            rad   = inv.get(Item::Rad),
        ));
    }

    out.push_str(&format!("1 {} sectors\n", mine.len()));
    out.push_str("0 dump\n");
    out
}

async fn dump_ships(ts: i64, ctx: &CmdCtx<'_>) -> String {
    let all = match ships::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|s| s.own == ctx.cnum)
        .collect();

    let mut out = String::new();
    out.push_str(&format!("1 DUMP SHIPS {ts}\n"));
    out.push_str("1 uid own x y type eff mob tech fl\n");

    for s in &mine {
        let rx = ctx.x_rel(s.x);
        let ry = ctx.y_rel(s.y);
        let type_name = ShipChr::for_type(s.ship_type as usize)
            .map(|c| c.name)
            .unwrap_or("unknown");
        out.push_str(&format!(
            "1 {} {} {} {} {} {} {} {} {}\n",
            s.uid, s.own, rx, ry, type_name, s.effic, s.mobil, s.tech, s.fleet
        ));
    }

    let n = mine.len();
    out.push_str(&format!("1 {n} ships\n"));
    out.push_str("0 sdump\n");
    out
}

// Outputs an empty dump for land/plane types — no records but valid
// header and footer so ParseDump terminates cleanly.
fn empty_dump(dump_type: &str, unit_name: &str, cmd_ok: &str, ts: i64) -> String {
    format!(
        "1 DUMP {dump_type} {ts}\n1 uid own x y type eff mob\n1 0 {unit_name}\n0 {cmd_ok}\n"
    )
}
