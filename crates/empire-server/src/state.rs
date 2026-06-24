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

// Shared game state and session registry.
//
// GameState is held behind Arc<RwLock<GameState>>:
//   - Update engine acquires a write lock for the full tick duration.
//   - Player command handlers acquire a read lock (concurrent OK).
//
// SessionRegistry is a separate Arc<SessionRegistry> with its own Mutex so
// that login/logout bookkeeping never blocks on the update-engine write lock.
// Mirrors the Players queue (accept.c) and getplayer() (accept.c).

use std::collections::HashMap;
use std::sync::Mutex;

use empire_db::Db;

// ── Game state (behind Arc<RwLock>) ──────────────────────────────────────────

pub struct GameState {
    pub db: Db,
    /// Monotonically increasing update counter (ETU tick number).
    pub update_number: u64,
}

impl GameState {
    pub fn new(db: Db) -> Self {
        GameState { db, update_number: 0 }
    }
}

// ── Session registry (separate Arc<SessionRegistry>) ─────────────────────────

/// Metadata about one active player session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub cnum: u8,
    pub host_addr: String,
    pub user_id: String,
    /// Formatted as "Session-{N}" for the journal thread column.
    pub thread_name: String,
}

/// Tracks which country numbers have an active PS_PLAYING session.
/// Mirrors the Players linked-list and getplayer() in src/lib/player/accept.c.
pub struct SessionRegistry {
    inner: Mutex<HashMap<u8, SessionInfo>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        SessionRegistry { inner: Mutex::new(HashMap::new()) }
    }

    /// Attempt to register a new session for `cnum`.
    /// Returns `false` (country in use) if a session for that cnum already exists.
    pub fn try_enter(&self, info: SessionInfo) -> bool {
        let mut map = self.inner.lock().unwrap();
        if map.contains_key(&info.cnum) {
            return false;
        }
        map.insert(info.cnum, info);
        true
    }

    /// Remove the session for `cnum` (called on disconnect).
    pub fn leave(&self, cnum: u8) {
        self.inner.lock().unwrap().remove(&cnum);
    }

    /// Return a copy of the SessionInfo for `cnum` if it is currently playing.
    pub fn get(&self, cnum: u8) -> Option<SessionInfo> {
        self.inner.lock().unwrap().get(&cnum).cloned()
    }

    /// Return the number of currently active sessions.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}
