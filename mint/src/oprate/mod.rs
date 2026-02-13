use concat_idents::concat_idents;

use basis::interface::*;
use field::*;
use protocol::operate::*;
use protocol::state::*;
use protocol::*;
use sys::*;

use super::genesis;

include! {"state.rs"}

include! {"channel.rs"}
