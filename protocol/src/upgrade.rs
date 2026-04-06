use sys::*;
use field::*;

// Local development is allowed from genesis to this height.
pub const DEV_OPEN_MAX_HEIGHT: u64 = 65_432;

// Set the real mainnet activation height before rollout.
pub const ONLINE_OPEN_HEIGHT: u64 = 765_432;

// One-time pre-upgrade allowlist.
// In the middle closed interval only legacy tx/action kinds below are allowed.
// Remove this whole file after the activation height has passed and the gate is no longer needed.

#[inline]
fn is_pre_upgrade_allowed_tx_type(tx_type: u8) -> bool {
    matches!(tx_type, 1 | 2)
}

#[inline]
fn is_pre_upgrade_allowed_action(kind: u16) -> bool {
    matches!(
        kind,
        1 | 13 | 14 | // Hac*Trs
        2 | 3 | // Channel*
        4 | // DiamondMint
        5 | 6 | 7 | 8 | // Dia*Trs
        32 | 33 // DiaInscPush / DiaInscClean
    )
}

#[inline]
pub fn is_global_upgrade_open(height: u64) -> bool {
    let dev_open = height <= DEV_OPEN_MAX_HEIGHT;
    let online_open = height >= ONLINE_OPEN_HEIGHT;
    dev_open || online_open
}

#[inline]
pub fn check_gated_tx(height: u64, tx_type: u8) -> Rerr {
    if is_global_upgrade_open(height) || is_pre_upgrade_allowed_tx_type(tx_type) {
        return Ok(());
    }
    errf!(
        "tx type {} not enabled at height {}, allowed when height <= {} or >= {}",
        tx_type,
        height,
        DEV_OPEN_MAX_HEIGHT,
        ONLINE_OPEN_HEIGHT
    )
}

#[inline]
pub fn check_gated_action(height: u64, kind: u16) -> Rerr {
    if is_global_upgrade_open(height) || is_pre_upgrade_allowed_action(kind) {
        return Ok(());
    }
    errf!(
        "action kind {} not enabled at height {}, allowed when height <= {} or >= {}",
        kind,
        height,
        DEV_OPEN_MAX_HEIGHT,
        ONLINE_OPEN_HEIGHT
    )
}

#[inline]
pub fn check_transfer_addr_online_open(height: u64, from: &Address, to: &Address) -> Rerr {
    if is_global_upgrade_open(height) {
        return Ok(());
    }
    if from.is_scriptmh() {
        return errf!(
            "transfer from scriptmh address is not enabled before height {}",
            ONLINE_OPEN_HEIGHT
        );
    }
    if from.is_contract() || to.is_contract() {
        return errf!(
            "contract transfer in/out is not enabled before height {}",
            ONLINE_OPEN_HEIGHT
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_height_is_open_for_gated_tx_and_action() {
        assert!(is_global_upgrade_open(0));
        assert!(check_gated_tx(0, 3).is_ok());
        assert!(check_gated_action(0, 25).is_ok());
    }

    #[test]
    fn middle_height_is_closed_for_gated_tx_and_action() {
        let height = DEV_OPEN_MAX_HEIGHT.saturating_add(1);
        assert!(!is_global_upgrade_open(height));
        assert!(check_gated_tx(height, 3).is_err());
        assert!(check_gated_action(height, 25).is_err());
    }

    #[test]
    fn ungated_kind_always_passes() {
        let height = DEV_OPEN_MAX_HEIGHT.saturating_add(1);
        assert!(check_gated_action(height, 1).is_ok());
        assert!(check_gated_action(height, 2).is_ok());
        assert!(check_gated_action(height, 4).is_ok());
        assert!(check_gated_action(height, 32).is_ok());
        assert!(check_gated_tx(height, 1).is_ok());
        assert!(check_gated_tx(height, 2).is_ok());
    }

    #[test]
    fn representative_non_allowlist_actions_are_gated() {
        let height = DEV_OPEN_MAX_HEIGHT.saturating_add(1);
        for kind in [
            10u16,  // SatToTrs
            17,     // AssetToTrs
            22,     // TexCellAct
            0x0401, // TxMessage
            0x0412, // HeightScope
            25,     // AstSelect
            34,     // DiaInscEdit
            40,     // ContractDeploy
            0x0601, // ViewBalance
            0x0701, // EnvHeight
        ] {
            assert!(check_gated_action(height, kind).is_err(), "kind {}", kind);
        }
    }

    #[test]
    fn tx_type3_is_gated_in_middle_closed_interval() {
        let height = DEV_OPEN_MAX_HEIGHT.saturating_add(1);
        assert!(check_gated_tx(height, 3).is_err());
    }
}
