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
// Ported from: include/sect.h
// Known contributors to the original:
//    Dave Pare
//    Ken Stevens, 1995
//    Steve McClure, 1998

// ref: include/sect.h

use serde::{Deserialize, Serialize};
use crate::coords::{Coord, NatId};
use crate::commodity::Inventory;

/// Sector designation types.  The mnemonic chars must match sect.config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i8)]
pub enum SectorType {
    Sea         = -1,  // .  — deep water
    Land        = 0,   // -  — blank land
    Mountain    = 1,   // ^  — mountains
    Agri        = 2,   // g  — agriculture
    Uranium     = 3,   // u  — uranium mine
    Plain       = 4,   // p  — plains (population)
    Park        = 5,   // P  — park
    Urban       = 6,   // c  — urban/capital
    Research    = 7,   // r  — research lab
    Wasteland   = 8,   // w  — wasteland
    Defense     = 9,   // d  — defensive position
    Bank        = 10,  // b  — bank
    Engineer    = 11,  // e  — engineering works
    Airfield    = 12,  // a  — airfield
    Highway     = 13,  // h  — highway
    Radar       = 14,  // j  — radar installation
    Naval       = 15,  // n  — naval base
    Missile     = 16,  // m  — missile base
    Harbor      = 17,  // *  — harbor
    Fort        = 18,  // f  — fort
    Tech        = 19,  // t  — tech center
    Bravery     = 20,  // s  — shrine of bravery  (happiness)
    LightIndus  = 21,  // l  — light industry (LCM)
    HeavyIndus  = 22,  // k  — heavy industry (HCM)
    Gold        = 23,  // G  — gold mine
    Oil         = 24,  // o  — oil field
    Unknown     = 25,  // ?  — occupied/uncharted
}

impl SectorType {
    pub fn mnemonic(self) -> char {
        match self {
            SectorType::Sea        => '.',
            SectorType::Land       => '-',
            SectorType::Mountain   => '^',
            SectorType::Agri       => 'g',
            SectorType::Uranium    => 'u',
            SectorType::Plain      => 'p',
            SectorType::Park       => 'P',
            SectorType::Urban      => 'c',
            SectorType::Research   => 'r',
            SectorType::Wasteland  => 'w',
            SectorType::Defense    => 'd',
            SectorType::Bank       => 'b',
            SectorType::Engineer   => 'e',
            SectorType::Airfield   => 'a',
            SectorType::Highway    => 'h',
            SectorType::Radar      => 'j',
            SectorType::Naval      => 'n',
            SectorType::Missile    => 'm',
            SectorType::Harbor     => '*',
            SectorType::Fort       => 'f',
            SectorType::Tech       => 't',
            SectorType::Bravery    => 's',
            SectorType::LightIndus => 'l',
            SectorType::HeavyIndus => 'k',
            SectorType::Gold       => 'G',
            SectorType::Oil        => 'o',
            SectorType::Unknown    => '?',
        }
    }
}

/// Full sector record.  ref: struct sctstr in sect.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sector {
    /// Linear index: XYOFFSET(x, y)
    pub uid: i32,
    /// Owner country number (0 = unowned)
    pub own: NatId,
    pub x: Coord,
    pub y: Coord,
    pub sector_type: SectorType,
    /// Efficiency 0–100%
    pub effic: i8,
    /// Mobility units available
    pub mobil: i8,
    /// Production stopped flag
    pub off: bool,

    pub loyal: u8,           // updates until civilians "converted"
    pub terr: [u8; 4],       // territory labels 0-3
    pub dterr: u8,           // deity territory
    pub dist_x: Coord,
    pub dist_y: Coord,
    pub avail: i16,          // available workforce
    pub flags: i16,
    pub elev: i16,
    pub work: u8,            // pct of civs actually working
    pub coastal: bool,
    pub new_type: SectorType,
    pub min: u8,             // ease of mining ore
    pub gmin: u8,            // gold ore amount
    pub fertil: u8,          // soil fertility
    pub oil: u8,             // oil content
    pub uran: u8,            // uranium ore content
    pub old_own: NatId,      // previous owner (for liberation)

    pub items: Inventory,

    pub del: [DistEntry; 26], // distribute entries (one per item + direction)
    pub mines: i16,
    pub pstage: i16,         // plague stage
    pub ptime: i16,          // plague time remaining
    pub fallout: i32,
}

/// One distribute entry (direction + threshold).  ref: struct diststr in sect.h
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DistEntry {
    pub path: u8,
    pub threshold: i16,
}
