use field::Address;
use vm::PrintOption;
use vm::lang::{Formater, lang_to_irnode_with_sourcemap};
use vm::rt::*;

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
        this.notify(total)
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

    let mut opt = PrintOption::new("    ", 0);
    opt.map = Some(&source_map);
    opt.call_short_syntax = true;
    let printed = Formater::new(&opt).print(&ir_block);
    assert!(printed.contains("Fund.deposit("));
    assert!(printed.contains("Fund::audit("));
    assert!(printed.contains("this.notify("));
    assert!(printed.contains("var total $0 ="));
    assert!(printed.contains("let increment $1 ="));
}

#[test]
fn source_map_json_roundtrip() {
    let mut map = SourceMap::default();
    map.register_lib(2, "Fund".to_string(), None).unwrap();
    let sig = extract_signature("deposit");
    map.register_func(sig, "deposit".to_string()).unwrap();
    map.register_slot(0, "total".to_string(), SlotKind::Var)
        .unwrap();
    map.mark_slot_mutated(0);
    assert!(map.slot_is_var(0));
    assert!(!map.slot_is_let(0));
    let json = map.to_json().unwrap();
    let restored = SourceMap::from_json(&json).unwrap();
    assert_eq!(restored.lib(2).unwrap().name, "Fund");
    assert_eq!(restored.func(&sig).map(|s| s.as_str()), Some("deposit"));
    assert_eq!(restored.slot(0).map(|s| s.as_str()), Some("total"));
    assert!(restored.slot_is_var(0));
    assert!(!restored.slot_is_let(0));
}

#[test]
fn source_map_param_names_roundtrip() {
    let mut map = SourceMap::default();
    let param_names = vec!["addr".to_string(), "sat".to_string()];
    map.register_param_names(param_names.clone()).unwrap();
    let json = map.to_json().unwrap();
    let restored = SourceMap::from_json(&json).unwrap();
    assert_eq!(restored.param_names().unwrap(), &param_names);
}
