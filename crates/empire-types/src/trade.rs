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
// Ported from: include/commodity.h (struct comstr)

use crate::commodity::Item;

/// A commodity lot on the marketplace.
///
/// Maps to the `trade_items` SQLite table.
/// Corresponds to C's `struct comstr` in commodity.h, simplified for the
/// commodity-only market (not the unit-auction system in trade.h).
#[derive(Debug, Clone)]
pub struct TradeItem {
    /// Unique lot ID (auto-assigned, starts at 1).
    pub uid: i32,
    /// Seller's country number.
    pub seller: u8,
    /// Commodity type being sold.
    pub item: Item,
    /// Quantity for sale.
    pub amount: i32,
    /// Price per unit in dollars.
    pub price: f64,
    /// Seller's source sector X coordinate (absolute).
    pub from_x: i16,
    /// Seller's source sector Y coordinate (absolute).
    pub from_y: i16,
    /// Unix timestamp when the listing was created.
    pub created: i64,
    /// True once a buyer has committed to purchasing.
    pub bought: bool,
    /// Buyer's country number (valid only when `bought` is true).
    pub buyer: u8,
}

impl Item {
    /// Convert a raw integer (SQLite INTEGER column) to an `Item`.
    ///
    /// Returns `None` for values outside the valid discriminant range.
    pub fn try_from_i32(v: i32) -> Option<Item> {
        match v {
            0  => Some(Item::Civil),
            1  => Some(Item::Milit),
            2  => Some(Item::Shell),
            3  => Some(Item::Gun),
            4  => Some(Item::Petrol),
            5  => Some(Item::Iron),
            6  => Some(Item::Dust),
            7  => Some(Item::Bar),
            8  => Some(Item::Food),
            9  => Some(Item::Oil),
            10 => Some(Item::Lcm),
            11 => Some(Item::Hcm),
            12 => Some(Item::Uw),
            13 => Some(Item::Rad),
            _  => None,
        }
    }
}
