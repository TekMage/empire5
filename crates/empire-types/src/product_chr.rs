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
// Ported from: src/lib/global/product.config, include/product.h
// Known contributors to the original:
//    Markus Armbruster, 2006-2021

// Product descriptor table.  Rust equivalent of the C `pchr[]` array
// (struct pchrstr, loaded from product.config).
//
// There are 15 products (indices 0-14); indexed by ProdIndex from sector_chr.rs.
// Access via `ProductChr::get(idx)`.

use crate::commodity::Item;

/// The four nation-level indices that products can contribute to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatLevel {
    Tech       = 0,  // NAT_TLEV
    Research   = 1,  // NAT_RLEV
    Education  = 2,  // NAT_ELEV
    Happiness  = 3,  // NAT_HLEV
}

/// A single material input slot (up to MAXPRCON = 3 per product).
#[derive(Debug, Clone, Copy)]
pub struct MaterialInput {
    /// The item type consumed.
    pub item: Item,
    /// Amount consumed per unit output.
    pub amount: i32,
}

/// Natural resource type consumed by mining products.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    None,
    Min,    // iron ore deposits
    Gold,   // gold deposits
    Fert,   // soil fertility
    OilRes, // oil deposits
    Uran,   // uranium deposits
}

/// Per-product descriptor.  ref: struct pchrstr in include/product.h
#[derive(Debug, Clone)]
pub struct ProductChr {
    /// Short name used in reports (e.g., "iron", "food").
    pub sname: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Material inputs (up to 3; item = Civil means "none used").
    pub inputs: [Option<MaterialInput>; 3],
    /// Work units required to produce one unit of output.
    pub bwork: i32,
    /// The item type produced (None = this product updates a nation level).
    pub item: Option<Item>,
    /// The nation level this product contributes to (if item is None).
    pub level: Option<NatLevel>,
    /// Cash cost per unit output.
    pub cost: i32,
    /// Natural resource slot this product depletes (if any).
    pub resource: Resource,
    /// Resource depletion rate per 100 units output.
    pub nrdep: i32,
    /// Nation level required (NAT_?LEV) for production to be efficient.
    pub nlndx: Option<NatLevel>,
    /// Minimum level required to start production.
    pub nlmin: i32,
    /// Level lag: how much extra level above nlmin is needed for full p.e.
    pub nllag: i32,
}

// ── Static product table (product.config order, uid 0-14) ────────────────────

const PCHR: &[ProductChr] = &[
    // 0: iron ore — mines Min resource → Iron
    ProductChr {
        sname: "iron", name: "iron ore",
        inputs: [None, None, None],
        bwork: 1, item: Some(Item::Iron), level: None, cost: 0,
        resource: Resource::Min, nrdep: 0, nlndx: None, nlmin: 0, nllag: 0,
    },
    // 1: gold dust — mines Gold resource → Dust
    ProductChr {
        sname: "dust", name: "gold dust",
        inputs: [None, None, None],
        bwork: 1, item: Some(Item::Dust), level: None, cost: 0,
        resource: Resource::Gold, nrdep: 20, nlndx: None, nlmin: 0, nllag: 0,
    },
    // 2: food — mines Fert resource → Food; needs tech >= -10
    ProductChr {
        sname: "food", name: "food",
        inputs: [None, None, None],
        bwork: 1, item: Some(Item::Food), level: None, cost: 0,
        resource: Resource::Fert, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: -10, nllag: 10,
    },
    // 3: oil — mines OilRes → Oil; needs tech >= -10
    ProductChr {
        sname: "oil", name: "oil",
        inputs: [None, None, None],
        bwork: 1, item: Some(Item::Oil), level: None, cost: 0,
        resource: Resource::OilRes, nrdep: 10, nlndx: Some(NatLevel::Tech), nlmin: -10, nllag: 10,
    },
    // 4: rad — mines Uran → Rad; needs tech >= 40
    ProductChr {
        sname: "rad", name: "radioactive materials",
        inputs: [None, None, None],
        bwork: 1, item: Some(Item::Rad), level: None, cost: 2,
        resource: Resource::Uran, nrdep: 35, nlndx: Some(NatLevel::Tech), nlmin: 40, nllag: 10,
    },
    // 5: shells — Lcm*2 + Hcm*1 → Shell; needs tech >= 20
    ProductChr {
        sname: "shells", name: "shells",
        inputs: [
            Some(MaterialInput { item: Item::Lcm, amount: 2 }),
            Some(MaterialInput { item: Item::Hcm, amount: 1 }),
            None,
        ],
        bwork: 3, item: Some(Item::Shell), level: None, cost: 3,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: 20, nllag: 10,
    },
    // 6: guns — Oil*1 + Lcm*5 + Hcm*10 → Gun; needs tech >= 20
    ProductChr {
        sname: "guns", name: "guns",
        inputs: [
            Some(MaterialInput { item: Item::Oil, amount: 1 }),
            Some(MaterialInput { item: Item::Lcm, amount: 5 }),
            Some(MaterialInput { item: Item::Hcm, amount: 10 }),
        ],
        bwork: 16, item: Some(Item::Gun), level: None, cost: 30,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: 20, nllag: 10,
    },
    // 7: petrol — Oil*1 → Petrol; needs tech >= 20
    ProductChr {
        sname: "petrol", name: "petrol",
        inputs: [
            Some(MaterialInput { item: Item::Oil, amount: 1 }),
            None, None,
        ],
        bwork: 1, item: Some(Item::Petrol), level: None, cost: 1,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: 20, nllag: 10,
    },
    // 8: bars — Dust*5 → Bar; needs no level
    ProductChr {
        sname: "bars", name: "gold bars",
        inputs: [
            Some(MaterialInput { item: Item::Dust, amount: 5 }),
            None, None,
        ],
        bwork: 5, item: Some(Item::Bar), level: None, cost: 10,
        resource: Resource::None, nrdep: 0, nlndx: None, nlmin: 0, nllag: 0,
    },
    // 9: lcm — Iron*1 → Lcm; needs tech >= -10
    ProductChr {
        sname: "lcm", name: "light construction materials",
        inputs: [
            Some(MaterialInput { item: Item::Iron, amount: 1 }),
            None, None,
        ],
        bwork: 1, item: Some(Item::Lcm), level: None, cost: 0,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: -10, nllag: 10,
    },
    // 10: hcm — Iron*2 → Hcm; needs tech >= -10
    ProductChr {
        sname: "hcm", name: "heavy construction materials",
        inputs: [
            Some(MaterialInput { item: Item::Iron, amount: 2 }),
            None, None,
        ],
        bwork: 2, item: Some(Item::Hcm), level: None, cost: 0,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Tech), nlmin: -10, nllag: 10,
    },
    // 11: tech — Dust*1 + Oil*5 + Lcm*10 → Tech level; needs edu >= 5
    ProductChr {
        sname: "tech", name: "technological breakthroughs",
        inputs: [
            Some(MaterialInput { item: Item::Dust, amount: 1 }),
            Some(MaterialInput { item: Item::Oil,  amount: 5 }),
            Some(MaterialInput { item: Item::Lcm, amount: 10 }),
        ],
        bwork: 16, item: None, level: Some(NatLevel::Tech), cost: 300,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Education), nlmin: 5, nllag: 10,
    },
    // 12: medical — Dust*1 + Oil*5 + Lcm*10 → Research level; needs edu >= 5
    ProductChr {
        sname: "medical", name: "medical discoveries",
        inputs: [
            Some(MaterialInput { item: Item::Dust, amount: 1 }),
            Some(MaterialInput { item: Item::Oil,  amount: 5 }),
            Some(MaterialInput { item: Item::Lcm, amount: 10 }),
        ],
        bwork: 16, item: None, level: Some(NatLevel::Research), cost: 90,
        resource: Resource::None, nrdep: 0, nlndx: Some(NatLevel::Education), nlmin: 5, nllag: 10,
    },
    // 13: edu — Lcm*1 → Education level; needs no level
    ProductChr {
        sname: "edu", name: "a class of graduates",
        inputs: [
            Some(MaterialInput { item: Item::Lcm, amount: 1 }),
            None, None,
        ],
        bwork: 1, item: None, level: Some(NatLevel::Education), cost: 9,
        resource: Resource::None, nrdep: 0, nlndx: None, nlmin: 0, nllag: 0,
    },
    // 14: happy — Lcm*1 → Happiness level; needs no level
    ProductChr {
        sname: "happy", name: "happy strollers",
        inputs: [
            Some(MaterialInput { item: Item::Lcm, amount: 1 }),
            None, None,
        ],
        bwork: 1, item: None, level: Some(NatLevel::Happiness), cost: 9,
        resource: Resource::None, nrdep: 0, nlndx: None, nlmin: 0, nllag: 0,
    },
];

impl ProductChr {
    /// Return the descriptor for product index `idx`.
    /// Returns None for invalid indices (including PRD_NONE = -1).
    pub fn get(idx: i8) -> Option<&'static ProductChr> {
        if idx < 0 { return None; }
        PCHR.get(idx as usize)
    }

    pub fn count() -> usize { PCHR.len() }
}
