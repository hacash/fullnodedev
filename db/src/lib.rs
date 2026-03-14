use std::path::*;
use std::sync::OnceLock;

use basis::interface::*;
use sys::Rerr;

/*****************************/

#[cfg(all(feature = "db-sled", feature = "db-rusty-leveldb"))]
compile_error!("db cannot be enabled at the same time");

#[cfg(all(feature = "db-sled", feature = "db-leveldb-sys"))]
compile_error!("db cannot be enabled at the same time");

#[cfg(all(feature = "db-sled", feature = "db-rocksdb"))]
compile_error!("db cannot be enabled at the same time");

#[cfg(all(feature = "db-leveldb-sys", feature = "db-rusty-leveldb"))]
compile_error!("db cannot be enabled at the same time");

#[cfg(all(feature = "db-leveldb-sys", feature = "db-rocksdb"))]
compile_error!("db cannot be enabled at the same time");

#[cfg(all(feature = "db-rusty-leveldb", feature = "db-rocksdb"))]
compile_error!("db cannot be enabled at the same time");

/*****************************/

fn db_sync_enabled() -> bool {
    static DB_SYNC: OnceLock<bool> = OnceLock::new();
    *DB_SYNC.get_or_init(|| {
        std::env::var("HACASH_DB_SYNC")
            .ok()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}

#[cfg(feature = "db-sled")]
include! {"disk_sled.rs"}

#[cfg(feature = "db-rusty-leveldb")]
include! {"disk_rusty_leveldb.rs"}

#[cfg(feature = "db-leveldb-sys")]
include! {"disk_leveldb_sys.rs"}

#[cfg(feature = "db-rocksdb")]
include! {"disk_rocksdb.rs"}

/*****************************/

include! {"batch.rs"}
