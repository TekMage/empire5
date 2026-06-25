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
// Ported from: src/util/pconfig.c
// Known contributors to the original:
//    Julian Onions

// pconfig: print effective server configuration.
// Loads config/empire.toml (or a specified file) and prints every
// key with its effective value in a format that mirrors the original
// C server's "key    value" econfig output.

use std::path::PathBuf;
use clap::Parser;
use empire_config::load_or_default;

#[derive(Parser, Debug)]
#[command(
    name = "pconfig",
    about = "Print effective Empire server configuration"
)]
struct Args {
    /// Config file to read (default: config/empire.toml)
    #[arg(short = 'e', long = "config", default_value = "config/empire.toml")]
    config: PathBuf,
}

fn main() {
    let args = Args::parse();
    let cfg = load_or_default(&args.config);

    println!("# Empire 5 effective configuration");
    println!("# Loaded from: {}", args.config.display());
    println!();

    // [server]
    println!("[server]");
    let s = &cfg.server;
    kv("listen_addr",   &fmt_str(&s.listen_addr));
    kv("port",          &s.port.to_string());
    kv("data_dir",      &s.data_dir.display().to_string());
    kv("info_dir",      &s.info_dir.display().to_string());
    kv("schedule_file", &s.schedule_file.display().to_string());
    kv("keep_journal",  &s.keep_journal.to_string());
    kv("motd_file",     &s.motd_file.display().to_string());
    kv("down_file",     &s.down_file.display().to_string());
    kv("tel_dir",       &s.tel_dir.display().to_string());
    println!();

    // [game]
    println!("[game]");
    let g = &cfg.game;
    kv("etu_per_update",     &g.etu_per_update.to_string());
    kv("start_cash",         &g.start_cash.to_string());
    kv("startmob",           &g.startmob.to_string());
    kv("rollover_avail_max", &g.rollover_avail_max.to_string());
    kv("anno_keep_days",     &g.anno_keep_days.to_string());
    kv("news_keep_days",     &g.news_keep_days.to_string());
    kv("lost_keep_hours",    &g.lost_keep_hours.to_string());
    kv("world_x",            &g.world_x.to_string());
    kv("world_y",            &g.world_y.to_string());
    kv("opt_market",         &g.opt_market.to_string());
    println!();

    // [update]
    println!("[update]");
    let u = &cfg.update;
    kv("update_interval_secs", &u.update_interval_secs.to_string());
    kv("update_window",        &u.update_window.to_string());
    kv("allow_force",          &u.allow_force.to_string());
    println!();

    // [rates]
    println!("[rates]");
    let r = &cfg.rates;
    kv("sect_mob_scale",   &r.sect_mob_scale.to_string());
    kv("sect_mob_max",     &r.sect_mob_max.to_string());
    kv("land_mob_scale",   &r.land_mob_scale.to_string());
    kv("land_mob_max",     &r.land_mob_max.to_string());
    kv("ship_mob_scale",   &r.ship_mob_scale.to_string());
    kv("ship_mob_max",     &r.ship_mob_max.to_string());
    kv("plane_mob_scale",  &r.plane_mob_scale.to_string());
    kv("plane_mob_max",    &r.plane_mob_max.to_string());
    kv("money_civ",        &r.money_civ.to_string());
    kv("money_uw",         &r.money_uw.to_string());
    kv("money_mil",        &r.money_mil.to_string());
    kv("money_res",        &r.money_res.to_string());
    kv("money_plane",      &r.money_plane.to_string());
    kv("money_ship",       &r.money_ship.to_string());
    kv("money_land",       &r.money_land.to_string());
    kv("bankint",          &r.bankint.to_string());
    kv("obrate",           &r.obrate.to_string());
    kv("uwbrate",          &r.uwbrate.to_string());
    kv("eatrate",          &r.eatrate.to_string());
    kv("babyeat",          &r.babyeat.to_string());
    kv("fcrate",           &r.fcrate.to_string());
    kv("fgrate",           &r.fgrate.to_string());
    kv("easy_tech",        &r.easy_tech.to_string());
    kv("tech_log_base",    &r.tech_log_base.to_string());
    kv("ally_factor",      &r.ally_factor.to_string());
    kv("level_age_rate",   &r.level_age_rate.to_string());
    kv("hap_avg",          &r.hap_avg.to_string());
    kv("edu_avg",          &r.edu_avg.to_string());
    kv("hap_cons",         &r.hap_cons.to_string());
    kv("edu_cons",         &r.edu_cons.to_string());
    kv("land_grow_scale",  &r.land_grow_scale.to_string());
    kv("ship_grow_scale",  &r.ship_grow_scale.to_string());
    kv("plane_grow_scale", &r.plane_grow_scale.to_string());
    println!();

    // [limits]
    println!("[limits]");
    let l = &cfg.limits;
    kv("max_nations",        &l.max_nations.to_string());
    kv("max_realms",         &l.max_realms.to_string());
    kv("max_connections",    &l.max_connections.to_string());
    kv("m_m_p_d",            &l.m_m_p_d.to_string());
    kv("max_idle",           &l.max_idle.to_string());
    kv("max_idle_visitor",   &l.max_idle_visitor.to_string());
    kv("login_grace_time",   &l.login_grace_time.to_string());
}

fn kv(key: &str, val: &str) {
    println!("  {key:<24} {val}");
}

fn fmt_str(s: &str) -> String {
    if s.is_empty() { "(all interfaces)".to_string() } else { s.to_string() }
}
