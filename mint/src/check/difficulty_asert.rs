const ASERT_UPGRADE_EPOCH: u64 = 2566; // 739008 height

const ASERT_HALF_LIFE: i64 = 10800;
const ASERT_RADIX_BITS: u32 = 16;
const ASERT_RADIX: i64 = 1i64 << ASERT_RADIX_BITS;
const ASERT_POLY_1: u128 = 195766423245049;
const ASERT_POLY_2: u128 = 971821376;
const ASERT_POLY_3: u128 = 5127;
const ASERT_POLY_TERM_SHIFT: usize = 48;
const ASERT_EASING_MAX_SCALE: u8 = 2;

impl DifficultyGnr {
    fn asert_anchor_height(&self) -> u64 {
        self.asert_upgrade_height().saturating_sub(1)
    }

    fn target_asert(
        &self,
        prevdiff: u32,
        _prevblkt: u64,
        hei: u64,
        blkt: u64,
        src: &dyn BlockIntroSource,
    ) -> DifficultyTarget {
        let anchor_hei = self.asert_anchor_height();
        let (anchor_time, anchor_diff, _) = self.req_block_intro(anchor_hei, src);
        let anchor_parent_time = if anchor_hei == 0 {
            anchor_time
        } else {
            self.req_block_intro(anchor_hei - 1, src).0
        };
        let time_delta = blkt as i128 - anchor_parent_time as i128;
        let height_delta = hei as i128 - anchor_hei as i128;
        let exponent = ((time_delta - self.cnf.each_block_target_time as i128 * (height_delta + 1)) * ASERT_RADIX as i128) / ASERT_HALF_LIFE as i128;
        let num_shifts = exponent >> ASERT_RADIX_BITS;
        let frac = (exponent - (num_shifts << ASERT_RADIX_BITS)) as u128;
        let frac2 = frac * frac;
        let frac3 = frac2 * frac;
        let factor = (((ASERT_POLY_1 * frac + ASERT_POLY_2 * frac2 + ASERT_POLY_3 * frac3 + (1u128 << (ASERT_POLY_TERM_SHIFT - 1))) >> ASERT_POLY_TERM_SHIFT) + ASERT_RADIX as u128) as u64;
        let prev_target = u32_to_biguint(prevdiff);
        let ease_target = prev_target * BigUint::from(ASERT_EASING_MAX_SCALE);
        let max_target = u32_to_biguint(LOWEST_DIFFICULTY);
        let mut next_target = u32_to_biguint(anchor_diff) * BigUint::from(factor);
        if num_shifts < 0 {
            next_target >>= (-num_shifts) as usize;
        } else if num_shifts > 0 {
            next_target <<= num_shifts as usize;
        }
        next_target >>= ASERT_RADIX_BITS as usize;
        if next_target == BigUint::default() {
            return DifficultyTarget::from_big(BigUint::from(1u8))
        }
        if next_target > ease_target {
            next_target = ease_target;
        }
        if next_target > max_target {
            return DifficultyTarget::from_num(LOWEST_DIFFICULTY)
        }
        DifficultyTarget::from_big(next_target)
    }
}
