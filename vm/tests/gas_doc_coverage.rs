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
        "stack_write_div",
        "CAT, JOIN, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP",
        "fixed: REV",
        "EQ, NEQ, XLG (`==` / `!=` only)",
        "stack_cmp_div",
        "`Tuple`/`Compo` equality is pointer-identity only",
    ] {
        assert!(DOC.contains(key), "missing stack-copy coverage key: {key}");
    }

    // action/native related; docs may still use pre-rename opcode names until docs are updated.
    for keys in [
        &["NTFUNC"][..],
        &["NTENV"][..],
        &["ACTVIEW", "EXTVIEW"][..],
        &["ACTENV", "EXTENV"][..],
        &["ACTION", "EXTACTION"][..],
        &["host-returned gas (`bgasu`)"][..],
    ] {
        assert!(
            keys.iter().any(|key| DOC.contains(key)),
            "missing action/native coverage key: {:?}",
            keys
        );
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
    assert!(
        !DOC.contains("NTCALL"),
        "legacy name NTCALL should not appear in doc"
    );
    assert!(
        !DOC.contains("EXTFUNC"),
        "legacy name EXTFUNC should not appear in doc"
    );
}
