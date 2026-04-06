#[cfg(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
))]
use std::path::*;

#[cfg(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
))]
use basis::interface::*;

#[cfg(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
))]
use sys::Rerr;

/*****************************/

#[cfg(not(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
)))]
compile_error!("at least one db backend feature must be enabled");

/*****************************/

#[cfg(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
))]
include! {"config.rs"}

#[cfg(feature = "db-sled")]
include! {"disk_sled.rs"}

#[cfg(feature = "db-rusty-leveldb")]
include! {"disk_rusty_leveldb.rs"}

#[cfg(feature = "db-leveldb-sys")]
include! {"disk_leveldb_sys.rs"}

#[cfg(feature = "db-rocksdb")]
include! {"disk_rocksdb.rs"}

/*****************************/

#[cfg(any(
    feature = "db-sled",
    feature = "db-rusty-leveldb",
    feature = "db-leveldb-sys",
    feature = "db-rocksdb"
))]
include! {"batch.rs"}
