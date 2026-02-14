//! Doc coverage checks for gas-cost.md.
//!
//! Goal: keep gas-cost.md aligned with current runtime billing behavior and
//! prevent legacy terms/omissions from creeping back.

const DOC: &str = include_str!("../doc/gas-cost.md");

#[test]
fn doc_tracks_current_dynamic_metering_groups() {
    // stack-copy related
    for key in [
        "DUPN",
        "GETX",
        "PBUF",
        "PBUFL",
        "PUT, PUTX, MPUT, GPUT",
        "space_write_div",
        "CAT, JOIN, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP",
        "fixed: REV",
    ] {
        assert!(DOC.contains(key), "missing stack-copy coverage key: {key}");
    }

    // extend/native related
    for key in [
        "NTFUNC",
        "NTENV",
        "EXTVIEW",
        "EXTENV",
        "EXTACTION",
        "host-returned gas (`bgasu`)",
    ] {
        assert!(DOC.contains(key), "missing extend/native coverage key: {key}");
    }

    // storage/space/manual sections
    for key in [
        "fixed: SREST",
        "fixed: SDEL",
        "every contract load",
        "BURN: add immediate `u16` gas",
        "Coverage Notes",
    ] {
        assert!(DOC.contains(key), "missing section key: {key}");
    }
}

#[test]
fn doc_does_not_use_legacy_opcode_names() {
    assert!(!DOC.contains("NTCALL"), "legacy name NTCALL should not appear in doc");
    assert!(!DOC.contains("EXTFUNC"), "legacy name EXTFUNC should not appear in doc");
}
