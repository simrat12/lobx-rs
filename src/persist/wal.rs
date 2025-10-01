
use crate::persist::types::{PersistResult, PersistanceError, WalOp};

/// Convert a WAL op into a JSON string 
pub fn op_to_json(op: &WalOp) -> PersistResult<String> {
    // STEP 1: use serde_json::to_string(op)
    // STEP 2: map serde errors to PersistanceError::SerializationFailure
    //

    let json_string = serde_json::to_string(op)
        .map_err(|_| PersistanceError::SerializationFailure)?;


    Ok(json_string)
}

/// Parse a WAL op JSON (read back from the DB) into a `WalOp` value.
pub fn op_from_json(s: &str) -> PersistResult<WalOp> {
    // STEP 1: use serde_json::from_str::<WalOp>(s)
    // STEP 2: map serde errors to PersistanceError::FormatMismatch
    //
    // RETURN: Ok(WalOp)
    let wal_op: WalOp = serde_json::from_str(s)
        .map_err(|_| PersistanceError::FormatMismatch)?;

    PersistResult::Ok(wal_op)
}
