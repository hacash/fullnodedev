use std::any::*;

use basis::component::*;
use basis::interface::*;
use field::*;
use sys::*;

use super::context::*;
use super::operate::*;
use super::setup::*;
use super::state::*;

include! {"util.rs"}
include! {"macro.rs"}
include! {"create.rs"}

include! {"hacash.rs"}
include! {"satoshi.rs"}
include! {"diamond.rs"}
include! {"asset.rs"}
// include!{"diamond_mint.rs"}
// include!{"diamond_insc.rs"}
// include!{"diamond_util.rs"}
// include!{"channel.rs"}
include! {"blob.rs"}
include! {"chain.rs"}

include! {"astselect.rs"}
include! {"astif.rs"}

/*
* register
*/
action_register! {

    // hac
    HacToTrs      
    HacFromTrs    
    HacFromToTrs    
    // HacAmountCompress  

    DiaSingleTrs       
    DiaFromToTrs   
    DiaToTrs       
    DiaFromTrs     

    // satoshi
    // SatoshiGenesis   
    SatToTrs            
    SatFromTrs         
    SatFromToTrs     

    // asset
    // AssetCreate     
    AssetToTrs         
    AssetFromTrs      
    AssetFromToTrs    

    AstSelect           
    AstIf          

    TxMessage
    TxBlob

    HeightScope       
    ChainAllow     


}
