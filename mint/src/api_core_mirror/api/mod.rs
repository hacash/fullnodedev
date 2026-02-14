use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;

use basis::component::*;
use basis::interface::*;
use field::*;
use sys::*;
// use basis::method::*;
// use basis::config::*;
// use super::*;
use protocol::state::*;
// use protocol::context::*;
use protocol::action::*;
use protocol::block::{self, *};
use protocol::transaction::{self, *};

use super::*;

include! {"action.rs"}

include! {"block.rs"}
include! {"transaction.rs"}

include! {"scan_transfer.rs"}

include! {"create_account.rs"}
include! {"create_transfer.rs"}

include! {"submit_transaction.rs"}
include! {"submit_block.rs"}

include! {"fee.rs"}

include! {"routes.rs"}
