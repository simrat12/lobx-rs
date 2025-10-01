//! Pure conversions between in-memory `Book` and the serializable snapshot types.
//!
//! This file MUST NOT talk to the database. Only struct <-> struct mapping lives here.

use crate::engine::book::Book;
use crate::engine::types::{Resting, Side};
use crate::persist::types::{
    PersistResult, SnapshotData, SnapshotLevel, SnapshotResting, SNAPSHOT_SCHEMA_VERSION, WalOp,
};
use std::collections::VecDeque;

/// Build a `SnapshotData` from the current in-memory `Book`.
/// - Do NOT set `wal_high_watermark` (leave it 0 here; DB layer stamps it when saving).
pub fn from_book(book: &Book) -> SnapshotData {
    // STEP 1: create Vec<SnapshotLevel> for bids.
    //   - Iterate book.bids (BTreeMap<i64, VecDeque<Resting>>), highest-to-lowest price or any order.
    //   - For each (price, queue), map each `Resting` to `SnapshotResting` by copying fields:
    //       id, quantity, ts, remaining, active
    //   - Push SnapshotLevel { price, orders } into a Vec.
    //
    let mut bid_side: Vec<SnapshotLevel> = Vec::new();
    let mut ask_side: Vec<SnapshotLevel> = Vec::new();

    for i in &book.bids {
        let price = *i.0;
        let mut orders: Vec<SnapshotResting> = Vec::new();
        for j in i.1 {
            let snap_resting = SnapshotResting {
                id: j.id,
                quantity: j.quantity,
                ts: j.ts,
                remaining: j.remaining,
                active: j.active,
            };
            orders.push(snap_resting);
        }
        let snap_level = SnapshotLevel { price, orders };
        bid_side.push(snap_level);
    }

    for i in &book.asks {
        let price = *i.0;
        let mut orders: Vec<SnapshotResting> = Vec::new();
        for j in i.1 {
            let snap_resting = SnapshotResting {
                id: j.id,
                quantity: j.quantity,
                ts: j.ts,
                remaining: j.remaining,
                active: j.active,
            };
            orders.push(snap_resting);
        }
        let snap_level = SnapshotLevel { price, orders };
        ask_side.push(snap_level);
    }


    // STEP 4: construct SnapshotData:
    //   - version = SNAPSHOT_SCHEMA_VERSION
    //   - bid_side, ask_side from above
    //   - id_index from above
    //   - next_order_id = book.next_order_id
    //   - wal_high_watermark = 0  (DB layer fills it at save time)
    //
    // RETURN: the SnapshotData value.


    SnapshotData {
        version: SNAPSHOT_SCHEMA_VERSION,
        bid_side,
        ask_side,
        next_order_id: book.next_order_id,
        wal_high_watermark: 0,
    }
}

/// Apply a previously saved snapshot into a fresh `Book`.
/// - Clears the book and repopulates all structures from the snapshot payload.
pub fn apply_to_book(book: &mut Book, snap: &SnapshotData) -> PersistResult<()> {
    // STEP 1: clear existing state:
    //   book.bids.clear();
    //   book.asks.clear();

    book.bids.clear();
    book.asks.clear();
    //
    // STEP 2: rebuild bids
    //   - For each SnapshotLevel in snap.bid_side:
    //       * create a VecDeque<Resting>
    //       * for each SnapshotResting, create Resting with:
    //           id, price = Some(level.price), quantity, ts, remaining, active
    //       * insert into book.bids at key = level.price
    //
    // STEP 3: rebuild asks (mirror of bids)

    for i in &snap.bid_side {
        let price = i.price;
        let mut orders: VecDeque<Resting> = VecDeque::new();
        for j in &i.orders {
            let resting = Resting {
                id: j.id,
                price: Some(price),
                quantity: j.quantity,
                ts: j.ts,
                remaining: j.remaining,
                active: j.active,
            };
            orders.push_back(resting);
        }
        book.bids.insert(price, orders);
    }

    for i in &snap.ask_side {
        let price = i.price;
        let mut orders: VecDeque<Resting> = VecDeque::new();
        for j in &i.orders {
            let resting = Resting {
                id: j.id,
                price: Some(price),
                quantity: j.quantity,
                ts: j.ts,
                remaining: j.remaining,
                active: j.active,
            };
            orders.push_back(resting);
        }
        book.asks.insert(price, orders);
    }

    // STEP 5: set next_order_id
    //   - book.next_order_id = snap.next_order_id;
    //

    book.next_order_id = snap.next_order_id;
    // STEP 6: return Ok(())

    // Placholder for now
    Ok(())
}

/// (Optional for now) Apply a single WAL operation to the in-memory `Book`.
/// Use this during startup replay to catch up from the snapshot.
pub fn apply_op(_book: &mut Book, _op: &WalOp) -> PersistResult<()> {
    // Placholder for now
    Ok(())
}
