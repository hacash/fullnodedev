// channel status
pub const CHANNEL_STATUS_OPENING                  : Uint1 = Uint1::from(0); // Normal opening
pub const CHANNEL_STATUS_CHALLENGING              : Uint1 = Uint1::from(1); // Challenging period
pub const CHANNEL_STATUS_AGREEMENT_CLOSED         : Uint1 = Uint1::from(2); // After negotiation is closed, reuse can be enabled again
pub const CHANNEL_STATUS_FINAL_ARBITRATION_CLOSED : Uint1 = Uint1::from(3); // Final arbitration closed, never reusable

// Interest attribution of 1% annualized: 0 Press end to assign 1 All to left 2 Give it all right
pub const CHANNEL_INTEREST_ATTRIBUTION_TYPE_DEFAULT          : Uint1 = Uint1::from(0); // default 
pub const CHANNEL_INTEREST_ATTRIBUTION_TYPE_ALL_TO_LEFT      : Uint1 = Uint1::from(1); // give all to left 
pub const CHANNEL_INTEREST_ATTRIBUTION_TYPE_ALL_TO_RIGHT     : Uint1 = Uint1::from(2); // give all to right  


// ChallengePeriodData
combi_struct!{ ChallengePeriodData, 
	// Status = 1 challenge period save data
	is_have_challenge_log             : Bool             // Record challenge data log
	challenge_launch_height           : BlockHeight      // Block height at the beginning of the challenge
	assert_bill_auto_number           : Uint8            // Statement serial number provided by the proposer
	assert_address_is_left_or_right   : Bool             // Whether the proposer is the left address or the right true left false right
	assert_bill                       : HacSat           // The amount claimed by the proponent
}

combi_optional!{ ChallengePeriodDataOptional, 
	challenge : ChallengePeriodData 
}



/******************************* */



// FinalDistributionData
combi_struct!{ ClosedDistributionData, 
	// Status = 2 or 3 
	left_bill : Balance
	right_bill: Balance
}

combi_optional!{ ClosedDistributionDataOptional, closed_distribution : ClosedDistributionData }




/*
* ChannelSto
*/
#[inline]
fn check_channel_status(status: Uint1, if_challenging: bool, if_distribution: bool) -> Ret<()> {
	match *status {
		0 => {
			if if_challenging || if_distribution {
				return errf!("channel opening status cannot carry extra data")
			}
		},
		1 => {
			if !if_challenging || if_distribution {
				return errf!("channel challenging status data mismatch")
			}
		},
		2 | 3 => {
			if if_challenging || !if_distribution {
				return errf!("channel closed status data mismatch")
			}
		},
		_ => return errf!("channel status invalid"),
	}
	Ok(())
}

combi_struct_with_parse!{ ChannelSto, (self, buf, {
	let mut mv = 0;
	let mut seek = buf;
	mv += self.status.parse_from(&mut seek)?;
	mv += self.reuse_version.parse_from(&mut seek)?;
	mv += self.open_height.parse_from(&mut seek)?;
	mv += self.close_height.parse_from(&mut seek)?;
	mv += self.arbitration_lock_block.parse_from(&mut seek)?;
	mv += self.interest_attribution.parse_from(&mut seek)?;
	mv += self.left_bill.parse_from(&mut seek)?;
	mv += self.right_bill.parse_from(&mut seek)?;
	mv += self.if_challenging.parse_from(&mut seek)?;
	mv += self.if_distribution.parse_from(&mut seek)?;
	check_channel_status(
		self.status,
		self.if_challenging.is_exist(),
		self.if_distribution.is_exist(),
	)?;
	Ok(mv)
}), 

	status                        : Uint1           // Closed and settled
	reuse_version                 : Uint4           // Reuse version number from 1

	open_height                   : BlockHeight     // Block height when channel is opened
	close_height                  : BlockHeight     // Block height when channel is closed
	arbitration_lock_block        : Uint2           // Number of blocks to be locked for unilateral end channel
	interest_attribution          : Uint1           // Interest attribution of 1% annualized: 0 Press end to assign 1 All to left 2 Give it all right
	
    left_bill                     : AddrBalance     
    right_bill                    : AddrBalance     

    // status = 1
    if_challenging                : ChallengePeriodDataOptional 

    // status = 2 or 3
    if_distribution               : ClosedDistributionDataOptional 

}
