use basis::component::*;
use basis::interface::*;
use basis::method::*;
use field::*;
use protocol::action::*;
use protocol::operate::*;
use protocol::setup::*;
use protocol::state::*;
use protocol::transaction::*;
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
