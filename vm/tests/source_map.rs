use field::Address;
use vm::IRNode;
use vm::lang::lang_to_irnode_with_sourcemap;
use vm::rt::calc_func_sign;

fn extract_signature(name: &str) -> [u8; 4] {
    calc_func_sign(name)
}

#[test]
fn source_map_recovery_records_symbols() {
    let script = r##"
        lib Fund = 2 : emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        var total = 100
        var increment $1 = 5
        total = total + increment
        Fund.deposit(total, increment)
        Fund::audit(increment)
        self.notify(total)
    "##;

    let (ir_block, source_map) = lang_to_irnode_with_sourcemap(script).unwrap();
    let addr = Address::from_readable("emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS").unwrap();
    let lib_entry = source_map.lib(2).expect("expected Fund lib entry");
    assert_eq!(lib_entry.name, "Fund");
    assert_eq!(lib_entry.address.as_ref(), Some(&addr));

    assert_eq!(source_map.slot(0).map(|s| s.as_str()), Some("total"));
    assert_eq!(source_map.slot(1).map(|s| s.as_str()), Some("increment"));

    let deposit_sig = extract_signature("deposit");
    let audit_sig = extract_signature("audit");
    let notify_sig = extract_signature("notify");
    assert_eq!(
        source_map.func(&deposit_sig).map(|s| s.as_str()),
        Some("deposit")
    );
    assert_eq!(
        source_map.func(&audit_sig).map(|s| s.as_str()),
        Some("audit")
    );
    assert_eq!(
        source_map.func(&notify_sig).map(|s| s.as_str()),
        Some("notify")
    );

    let printed = ir_block.print("    ", 0, true, Some(&source_map));
    assert!(printed.contains("Fund.deposit("));
    assert!(printed.contains("Fund::audit("));
    assert!(printed.contains("self.notify("));
    assert!(printed.contains("total ="));
    assert!(printed.contains("increment ="));
}
