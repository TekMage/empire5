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
// Ported from: include/econfig-spec.h, include/optlist.h,
//              src/lib/global/constants.c
// Known contributors to the original:
//    Julian Onions (optlist)
//    Ken Stevens, 1995
//    Marc Olzheim, 2004
//    Steve McClure, 1998
//    Markus Armbruster, 2004-2020

// The original econfig key-value format is replaced by TOML.
// Key names and default values are preserved from constants.c.

pub mod rdsched;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config file {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("cannot parse config file {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
}

/// Top-level configuration, loaded from `empire.toml`.
/// Equivalent to the `econfig` key space in the C server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub game: GameConfig,
    pub update: UpdateConfig,
    pub rates: UpdateRates,
    pub limits: LimitsConfig,
}

/// Network and filesystem paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// IP address the server binds (empty = all interfaces).
    pub listen_addr: String,
    /// TCP port.  Default 6665 matches the historic Empire port.
    pub port: u16,
    /// Directory where game state (database) is stored.
    pub data_dir: PathBuf,
    /// Directory where info/help pages are stored.
    pub info_dir: PathBuf,
    /// Path to schedule file (update times).
    pub schedule_file: PathBuf,
    /// Write a journal log of all player I/O.
    pub keep_journal: bool,
    /// Message-of-the-day file shown at login.
    pub motd_file: PathBuf,
    /// Downtime message file.
    pub down_file: PathBuf,
    /// Telegram directory.
    pub tel_dir: PathBuf,
}

/// Game-balance parameters.  ref: src/lib/global/constants.c, econfig-spec.h
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    /// ETU (Empire Time Units) per update cycle.  Default 60.
    pub etu_per_update: i32,
    /// Starting money for a new nation.
    pub start_cash: i32,
    /// Starting mobility for sanctuaries.
    pub startmob: i32,
    /// Maximum avail units that roll over an update.
    pub rollover_avail_max: i32,
    /// Keep announcements for this many days (< 0 = forever).
    pub anno_keep_days: i32,
    /// Keep news items for this many days.
    pub news_keep_days: i32,
    /// Keep lost-items entries for this many hours.
    pub lost_keep_hours: i32,
    /// World width in sectors (must be even; default 64 matches fairland).
    pub world_x: i32,
    /// World height in sectors (must be even; default 32 matches fairland).
    pub world_y: i32,
    /// Enable the market/trade system (mirrors opt_MARKET in C).
    pub opt_market: bool,
    /// Starting civilians placed in a new capital by newcap.
    pub newcap_start_civ: i16,
    /// Starting food placed in a new capital by newcap.
    pub newcap_start_food: i16,
}

/// Update schedule settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    /// Seconds between update cycles (when not using a schedule file).
    pub update_interval_secs: u64,
    /// Time window (seconds) after scheduled time in which update may fire.
    pub update_window: i32,
    /// Allow players to force an update (deity only).
    pub allow_force: bool,
    /// Emit per-sector debug logs during update ticks (toggle off in production).
    pub verbose_update: bool,
}

/// Tunable rates used during the update cycle.
/// All defaults match src/lib/global/constants.c.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateRates {
    // ── Mobility ──────────────────────────────────────────────────────────────
    /// Sector mobility accumulation per ETU.
    pub sect_mob_scale: f32,
    /// Maximum mobility sectors can hold.
    pub sect_mob_max: i32,
    /// Land unit mobility per ETU.
    pub land_mob_scale: f32,
    /// Maximum land unit mobility.
    pub land_mob_max: i32,
    /// Ship mobility per ETU.
    pub ship_mob_scale: f32,
    /// Maximum ship mobility.
    pub ship_mob_max: i32,
    /// Plane mobility per ETU.
    pub plane_mob_scale: f32,
    /// Maximum plane mobility.
    pub plane_mob_max: i32,

    // ── Money ─────────────────────────────────────────────────────────────────
    /// Tax per civilian per ETU.
    pub money_civ: f64,
    /// Tax per uncompensated worker per ETU.
    pub money_uw: f64,
    /// Tax (negative) per active soldier per ETU.
    pub money_mil: f64,
    /// Tax (negative) per reserve soldier per ETU.
    pub money_res: f64,
    /// Fraction-of-price maintenance cost per ETU for planes.
    pub money_plane: f64,
    /// Fraction-of-price maintenance cost per ETU for ships.
    pub money_ship: f64,
    /// Fraction-of-price maintenance cost per ETU for land units.
    pub money_land: f64,
    /// Bank interest per bar per ETU.
    pub bankint: f64,

    // ── Populace ──────────────────────────────────────────────────────────────
    /// Civilian birth rate.
    pub obrate: f64,
    /// Uncompensated worker birth rate.
    pub uwbrate: f64,
    /// Food eating rate per person per ETU.
    pub eatrate: f64,
    /// Food required to mature one baby.
    pub babyeat: f64,
    /// Food cultivation rate (* workforce).
    pub fcrate: f64,
    /// Food growth rate (* fertility).
    pub fgrate: f64,

    // ── Tech/Research/Education/Happiness ────────────────────────────────────
    /// Amount of tech with no production penalty.
    pub easy_tech: f32,
    /// Log base for tech production penalty above easy_tech.
    pub tech_log_base: f32,
    /// Shared tech between allies = 1/ally_factor.
    pub ally_factor: f32,
    /// ETU rate at which tech/research decay (0 = no decay).
    pub level_age_rate: f32,
    /// ETUs happiness is averaged over.
    pub hap_avg: f32,
    /// ETUs education is averaged over.
    pub edu_avg: f32,
    /// Happiness consumption factor (hap_cons civs → 1 hap level).
    pub hap_cons: f64,
    /// Education consumption factor.
    pub edu_cons: f64,

    // ── Unit growth ───────────────────────────────────────────────────────────
    /// Efficiency growth rate per ETU for land units.
    pub land_grow_scale: f32,
    /// Efficiency growth rate per ETU for ships.
    pub ship_grow_scale: f32,
    /// Efficiency growth rate per ETU for planes.
    pub plane_grow_scale: f32,
}

/// Hard limits and caps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    /// Maximum number of countries (MAXNOC).
    pub max_nations: usize,
    /// Maximum number of realms per nation.
    pub max_realms: usize,
    /// Maximum simultaneous player connections.
    pub max_connections: usize,
    /// Max minutes a country may be logged in per day.
    pub m_m_p_d: i32,
    /// Minutes before an idle session is dropped.
    pub max_idle: i32,
    /// Minutes before an idle visitor session is dropped.
    pub max_idle_visitor: i32,
    /// Seconds a client has to complete login/logout handshake.
    pub login_grace_time: i32,
}

// ── Default implementations (values from constants.c) ────────────────────────

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig::default(),
            game: GameConfig::default(),
            update: UpdateConfig::default(),
            rates: UpdateRates::default(),
            limits: LimitsConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            listen_addr: String::new(),
            port: 6665,
            data_dir: PathBuf::from("data"),
            info_dir: PathBuf::from("info"),
            schedule_file: PathBuf::from("schedule"),
            keep_journal: true,
            motd_file: PathBuf::from("motd"),
            down_file: PathBuf::from("down"),
            tel_dir: PathBuf::from("tele"),
        }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            etu_per_update: 60,
            start_cash: 20000,
            startmob: 127,
            rollover_avail_max: 50,
            anno_keep_days: 7,
            news_keep_days: 10,
            lost_keep_hours: 48,
            world_x: 64,
            world_y: 32,
            opt_market: false,
            newcap_start_civ: 500,
            newcap_start_food: 5000,
        }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig {
            update_interval_secs: 3600,
            update_window: 0,
            allow_force: false,
            verbose_update: false,
        }
    }
}

impl Default for UpdateRates {
    fn default() -> Self {
        UpdateRates {
            // Mobility (constants.c)
            sect_mob_scale: 1.0,
            sect_mob_max: 127,
            land_mob_scale: 1.0,
            land_mob_max: 127,
            ship_mob_scale: 1.5,
            ship_mob_max: 127,
            plane_mob_scale: 1.0,
            plane_mob_max: 127,
            // Money
            money_civ:   0.0083333,
            money_uw:    0.0017777,
            money_mil:  -0.0833333,
            money_res:  -0.0083333,
            money_plane:-0.001,
            money_ship: -0.001,
            money_land: -0.001,
            bankint:     0.25,
            // Populace
            obrate:   0.005,
            uwbrate:  0.0025,
            eatrate:  0.0005,
            babyeat:  0.0060,
            fcrate:   0.0013,
            fgrate:   0.0012,
            // Tech/Res/Edu/Hap
            easy_tech:       1.00,
            tech_log_base:   2.0,
            ally_factor:     2.0,
            level_age_rate: 96.0,
            // 4.4.1 used hap_avg=48 and edu_avg=192 calibrated for etu=8-24.
            // At etu=60 the moving-average window is proportionally shorter,
            // so scale both up by 60/24 = 2.5 to preserve the same number
            // of update-cycles to equilibrium as a classic 1-update/day game.
            hap_avg:        120.0,  // 48 * (60/24)
            edu_avg:        480.0,  // 192 * (60/24)
            hap_cons:   600_000.0,
            edu_cons:   600_000.0,
            // Unit growth
            land_grow_scale:  2.0,
            ship_grow_scale:  3.0,
            plane_grow_scale: 2.0,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        LimitsConfig {
            max_nations: 99,
            max_realms: 50,
            max_connections: 512,
            m_m_p_d: 1440,
            max_idle: 15,
            max_idle_visitor: 5,
            login_grace_time: 120,
        }
    }
}

/// Load configuration from a TOML file.  Missing keys use defaults.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_owned(),
        source: e,
    })?;
    toml::from_str(&text).map_err(|e| ConfigError::Parse {
        path: path.to_owned(),
        source: e,
    })
}

/// Load configuration, falling back to compiled-in defaults if file not found.
pub fn load_or_default(path: &Path) -> Config {
    match load(path) {
        Ok(cfg) => cfg,
        Err(ConfigError::Io { .. }) => Config::default(),
        Err(e) => {
            eprintln!("Warning: {e}; using defaults");
            Config::default()
        }
    }
}
