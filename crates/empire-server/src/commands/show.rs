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
// Ported from: src/lib/commands/show.c

// "show" command — display game tables.
// Usage:
//   show sect b            — sector build costs
//   show sect s            — sector stats
//   show sect c            — sector capabilities
//   show item              — commodity table
//   show product           — product table
//   show updates [N]       — upcoming update schedule

use empire_types::sector::SectorType;
use empire_types::sector_chr::SectorChr;
use empire_types::item_chr::ItemChr;
use empire_types::product_chr::ProductChr;
use empire_types::ship_chr::ShipChr;
use empire_types::land_chr::LandChr;
use empire_types::plane_chr::PlaneChr;
use empire_config::rdsched;
use super::ctx::CmdCtx;
use chrono::Local;

// Ordered list of designatable sector types (skip Sea, Unknown which are
// water/deity-only or uncharted).
const SECT_TYPES: &[SectorType] = &[
    SectorType::Land,
    SectorType::Mountain,
    SectorType::Agri,
    SectorType::Uranium,
    SectorType::Plain,
    SectorType::Park,
    SectorType::Urban,
    SectorType::Research,
    SectorType::Wasteland,
    SectorType::Defense,
    SectorType::Bank,
    SectorType::Engineer,
    SectorType::Airfield,
    SectorType::Highway,
    SectorType::Radar,
    SectorType::Naval,
    SectorType::Missile,
    SectorType::Harbor,
    SectorType::Fort,
    SectorType::Tech,
    SectorType::Bravery,
    SectorType::LightIndus,
    SectorType::HeavyIndus,
    SectorType::Gold,
    SectorType::Oil,
];

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let topic = parts.first().copied().unwrap_or("").to_lowercase();

    match topic.as_str() {
        "sect" => {
            let sub = parts.get(1).copied().unwrap_or("b");
            show_sect(sub)
        }
        "ship" => {
            let sub = parts.get(1).copied().unwrap_or("b");
            show_ship(sub)
        }
        "land" => {
            let sub = parts.get(1).copied().unwrap_or("b");
            show_land(sub)
        }
        "plane" => {
            let sub = parts.get(1).copied().unwrap_or("b");
            show_plane(sub)
        }
        "item" => show_item(),
        "product" => show_product(),
        "updates" | "update" => {
            let count: usize = parts.get(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(8);
            show_updates(ctx, count)
        }
        "" => {
            "1 Topics: sect, ship, land, plane, item, product, updates\n0 show\n".to_string()
        }
        _ => {
            format!("10 Unknown show topic '{}'. Try: sect, ship, land, plane, item, product, updates\n", topic)
        }
    }
}

// ── show sect b — build costs ─────────────────────────────────────────────────

fn show_sect(sub: &str) -> String {
    match sub {
        "b" => show_sect_build(),
        "s" => show_sect_stats(),
        "c" => show_sect_capabilities(),
        _ => format!("10 Unknown sect sub-command '{}'. Try b, s, or c\n", sub),
    }
}

fn show_sect_build() -> String {
    let mut out = String::new();
    out.push_str("1 sector type  bld mob cost  bld name\n");
    for &st in SECT_TYPES {
        let dchr = SectorChr::for_type(st);
        out.push_str(&format!(
            "1   {:3}           {:3}  {}  {:5}   {}\n",
            st.mnemonic(),
            dchr.bwork,
            if dchr.maint > 0 { format!("{:2}", dchr.maint) } else { " 0".to_string() },
            dchr.cost,
            dchr.name,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show sect s — sector stats ────────────────────────────────────────────────

fn show_sect_stats() -> String {
    let mut out = String::new();
    out.push_str("1   sect              maint  maxpop  bwork\n");
    for &st in SECT_TYPES {
        let dchr = SectorChr::for_type(st);
        out.push_str(&format!(
            "1   {:3} ({:20}) {:5}  {:6}  {:5}\n",
            st.mnemonic(),
            dchr.name,
            dchr.maint,
            dchr.maxpop,
            dchr.bwork,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show sect c — sector capabilities ────────────────────────────────────────

fn show_sect_capabilities() -> String {
    let mut out = String::new();
    out.push_str("1   sect  product\n");
    for &st in SECT_TYPES {
        let dchr = SectorChr::for_type(st);
        let prd_name = if dchr.prd >= 0 {
            ProductChr::get(dchr.prd)
                .map(|p| p.sname)
                .unwrap_or("none")
        } else {
            "none"
        };
        out.push_str(&format!(
            "1   {:3}   {}\n",
            st.mnemonic(),
            prd_name,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show item ────────────────────────────────────────────────────────────────

fn show_item() -> String {
    let mut out = String::new();
    out.push_str("1   Item                           Mne  Lbs  Pack(INUWB)\n");
    for ichr in ItemChr::all() {
        out.push_str(&format!(
            "1   {:30}   {}  {:3}  {} {} {} {} {}\n",
            ichr.name,
            ichr.mnemonic,
            ichr.weight,
            ichr.packing[0],
            ichr.packing[1],
            ichr.packing[2],
            ichr.packing[3],
            ichr.packing[4],
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show product ─────────────────────────────────────────────────────────────

fn show_product() -> String {
    let mut out = String::new();
    out.push_str("1   Prod    Name                              Produces   Cost  Inputs\n");
    for idx in 0..ProductChr::count() {
        let Some(prd) = ProductChr::get(idx as i8) else { continue };
        let produces = if let Some(item) = prd.item {
            item.name().to_string()
        } else if let Some(lev) = prd.level {
            format!("{:?} level", lev)
        } else {
            "nothing".to_string()
        };
        let inputs: Vec<String> = prd.inputs.iter()
            .flatten()
            .map(|mi| format!("{}x{}", mi.amount, mi.item.mnemonic()))
            .collect();
        let inputs_str = if inputs.is_empty() {
            "none".to_string()
        } else {
            inputs.join(", ")
        };
        out.push_str(&format!(
            "1   {:7} {:34} {:10}  {:4}  {}\n",
            prd.sname, prd.name, produces, prd.cost, inputs_str
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show ship ─────────────────────────────────────────────────────────────────

fn show_ship(sub: &str) -> String {
    match sub {
        "b" => show_ship_build(),
        "s" => show_ship_stats(),
        "c" => show_ship_caps(),
        _ => format!("10 Unknown ship sub-command '{}'. Try b, s, or c\n", sub),
    }
}

fn show_ship_build() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                      lcm  hcm bwork tech   cost\n");
    for (idx, c) in ShipChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:26} {:4} {:4} {:5} {:4} {:6}\n",
            idx, c.sname, c.name, c.lcm, c.hcm, c.bwork, c.tech, c.cost,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_ship_stats() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                      arm spd vis vrn frn gli\n");
    for (idx, c) in ShipChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:26} {:3} {:3} {:3} {:3} {:3} {:3}\n",
            idx, c.sname, c.name,
            c.armor, c.speed, c.visib, c.vrnge, c.frnge, c.glim,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_ship_caps() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                      nxl nch npl nla flags\n");
    for (idx, c) in ShipChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:26} {:3} {:3} {:3} {:3} {:?}\n",
            idx, c.sname, c.name,
            c.nxlight, c.nchoppers, c.nplanes, c.nland,
            c.flags,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show land ─────────────────────────────────────────────────────────────────

fn show_land(sub: &str) -> String {
    match sub {
        "b" => show_land_build(),
        "s" => show_land_stats(),
        "c" => show_land_caps(),
        _ => format!("10 Unknown land sub-command '{}'. Try b, s, or c\n", sub),
    }
}

fn show_land_build() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                lcm  hcm bwork tech   cost\n");
    for (idx, c) in LandChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:20} {:4} {:4} {:5} {:4} {:6}\n",
            idx, c.sname, c.name, c.lcm, c.hcm, c.bwork, c.tech, c.cost,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_land_stats() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                att  def vul spd vis spy rad frg acc dam amm aaf\n");
    for (idx, c) in LandChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:20} {:4.1} {:4.1} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3} {:3}\n",
            idx, c.sname, c.name,
            c.att, c.def, c.vul, c.spd, c.vis, c.spy, c.rad,
            c.frg, c.acc, c.dam, c.ammo, c.aaf,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_land_caps() -> String {
    let mut out = String::new();
    out.push_str("1  # type     name                nxl nla flags\n");
    for (idx, c) in LandChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:8} {:20} {:3} {:3} {:?}\n",
            idx, c.sname, c.name,
            c.nxlight, c.nland, c.flags,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show plane ────────────────────────────────────────────────────────────────

fn show_plane(sub: &str) -> String {
    match sub {
        "b" => show_plane_build(),
        "s" => show_plane_stats(),
        "c" => show_plane_caps(),
        _ => format!("10 Unknown plane sub-command '{}'. Try b, s, or c\n", sub),
    }
}

fn show_plane_build() -> String {
    let mut out = String::new();
    out.push_str("1  # type  name                          lcm  hcm mil bwork tech   cost\n");
    for (idx, c) in PlaneChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:6} {:30} {:4} {:4} {:3} {:5} {:4} {:6}\n",
            idx, c.sname, c.name, c.lcm, c.hcm, c.mil, c.bwork, c.tech, c.cost,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_plane_stats() -> String {
    let mut out = String::new();
    out.push_str("1  # type  name                          acc loa att def ran fue ste\n");
    for (idx, c) in PlaneChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:6} {:30} {:3} {:3} {:3} {:3} {:3} {:3} {:3}\n",
            idx, c.sname, c.name,
            c.acc, c.load, c.att, c.def, c.range, c.fuel, c.stealth,
        ));
    }
    out.push_str("0 show\n");
    out
}

fn show_plane_caps() -> String {
    let mut out = String::new();
    out.push_str("1  # type  name                          flags\n");
    for (idx, c) in PlaneChr::all().iter().enumerate() {
        out.push_str(&format!(
            "1 {:2} {:6} {:30} {:?}\n",
            idx, c.sname, c.name, c.flags,
        ));
    }
    out.push_str("0 show\n");
    out
}

// ── show updates ────────────────────────────────────────────────────────────

fn show_updates(ctx: &CmdCtx<'_>, count: usize) -> String {
    let sched_path = &ctx.config.server.schedule_file;
    if sched_path.as_os_str().is_empty() || !sched_path.exists() {
        return "1 No schedule file configured\n0 show\n".to_string();
    }

    let now = Local::now();
    let after = now;
    let anchor = {
        let secs = now.timestamp();
        let rounded = (secs + 59) / 60 * 60;
        match chrono::DateTime::from_timestamp(rounded, 0) {
            Some(utc) => utc.with_timezone(&Local),
            None => now,
        }
    };

    let sched = match rdsched::read_schedule(sched_path, after, anchor, count.max(1)) {
        Ok(s) => s,
        Err(e) => return format!("10 Cannot read schedule: {e}\n"),
    };

    let mut out = String::new();
    if sched.is_empty() {
        out.push_str("1 No upcoming updates scheduled\n");
    } else {
        out.push_str(&format!("1 Next {} update(s):\n", sched.len()));
        for dt in &sched {
            out.push_str(&format!("1   {}\n", dt.format("%Y-%m-%d %H:%M:%S %Z")));
        }
    }
    out.push_str("0 show\n");
    out
}
