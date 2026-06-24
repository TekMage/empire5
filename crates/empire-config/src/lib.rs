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
// Ported from: include/econfig-spec.h, include/optlist.h
// Known contributors to the original:
//    Julian Onions (optlist)
//    Ken Stevens, 1995
//    Marc Olzheim, 2004
//    Steve McClure, 1998
//    Markus Armbruster, 2004-2020

// ref: include/econfig-spec.h, include/optlist.h
//
// The original econfig key-value format is replaced by TOML for readability
// and editor tooling support.  Key names are preserved where sensible.

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
    pub limits: LimitsConfig,
}

/// Network and filesystem paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// IP address the server binds (empty string = all interfaces).
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
    /// Path to journal file.
    pub journal_file: PathBuf,
    /// Message-of-the-day file shown at login.
    pub motd_file: PathBuf,
    /// Downtime message file.
    pub down_file: PathBuf,
    /// Telegram directory.
    pub tel_dir: PathBuf,
}

/// Game-balance parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    /// ETU (Empire Time Units) per update cycle.
    pub etu_per_update: i32,
    /// Technology multiplier (float, default 1.0).
    pub tech_pop: f64,
    /// Production rate for light construction materials.
    pub lcm_per_etu: f64,
    /// Production rate for heavy construction materials.
    pub hcm_per_etu: f64,
    /// Nuclear fallout decay rate.
    pub fallout_spread: f64,
    /// Mobility gain per ETU.
    pub mob_scale: f64,
    /// Maximum mobility a unit/sector can accumulate.
    pub mob_max: i32,
    /// Starting money for a new nation.
    pub start_money: i32,
    /// Minimum efficiency at which ships can be built.
    pub ship_mineff: i8,
    /// Minimum efficiency at which planes can be built.
    pub plane_mineff: i8,
    /// Minimum efficiency at which land units can be built.
    pub land_mineff: i8,
}

/// Update schedule settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    /// Seconds between update cycles (when not using a schedule file).
    pub update_interval_secs: u64,
    /// Allow players to force an update (deity only).
    pub allow_force: bool,
}

/// Hard limits and caps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    /// Maximum number of countries.  Hardcoded 99 in C (MAXNOC).
    pub max_nations: usize,
    /// Maximum number of realms per nation.
    pub max_realms: usize,
    /// Maximum simultaneous player connections.
    pub max_connections: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig::default(),
            game: GameConfig::default(),
            update: UpdateConfig::default(),
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
            journal_file: PathBuf::from("journal"),
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
            tech_pop: 1.0,
            lcm_per_etu: 1.0,
            hcm_per_etu: 1.0,
            fallout_spread: 0.5,
            mob_scale: 1.0,
            mob_max: 127,
            start_money: 20000,
            ship_mineff: 20,
            plane_mineff: 10,
            land_mineff: 10,
        }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig {
            update_interval_secs: 3600,
            allow_force: false,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        LimitsConfig {
            max_nations: 99,
            max_realms: 50,
            max_connections: 512,
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
