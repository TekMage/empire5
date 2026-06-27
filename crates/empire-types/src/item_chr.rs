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
//
// Ported from: src/lib/global/item.config, include/item.h

// Commodity (item) descriptor table.
// Packaging indices: [IPKG, NPKG, WPKG, UPKG, BPKG]
//   IPKG = 0 (inefficient, < 60% eff)
//   NPKG = 1 (no packaging)
//   WPKG = 2 (warehouse)
//   UPKG = 3 (urban)
//   BPKG = 4 (bank)

use crate::commodity::Item;

/// Commodity descriptor.
#[derive(Debug, Clone, Copy)]
pub struct ItemChr {
    /// Full name of the commodity.
    pub name: &'static str,
    /// Single-character mnemonic.
    pub mnemonic: char,
    /// Weight in pounds per unit.
    pub weight: i32,
    /// Packing efficiency factors: [IPKG, NPKG, WPKG, UPKG, BPKG].
    pub packing: [i32; 5],
    /// Power contribution per item (used in power reports).
    pub power: f64,
}

// Static item descriptor table indexed by Item discriminant.
// Packing columns: [IPKG, NPKG, WPKG, UPKG, BPKG] — from item.config in Empire 4.4.1.
// WPKG (index 2) = warehouse packing: most items pack 10× denser in/through a harbor or warehouse.
const ICHR: &[ItemChr] = &[
    // 0: Civil (c)
    ItemChr { name: "civilians",                    mnemonic: 'c', weight: 1,  packing: [1, 10, 10, 10, 10], power: 0.5  },
    // 1: Milit (m)
    ItemChr { name: "military",                     mnemonic: 'm', weight: 1,  packing: [1,  1,  1,  1,  1], power: 4.0  },
    // 2: Shell (s)
    ItemChr { name: "shells",                       mnemonic: 's', weight: 1,  packing: [1,  1, 10,  1,  1], power: 2.0  },
    // 3: Gun (g)
    ItemChr { name: "guns",                         mnemonic: 'g', weight: 10, packing: [1,  1, 10,  1,  1], power: 50.0 },
    // 4: Petrol (p)
    ItemChr { name: "petrol",                       mnemonic: 'p', weight: 1,  packing: [1,  1, 10,  1,  1], power: 0.5  },
    // 5: Iron (i)
    ItemChr { name: "iron ore",                     mnemonic: 'i', weight: 1,  packing: [1,  1, 10,  1,  1], power: 0.5  },
    // 6: Dust (d)
    ItemChr { name: "gold dust",                    mnemonic: 'd', weight: 5,  packing: [1,  1, 10,  1,  1], power: 2.5  },
    // 7: Bar (b)
    ItemChr { name: "gold bars",                    mnemonic: 'b', weight: 50, packing: [1,  1,  5,  1,  4], power: 25.0 },
    // 8: Food (f)
    ItemChr { name: "food",                         mnemonic: 'f', weight: 1,  packing: [1,  1, 10,  1,  1], power: 0.5  },
    // 9: Oil (o)
    ItemChr { name: "oil",                          mnemonic: 'o', weight: 1,  packing: [1,  1, 10,  1,  1], power: 0.5  },
    // 10: Lcm (l)
    ItemChr { name: "light construction materials", mnemonic: 'l', weight: 1,  packing: [1,  1, 10,  1,  1], power: 0.5  },
    // 11: Hcm (h)
    ItemChr { name: "heavy construction materials", mnemonic: 'h', weight: 2,  packing: [1,  1, 10,  1,  1], power: 1.0  },
    // 12: Uw (u)
    ItemChr { name: "undesirables",                 mnemonic: 'u', weight: 2,  packing: [1,  1,  2,  1,  1], power: 1.0  },
    // 13: Rad (r)
    ItemChr { name: "radioactive materials",        mnemonic: 'r', weight: 8,  packing: [1,  1, 10,  1,  1], power: 4.0  },
];

impl ItemChr {
    /// Return the descriptor for the given item type.
    pub fn for_item(item: Item) -> &'static ItemChr {
        &ICHR[item as usize]
    }

    /// Return all item descriptors in order.
    pub fn all() -> &'static [ItemChr] {
        ICHR
    }

    /// Number of item types.
    pub fn count() -> usize {
        ICHR.len()
    }
}
