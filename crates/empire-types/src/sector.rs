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

/// Sector designation types.  UIDs and mnemonics match Empire 4.4.1 sect.config exactly.
/// The repr(u8) values are the sect.config uid column and are stored in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SectorType {
    Sea          =  0,  // .  — deep water
    Mountain     =  1,  // ^  — mountain (impassable, produces gold dust)
    Sanctuary    =  2,  // s  — sanctuary (deity-only; new player start zone)
    Wasteland    =  3,  // \  — nuclear wasteland (deity-only)
    Wilderness   =  4,  // -  — unexplored land
    Capital      =  5,  // c  — capital / city
    UraniumMine  =  6,  // u  — uranium mine
    Park         =  7,  // p  — park (happiness)
    DefensePlant =  8,  // d  — defense plant (guns)
    ShellIndus   =  9,  // i  — shell industry
    Mine         = 10,  // m  — mine (iron ore)
    GoldMine     = 11,  // g  — gold mine
    Harbor       = 12,  // h  — harbor (WPKG packing)
    Warehouse    = 13,  // w  — warehouse (WPKG packing)
    Airfield     = 14,  // *  — airfield
    Agri         = 15,  // a  — agribusiness (food)
    OilField     = 16,  // o  — oil field
    LightManuf   = 17,  // j  — light manufacturing (LCM)
    HeavyManuf   = 18,  // k  — heavy manufacturing (HCM)
    Fortress     = 19,  // f  — fortress
    TechCenter   = 20,  // t  — technical center
    ResearchLab  = 21,  // r  — research lab
    NuclearPlant = 22,  // n  — nuclear plant
    Library      = 23,  // l  — library / school (education)
    Highway      = 24,  // +  — highway
    Radar        = 25,  // )  — radar installation
    Headquarters = 26,  // !  — headquarters
    BridgeHead   = 27,  // #  — bridge head
    BridgeSpan   = 28,  // =  — bridge span
    Bank         = 29,  // b  — bank
    Refinery     = 30,  // %  — refinery (petrol)
    Enlist       = 31,  // e  — enlistment center
    Plains       = 32,  // ~  — plains
    BridgeTower  = 33,  // @  — bridge tower
}

impl SectorType {
    pub fn mnemonic(self) -> char {
        match self {
            SectorType::Sea          => '.',
            SectorType::Mountain     => '^',
            SectorType::Sanctuary    => 's',
            SectorType::Wasteland    => '\\',
            SectorType::Wilderness   => '-',
            SectorType::Capital      => 'c',
            SectorType::UraniumMine  => 'u',
            SectorType::Park         => 'p',
            SectorType::DefensePlant => 'd',
            SectorType::ShellIndus   => 'i',
            SectorType::Mine         => 'm',
            SectorType::GoldMine     => 'g',
            SectorType::Harbor       => 'h',
            SectorType::Warehouse    => 'w',
            SectorType::Airfield     => '*',
            SectorType::Agri         => 'a',
            SectorType::OilField     => 'o',
            SectorType::LightManuf   => 'j',
            SectorType::HeavyManuf   => 'k',
            SectorType::Fortress     => 'f',
            SectorType::TechCenter   => 't',
            SectorType::ResearchLab  => 'r',
            SectorType::NuclearPlant => 'n',
            SectorType::Library      => 'l',
            SectorType::Highway      => '+',
            SectorType::Radar        => ')',
            SectorType::Headquarters => '!',
            SectorType::BridgeHead   => '#',
            SectorType::BridgeSpan   => '=',
            SectorType::Bank         => 'b',
            SectorType::Refinery     => '%',
            SectorType::Enlist       => 'e',
            SectorType::Plains       => '~',
            SectorType::BridgeTower  => '@',
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
    pub che: u8,             // number of guerrillas (0..CHE_MAX=255)
    pub che_target: NatId,   // nation that CHE fights (0 = none)

    pub items: Inventory,

    pub del: [DistEntry; 26], // distribute entries (one per item + direction)
    pub mines: i16,
    pub pstage: i16,         // plague stage
    pub ptime: i16,          // plague time remaining
    pub fallout: i32,
}

pub const CHE_MAX: u8 = 255;

/// One distribute entry (direction + threshold).  ref: struct diststr in sect.h
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DistEntry {
    pub path: u8,
    pub threshold: i16,
}
