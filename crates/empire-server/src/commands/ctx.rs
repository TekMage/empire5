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

// Per-command execution context.
// Replaces the C server's `struct player *player` global that every command
// reads.  Fields match what commands need: identity, DB handle, world dims.

use empire_types::Nation;
use empire_types::coords::Coord;
use empire_db::Db;
use empire_config::Config;
use crate::subs::geo;
use crate::state::GameState;

/// Context threaded through every command handler.
pub struct CmdCtx<'a> {
    /// Country number of the issuing player.
    pub cnum: u8,
    /// Nation record of the issuing player (loaded once per command).
    pub nat: Nation,
    /// True if the player has deity (administrator) privileges.
    pub is_deity: bool,
    /// DB handle shortcut (same as &state.db; kept for ergonomics).
    pub db: &'a Db,
    /// Full game state — needed by enable/disable/shutdown commands.
    pub state: &'a GameState,
    /// Server configuration — needed by show updates and other commands.
    pub config: &'a Config,
    /// World width in sectors.
    pub world_x: i32,
    /// World height in sectors.
    pub world_y: i32,
    /// ETU per update cycle (from config).
    pub etu: i32,
}

impl<'a> CmdCtx<'a> {
    /// Format absolute coordinates as "x,y" relative to the player's origin.
    pub fn format_xy(&self, x: Coord, y: Coord) -> String {
        geo::format_xy(&self.nat, x, y, self.world_x, self.world_y)
    }

    /// Convert player-relative x to absolute.
    pub fn x_abs(&self, rel_x: Coord) -> Coord {
        geo::x_abs(&self.nat, rel_x, self.world_x)
    }

    /// Convert player-relative y to absolute.
    pub fn y_abs(&self, rel_y: Coord) -> Coord {
        geo::y_abs(&self.nat, rel_y, self.world_y)
    }

    /// Convert absolute x to player-relative.
    pub fn x_rel(&self, abs_x: Coord) -> Coord {
        geo::x_rel(&self.nat, abs_x, self.world_x)
    }

    /// Convert absolute y to player-relative.
    pub fn y_rel(&self, abs_y: Coord) -> Coord {
        geo::y_rel(&self.nat, abs_y, self.world_y)
    }
}
