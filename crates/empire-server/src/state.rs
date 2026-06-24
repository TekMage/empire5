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

// Shared game state.  Held behind Arc<RwLock<GameState>>.
// The update engine acquires a write lock for the duration of each update.
// Player command handlers acquire a read lock (concurrent reads are fine).

use empire_db::Db;

pub struct GameState {
    pub db: Db,
    /// Monotonically increasing update counter (ETU tick number).
    pub update_number: u64,
    /// Whether the server is accepting new connections.
    pub accepting: bool,
}

impl GameState {
    pub fn new(db: Db) -> Self {
        GameState {
            db,
            update_number: 0,
            accepting: true,
        }
    }
}
