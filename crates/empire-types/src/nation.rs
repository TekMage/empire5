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
// Ported from: include/nat.h
// Known contributors to the original:
//    Thomas Ruschak
//    Ken Stevens, 1995
//    Steve McClure, 1998-2000

// ref: include/nat.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};

/// Nation status progression.  Order matters — the C code uses inequality comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum NatStatus {
    Unused  = 0,  // STAT_UNUSED — slot available
    New     = 1,  // STAT_NEW    — just initialized
    Visitor = 2,  // STAT_VIS    — visitor account
    Sanct   = 3,  // STAT_SANCT  — still in sanctuary
    Active  = 4,  // STAT_ACTIVE — sanctuary broken, playing
    Deity   = 5,  // STAT_GOD    — deity (admin) powers
}

impl NatStatus {
    pub fn is_playable(self) -> bool {
        self >= NatStatus::Visitor
    }
    pub fn is_active(self) -> bool {
        self >= NatStatus::Active
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NatFlags: u32 {
        const FLASH      = 0x0001;
        const BEEP       = 0x0002;
        const COASTWATCH = 0x0004;
        const SONAR      = 0x0008;
        const TECHLISTS  = 0x0010;
        /// Inline telegram notification (NF_INFORM)
        const INFORM     = 0x0020;
    }
}
impl serde::Serialize for NatFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.bits())
    }
}
impl<'de> serde::Deserialize<'de> for NatFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bits = u32::deserialize(d)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

/// One realm (bounding box) owned by a nation.  ref: struct realmstr
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Realm {
    pub uid: i32,
    pub cnum: NatId,
    pub realm: u16,
    pub xl: Coord,
    pub xh: Coord,
    pub yl: Coord,
    pub yh: Coord,
}

/// Full nation record.  ref: struct natstr in nat.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nation {
    pub uid: i32,
    pub cnum: NatId,
    pub status: NatStatus,
    pub flags: NatFlags,

    /// Country name (≤ 19 UTF-8 chars; C used fixed char[20])
    pub name: String,
    /// Representative / contact name
    pub representative: String,
    /// Last login host address
    pub host_addr: String,
    /// Last login Unix user ID (may be empty)
    pub user_id: String,

    /// Capital location (absolute coordinates)
    pub xcap: Coord,
    pub ycap: Coord,
    /// Origin (lower-left of player's relative coordinate frame)
    pub xorg: Coord,
    pub yorg: Coord,

    // Economic state
    pub money: i32,
    pub reserve: i32,       // mil reserve
    pub tech: f64,
    pub research: f64,
    pub education: f64,
    pub happiness: f64,

    // Login tracking
    pub login_count: i32,
    pub tele_cnt: i32,      // # personal telegrams waiting
    pub ann_cnt:  i32,      // # unread announcements
    pub last_ann_read: i64, // unix timestamp: last time announces were read

    // Authentication (Phase 2)
    /// bcrypt hash of the nation password; empty = no password set
    pub passwd_hash: String,
    /// Unix timestamp of last successful login (0 = never)
    pub last_login: i64,
    /// Unix timestamp of last logout (0 = never)
    pub last_logout: i64,
    /// Unix timestamp of when player last read news (0 = never; nat_newstim in 4.4.1)
    pub news_time: i64,
}

impl Nation {
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub fn is_deity(&self) -> bool {
        self.status == NatStatus::Deity
    }
}
