use basis::component::*;
use basis::interface::*;
use basis::method::*;
use field::*;
use protocol::operate::*;
use protocol::setup::*;
use protocol::state::*;
use crate::TransactionCoinbase;
use protocol::*;
use std::any::Any;
use sys::*;

use super::oprate::*;
// use super::genesis::*;

include! {"channel.rs"}
include! {"diamond_util.rs"}
include! {"diamond_mint.rs"}
include! {"diamond_insc.rs"}
include! {"asset.rs"}
include! {"util.rs"}

/*
* actions register
*/
pub fn register(setup: &mut protocol::setup::ProtocolSetup) {
    setup.action_codec(ACTION_CODEC_KINDS, try_create, try_json_decode)
}

action_register! {


    // channel
    ChannelOpen
    ChannelClose
    DiamondMint

    // asset
    AssetCreate

    // inscription
    DiaInscPush
    DiaInscClean

    // HIP-22 inscription upgrade
    DiaInscMove
    DiaInscDrop
    DiaInscEdit

}
