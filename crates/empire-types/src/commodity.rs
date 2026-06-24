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
// Ported from: include/item.h
// Known contributors to the original:
//    Markus Armbruster, 2004-2020

// ref: include/item.h

use serde::{Deserialize, Serialize};

/// Commodity types.  Order must match `item.config` in the reference server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i8)]
pub enum Item {
    Civil   = 0,  // c — civilians
    Milit   = 1,  // m — military
    Shell   = 2,  // s — shells
    Gun     = 3,  // g — guns
    Petrol  = 4,  // p — petrol
    Iron    = 5,  // i — iron ore
    Dust    = 6,  // d — gold dust
    Bar     = 7,  // b — gold bars
    Food    = 8,  // f — food
    Oil     = 9,  // o — oil
    Lcm     = 10, // l — light construction materials
    Hcm     = 11, // h — heavy construction materials
    Uw      = 12, // u — undesirables (workforce slaves)
    Rad     = 13, // r — radioactive materials
}

impl Item {
    pub const MAX: Item = Item::Rad;
    pub const COUNT: usize = 14;

    pub fn mnemonic(self) -> char {
        match self {
            Item::Civil  => 'c',
            Item::Milit  => 'm',
            Item::Shell  => 's',
            Item::Gun    => 'g',
            Item::Petrol => 'p',
            Item::Iron   => 'i',
            Item::Dust   => 'd',
            Item::Bar    => 'b',
            Item::Food   => 'f',
            Item::Oil    => 'o',
            Item::Lcm    => 'l',
            Item::Hcm    => 'h',
            Item::Uw     => 'u',
            Item::Rad    => 'r',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::Civil  => "civilians",
            Item::Milit  => "military",
            Item::Shell  => "shells",
            Item::Gun    => "guns",
            Item::Petrol => "petrol",
            Item::Iron   => "iron ore",
            Item::Dust   => "gold dust",
            Item::Bar    => "gold bars",
            Item::Food   => "food",
            Item::Oil    => "oil",
            Item::Lcm    => "light construction materials",
            Item::Hcm    => "heavy construction materials",
            Item::Uw     => "undesirables",
            Item::Rad    => "radioactive materials",
        }
    }

    pub fn from_mnemonic(c: char) -> Option<Item> {
        match c {
            'c' | 'C' => Some(Item::Civil),
            'm' | 'M' => Some(Item::Milit),
            's' | 'S' => Some(Item::Shell),
            'g' | 'G' => Some(Item::Gun),
            'p' | 'P' => Some(Item::Petrol),
            'i' | 'I' => Some(Item::Iron),
            'd' | 'D' => Some(Item::Dust),
            'b' | 'B' => Some(Item::Bar),
            'f' | 'F' => Some(Item::Food),
            'o' | 'O' => Some(Item::Oil),
            'l' | 'L' => Some(Item::Lcm),
            'h' | 'H' => Some(Item::Hcm),
            'u' | 'U' => Some(Item::Uw),
            'r' | 'R' => Some(Item::Rad),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Packaging bonus categories.  ref: enum i_packing in item.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Packaging {
    Inefficient,  // IPKG — efficiency < 60%
    None,         // NPKG — no special packaging
    Warehouse,    // WPKG
    Urban,        // UPKG
    Bank,         // BPKG
}

/// Fixed-size commodity storage array (one slot per Item).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory(pub [i16; Item::COUNT]);

impl Inventory {
    pub fn zero() -> Self {
        Inventory([0; Item::COUNT])
    }

    pub fn get(&self, item: Item) -> i16 {
        self.0[item.index()]
    }

    pub fn set(&mut self, item: Item, value: i16) {
        self.0[item.index()] = value;
    }

    pub fn add(&mut self, item: Item, delta: i16) {
        self.0[item.index()] = self.0[item.index()].saturating_add(delta);
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::zero()
    }
}
