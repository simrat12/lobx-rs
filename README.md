# lobx-rs

A Rust-based low-latency exchange connector and in-memory limit order book (LOB).
This project is being developed as part of a bootcamp capstone with the long-term vision of evolving into a production-grade trading infrastructure component.

## 🚀 End Goal

By the end of this project, the service will:

- Maintain a price–time–priority limit order book in-memory.
- Ingest exchange feeds (simulated first; then Binance/Coinbase and others via WebSocket).
- Support submitting/cancelling client orders through a mock gateway.
- Emit structured events (Ack, Fill, Done) for every order lifecycle.
- Track latency & throughput with benchmarks.
- Run a simple micro-strategy (e.g. VWAP or market-making stub).
- Include resilience features: reconnect logic, snapshot/restore, and unit tests for correctness.

## 🏗️ Architecture Overview

```
            [ Exchange Feeds / Simulated Input ]
                           │
                           ▼
               +-----------------------+
               |   Normalizers         |
               |  (per venue adapter)  |
               +-----------------------+
                           │
                           ▼
               +-----------------------+
               |   In-Memory Book      |
               | (BTreeMap Levels,     |
               |  Price-Time Priority) |
               +-----------------------+
                           │
          ┌────────────────┴────────────────┐
          ▼                                 ▼
+-------------------+              +--------------------+
|  Client Orders    |              |  Order Lifecycle   |
|  (limit/market)   |──► Submit ──►|  Events            |
|                   |              |  Ack / Fill / Done |
+-------------------+              +--------------------+
          │                                 │
          │                                 ▼
          │                          +---------------+
          │                          |  OMS & Logs   |
          │                          +---------------+
          ▼
+-------------------+
| Mock Gateway /    |
| Strategy Driver   |
+-------------------+
```

- **Book**: local state of bids/asks for fast querying and order matching.
- **Events**: audit trail & feed into strategy / OMS.
- **Connectors**: simulate or connect to real exchanges.

## 📂 Repo Layout

```
src/
 └── engine/
      ├── types.rs   # Core domain types (Order, Resting, Event, Fill, DoneReason, SubmitResult)
      └── book.rs    # Book implementation: submit logic, matching, best bid/ask, spread
 └── main.rs         # CLI demo interface (enter limit/market orders via terminal)
```

## ✅ Completed Components

### types.rs

Defines the domain model:

- **Side**: BUY / SELL
- **Order**: submitted by clients (limit or market, with id, price, quantity)
- **Resting**: order stored in the book with mutable remaining, active flag
- **Fill**: execution record (taker vs maker, price, qty)
- **Event**: event stream (Ack, Fill, Done)
- **SubmitResult**: wraps the event sequence for each submission

### book.rs

Implements the order book logic:

- Backed by `BTreeMap<i64, Level>` for both bids and asks
- Each Level holds a FIFO `VecDeque<Resting>` to enforce price–time priority

**Methods:**
- `new()`: initialize with dummy levels
- `best_bid()` / `best_ask()`: query top of book
- `spread()`: difference between best bid/ask
- `submit(Order) -> SubmitResult`:
  - Limit orders: rest in book or match if crossing
  - Market orders: execute against opposite side
  - Emits Done (and now Ack + Fill as enhancements)

## 🎮 CLI Demo

> **Note**: The current `main.rs` and the architecture diagram above are AI-generated for convenience and represent the intended design. The actual `main.rs` is currently just a quick and dirty CLI demo - sorry! 😅

Run the book live from your terminal:

```bash
cargo run
```

**Example session:**

```
LOBX demo. Commands:
  limit BUY  <price> <qty>
  limit SELL <price> <qty>
  market BUY  <qty>
  market SELL <qty>
  top
  quit

> limit SELL 10 100
events: [Done { id: 1, reason: Rested, ts: ... }]
TOP: BID=None  ASK=(10, 100)

> market BUY 10
events: [Ack { id: 2, ts: ... }, Fill { taker_id: 2, maker_id: 1, price: 10, qty: 10, ts: ... }, Done { id: 2, reason: Filled, ts: ... }]
TOP: BID=None  ASK=(10, 90)
```

## 🔜 Next Steps

- ✅ Emit Fill events for all matches (in progress)
- ✅ Implement cancel flow (Done::Cancelled)
- ✅ Add multiple-level matching (walk the book until counter=0 or limit exceeded)
- ✅ Integrate latency measurement & benchmarks
- ⏳ Add snapshot/restore for persistence
- ⏳ Connect to real exchange feeds (Binance, Coinbase)

## UnifiedBook demo (internal + external view)

This demo shows how a tiny read-only facade (`UnifiedBook`) merges:

- **Internal book**: your own working orders (in-memory `Book`)
- **External book**: live Hyperliquid depth (`ExternalBook`)

Run:
```bash
cargo run -- --unified-demo
```

You'll see the external BBO and the combined BBO printed every second.
The demo injects one local BUY slightly above the external best bid; the combined BBO moves immediately, showing how your intent changes your actionable view without modifying external data.

### Notes
* Keep this intentionally minimal — the UnifiedBook is a **query layer**, not a matcher.
* No changes to matching logic, persistence, or adapter code were made beyond a new demo entry point and a small helper module.
* Prices printed are integer ticks (scaled). You can format them later for UI.

## 📌 Bootcamp milestone

A working in-memory limit order book with CLI demo and event emission.
