# LOBX Snapshot/Restore Demo Script

This script demonstrates the complete snapshot/restore workflow for the LOBX order book system.

## Prerequisites

1. Start PostgreSQL database:
```bash
docker-compose -f docker-compose.db.yml up -d
```

2. Set environment variables:
```bash
export DATABASE_URL="postgresql://lobx:lobx@localhost:5432/lobx"
export LOBX_SYMBOL="BTC-USD"
```

## Demo Steps

### Step 1: Start the Application
```bash
cargo run
```

### Step 2: Build Some Order Book State
```
LOBX CLI> help
LOBX CLI> 1
Enter price and quantity (e.g., '100 10'): 50000 100
LOBX CLI> 2  
Enter price and quantity (e.g., '100 10'): 51000 50
LOBX CLI> 1
Enter price and quantity (e.g., '100 10'): 49500 75
LOBX CLI> 2
Enter price and quantity (e.g., '100 10'): 52000 25
```

### Step 3: View Current State
```
LOBX CLI> top
```

### Step 4: Create Snapshot
```
LOBX CLI> snapshot
```

You should see output like:
```
Creating snapshot...
✅ Saved snapshot for BTC-USD: 2 bid levels, 2 ask levels, 4 total orders

=== Book State Summary ===
Bid levels: 2, Ask levels: 2, Total orders: 4
Best bid: 100 @ 50000
Best ask: 50 @ 51000
Spread: 1000
========================
```

### Step 5: Add More Orders (to show difference)
```
LOBX CLI> 1
Enter price and quantity (e.g., '100 10'): 49000 200
LOBX CLI> top
```

### Step 6: Restore from Snapshot
```
LOBX CLI> restore
```

You should see the book restored to the state from Step 4, without the additional order from Step 5.

### Step 7: Verify Restoration
```
LOBX CLI> top
```

The book should now show the same state as before Step 5.

### Step 8: Exit
```
LOBX CLI> quit
```

## Expected Behavior

- **Snapshot**: Captures the current state of the order book and saves it to the database
- **Restore**: Loads the latest snapshot and completely replaces the current book state
- **State Summary**: Shows bid/ask levels, total orders, best bid/ask, and spread
- **Persistence**: The snapshot survives application restarts

## Key Features Demonstrated

1. **State Capture**: The `snapshot` command captures the complete order book state
2. **State Restoration**: The `restore` command rebuilds the book from the saved snapshot  
3. **Visual Feedback**: Clear output showing what was captured/restored
4. **Persistence**: Data survives application restarts
5. **Complete Workflow**: Full cycle of save → modify → restore → verify

This demonstrates that the order book system can reliably save and restore its state, which is crucial for production trading systems.
ToDo: Incorporate WAL.
