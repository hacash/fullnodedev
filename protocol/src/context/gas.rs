const TX_GAS_BUDGET_CAP: i64 = 8192;

#[inline(always)]
pub(crate) fn apply_extra9_surcharge(extra9: bool, gas: u32) -> u32 {
    // Returned-gas charging in this channel intentionally accounts only for the extra9 delta; plain actions add no returned-gas charge here by design.
    if extra9 { gas.saturating_mul(9) } else { 0 }
}

/// - `table[i] = floor(138 * 1.07^i)`, `i in 0..=255`.
/// - `table[255] = 4,292,817,207` (fits in `u32`).
/// start from 138 but set it 0
/// Runtime decoding keeps `gas_max=0` as a reserved "no VM gas" value.
/// Therefore `decode_gas_budget(0)=0`, while non-zero bytes use the lookup table.
pub const GAS_BUDGET_LOOKUP_1P07_FROM_138: [u32; 256] = [
    0, 147, 157, 169, 180, 193, 207, 221, 237, 253, 271, 290, 310, 332, 355, 380, 407, 435, 466,
    499, 534, 571, 611, 654, 699, 748, 801, 857, 917, 981, 1050, 1124, 1202, 1286, 1376, 1473,
    1576, 1686, 1804, 1931, 2066, 2211, 2365, 2531, 2708, 2898, 3101, 3318, 3550, 3799, 4065, 4349,
    4654, 4979, 5328, 5701, 6100, 6527, 6984, 7473, 7996, 8556, 9155, 9796, 10481, 11215, 12000,
    12840, 13739, 14701, 15730, 16831, 18009, 19270, 20619, 22062, 23607, 25259, 27027, 28919,
    30944, 33110, 35428, 37908, 40561, 43401, 46439, 49689, 53168, 56889, 60872, 65133, 69692,
    74571, 79791, 85376, 91352, 97747, 104589, 111911, 119744, 128126, 137095, 146692, 156961,
    167948, 179704, 192284, 205743, 220146, 235556, 252045, 269688, 288566, 308766, 330379, 353506,
    378251, 404729, 433060, 463374, 495811, 530517, 567654, 607389, 649907, 695400, 744078, 796164,
    851895, 911528, 975335, 1043608, 1116661, 1194827, 1278465, 1367958, 1463715, 1566175, 1675807,
    1793114, 1918632, 2052936, 2196642, 2350407, 2514935, 2690980, 2879349, 3080904, 3296567,
    3527327, 3774240, 4038436, 4321127, 4623606, 4947258, 5293566, 5664116, 6060604, 6484847,
    6938786, 7424501, 7944216, 8500311, 9095333, 9732006, 10413247, 11142174, 11922126, 12756675,
    13649643, 14605118, 15627476, 16721399, 17891897, 19144330, 20484433, 21918343, 23452627,
    25094311, 26850913, 28730477, 30741611, 32893523, 35196070, 37659795, 40295981, 43116699,
    46134868, 49364309, 52819811, 56517198, 60473402, 64706540, 69235998, 74082517, 79268294,
    84817074, 90754270, 97107068, 103904563, 111177883, 118960335, 127287558, 136197687, 145731525,
    155932732, 166848023, 178527385, 191024302, 204396003, 218703723, 234012984, 250393893,
    267921466, 286675968, 306743286, 328215316, 351190388, 375773715, 402077876, 430223327,
    460338960, 492562687, 527042075, 563935020, 603410472, 645649205, 690844649, 739203775,
    790948039, 846314402, 905556410, 968945359, 1036771534, 1109345541, 1186999729, 1270089710,
    1358995990, 1454125710, 1555914509, 1664828525, 1781366522, 1906062178, 2039486531, 2182250588,
    2335008129, 2498458698, 2673350807, 2860485364, 3060719339, 3274969693, 3504217571, 3749512802,
    4011978698, 4292817207,
];

#[inline(always)]
pub fn decode_gas_budget(b: u8) -> i64 {
    GAS_BUDGET_LOOKUP_1P07_FROM_138[b as usize] as i64
}

pub fn tx_gas_params_from_byte(gas_max_byte: u8) -> Ret<(i64, i64)> {
    let decoded = decode_gas_budget(gas_max_byte);
    let budget = decoded.min(TX_GAS_BUDGET_CAP);
    if budget <= 0 {
        return errf!(
            "gas budget invalid after clamp: gas_max_byte={} decoded={} chain_cap={}",
            gas_max_byte,
            decoded,
            TX_GAS_BUDGET_CAP
        );
    }
    Ok((budget, 1))
}

#[derive(Clone)]
struct GasSession {
    remaining: i64,
    initial_budget: i64,
    purity_fee: i128,
    purity_size: i128,
    gas_rate: i64,
}

#[derive(Clone)]
struct SettledGas {
    remaining: i64,
    max_charge: Amount,
    used_charge: Amount,
}

#[derive(Clone, Default)]
enum GasCounter {
    #[default]
    Inactive,
    Active(GasSession),
    Settled(SettledGas),
}

impl Default for GasSession {
    fn default() -> Self {
        Self {
            remaining: 0,
            initial_budget: 0,
            purity_fee: 0,
            purity_size: 0,
            gas_rate: 1,
        }
    }
}

impl GasSession {
    fn calc_burn_amount(
        cost: i64,
        purity_fee: i128,
        purity_size: i128,
        gas_rate: i64,
    ) -> Ret<Amount> {
        if cost <= 0 {
            return errf!("gas cost invalid: {}", cost);
        }
        let gas_rate = gas_rate.max(1) as i128;
        let num = (cost as i128)
            .checked_mul(purity_fee)
            .ok_or_else(|| format!("gas burn overflow: cost={} purity_fee={}", cost, purity_fee))?;
        let den = purity_size.checked_mul(gas_rate).ok_or_else(|| {
            format!(
                "gas rate overflow: purity_size={} rate={}",
                purity_size, gas_rate
            )
        })?;
        if den <= 0 {
            return errf!(
                "gas settle denominator invalid: purity_size={} rate={}",
                purity_size,
                gas_rate
            );
        }
        let burn = (num + den - 1) / den;
        if burn <= 0 {
            return errf!(
                "gas burn underflow: cost={} purity_fee={} purity_size={} rate={}",
                cost,
                purity_fee,
                purity_size,
                gas_rate
            );
        }
        if burn > u64::MAX as i128 {
            return errf!("gas burn overflow: {}", burn);
        }
        Ok(Amount::unit238(burn as u64))
    }

    fn burn_amount(&self, cost: i64) -> Ret<Amount> {
        Self::calc_burn_amount(cost, self.purity_fee, self.purity_size, self.gas_rate)
    }

    fn max_charge(&self) -> Ret<Amount> {
        self.burn_amount(self.initial_budget)
    }

    fn used_charge(&self) -> Ret<Amount> {
        let used = self.initial_budget.saturating_sub(self.remaining);
        if used <= 0 {
            return Ok(Amount::zero());
        }
        self.burn_amount(used)
    }
}

impl GasCounter {
    fn reset(&mut self) {
        *self = Self::Inactive;
    }

    fn remaining(&self) -> i64 {
        match self {
            Self::Inactive => 0,
            Self::Active(gas) => gas.remaining,
            Self::Settled(gas) => gas.remaining,
        }
    }

    fn max_charge(&self) -> Ret<Amount> {
        match self {
            Self::Inactive => errf!("gas not initialized"),
            Self::Active(gas) => gas.max_charge(),
            Self::Settled(gas) => Ok(gas.max_charge.clone()),
        }
    }

    fn used_charge(&self) -> Ret<Amount> {
        match self {
            Self::Inactive => errf!("gas not initialized"),
            Self::Active(gas) => gas.used_charge(),
            Self::Settled(gas) => Ok(gas.used_charge.clone()),
        }
    }

    fn begin(&mut self, budget: i64, gas_rate: i64, purity_fee: i128, purity_size: i128) -> Rerr {
        match self {
            Self::Inactive => {
                *self = Self::Active(GasSession {
                    remaining: budget,
                    initial_budget: budget,
                    purity_fee,
                    purity_size,
                    gas_rate: gas_rate.max(1),
                });
                Ok(())
            }
            Self::Active(_) => Ok(()),
            Self::Settled(_) => errf!("gas already settled"),
        }
    }

    fn settle(&mut self) -> Ret<(Amount, Amount)> {
        let Self::Active(gas) = self else {
            return match self {
                Self::Inactive => errf!("gas not initialized"),
                Self::Settled(_) => errf!("gas already settled"),
                Self::Active(_) => unreachable!(),
            };
        };
        let max_charge = gas.max_charge()?;
        let used_charge = gas.used_charge()?;
        let remaining = gas.remaining;
        *self = Self::Settled(SettledGas {
            remaining,
            max_charge: max_charge.clone(),
            used_charge: used_charge.clone(),
        });
        Ok((max_charge, used_charge))
    }

    fn charge(&mut self, gas: i64) -> Rerr {
        if gas < 0 {
            return errf!("gas cost invalid: {}", gas);
        }
        match self {
            Self::Inactive => errf!("gas not initialized"),
            Self::Settled(_) => errf!("gas already settled"),
            Self::Active(state) => {
                let Some(next) = state.remaining.checked_sub(gas) else {
                    return errf!("gas has run out");
                };
                if next < 0 {
                    return errf!("gas has run out");
                }
                state.remaining = next;
                Ok(())
            }
        }
    }
}

impl ContextInst<'_> {
    fn gas_init_tx_inner(&mut self, budget: i64, gas_rate: i64) -> Rerr {
        match &self.gas {
            GasCounter::Inactive => {}
            GasCounter::Active(_) => return Ok(()),
            GasCounter::Settled(_) => return errf!("gas already settled"),
        }
        if budget <= 0 {
            return errf!("gas budget invalid: {}", budget);
        }
        let tx = self.tx();
        let purity_fee = tx.gas_price_purity() as i128;
        let purity_size = 1i128;
        if purity_fee <= 0 || purity_size <= 0 {
            return errf!(
                "tx gas price invalid for gas: purity_fee={} purity_size={}",
                purity_fee,
                purity_size
            );
        }
        let max_burn_amt = GasSession::calc_burn_amount(budget, purity_fee, purity_size, gas_rate)?;
        let main = self.env().tx.main;
        crate::operate::hac_check(self, &main, &max_burn_amt)?;
        crate::operate::hac_sub(self, &main, &max_burn_amt)?;
        self.gas.begin(budget, gas_rate, purity_fee, purity_size)
    }

    fn gas_refund_inner(&mut self) -> Rerr {
        let (max_charge, used_charge) = self.gas.settle()?;
        let refund = max_charge.sub_mode_u128(&used_charge)?;
        if refund.is_positive() {
            let main = self.env().tx.main;
            crate::operate::hac_add(self, &main, &refund)?;
        }
        if used_charge.is_positive() {
            let used_238 = used_charge.to_238_u64()?;
            if used_238 > 0 {
                let mut state = crate::state::CoreState::wrap(self.state());
                let mut ttcount = state.get_total_count();
                let next_burn = (*ttcount.ast_vm_gas_burn_238)
                    .checked_add(used_238)
                    .ok_or_else(|| "ast_vm_gas_burn_238 overflow".to_string())?;
                ttcount.ast_vm_gas_burn_238 = Uint8::from(next_burn);
                state.set_total_count(&ttcount);
            }
        }
        Ok(())
    }

    fn gas_remaining_inner(&self) -> i64 {
        self.gas.remaining()
    }

    fn gas_charge_inner(&mut self, gas: i64) -> Rerr {
        self.gas.charge(gas)
    }
}
