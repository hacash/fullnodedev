#[cfg(test)]
mod tests {
    use crate::*;

    // Define a complex composite structure
    combi_struct! { TestSubStruct,
        age: Uint1
        is_ok: Bool
    }

    combi_list! { TestList, Uint2, Uint2 }

    combi_optional! { TestOptional, name: BytesW1 }

    combi_struct! { TestMainStruct,
        id: Uint4
        addr: Fixed21
        sub: TestSubStruct
        tags: TestList
        opt: TestOptional
    }

    #[test]
    fn test_json_full_cycle() {
        let mut main = TestMainStruct::default();
        
        // 1. Set data
        main.id = Uint4::from(1024);
        // Hacash address example (Base58Check)
        let addr_str = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
        main.addr = Fixed21::from_hex(b"00681990afd226b1cbc6c5f085cfdc2092d0843241").unwrap();
        
        main.sub.age = Uint1::from(25);
        main.sub.is_ok = Bool::new(true);
        
        main.tags.push(Uint2::from(100));
        main.tags.push(Uint2::from(200));
        
        main.opt = TestOptional::must(BytesW1::from(vec![1, 2, 3, 4]).unwrap());

        // 2. Serialize to JSON (default Hex format)
        let json_hex = main.to_json();
        println!("JSON Hex: {}", json_hex);

        // 3. Serialize to JSON (Base58Check format)
        let fmt_58 = JSONFormater { unit: "HAC".to_string(), binary: JSONBinaryFormat::Base58Check };
        let json_58 = main.to_json_fmt(&fmt_58);
        println!("JSON B58: {}", json_58);

        // Verify that JSON 58 contains the address string
        assert!(json_58.contains(addr_str));

        // 4. Deserialize from JSON
        let mut main2 = TestMainStruct::default();
        main2.from_json(&json_58).expect("Parse JSON failed");

        // 5. Deep comparison
        assert_eq!(main.id, main2.id);
        assert_eq!(main.addr, main2.addr);
        assert_eq!(main.sub.age, main2.sub.age);
        assert_eq!(main.sub.is_ok, main2.sub.is_ok);
        assert_eq!(main.tags.length(), main2.tags.length());
        assert_eq!(main.tags[0], main2.tags[0]);
        assert_eq!(main.opt.value().to_vec(), main2.opt.value().to_vec());

        // 6. Test minimal/boundary cases
        let mut main3 = TestMainStruct::default();
        let minimal_json = "{\"id\":777}"; // Only partial fields provided
        main3.from_json(minimal_json).unwrap();
        assert_eq!(*main3.id, 777);
        assert_eq!(*main3.sub.age, 0); // Default value preserved
    }

    #[test]
    fn test_binary_auto_recognition() {
        let mut d = BytesW1::default();
        
        // hex (0x prefix)
        d.from_json(r#""0x010203""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3]);

        // base64 (b64: prefix)
        d.from_json(r#""b64:AQIDBA==""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3, 4]);

        // base58check address (b58: prefix)
        let addr = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
        let mut f21 = Fixed21::default();
        f21.from_json(&format!(r#""b58:{}""#, addr)).unwrap();
        assert_eq!(f21.to_hex(), "00681990afd226b1cbc6c5f085cfdc2092d0843241");

        // plain string (no prefix, no trim)
        d.from_json(r#""hello""#).unwrap();
        assert_eq!(d.to_vec(), b"hello");
        d.from_json(r#""  hello  ""#).unwrap();
        assert_eq!(d.to_vec(), b"  hello  ");

        // hex/b64 with trim
        d.from_json(r#""  0x010203  ""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3]);
        d.from_json(r#""  b64:AQIDBA==  ""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3, 4]);

        // roundtrip: to_json then from_json
        let d2 = BytesW1::from(vec![1, 2, 3, 4, 5]).unwrap();
        let json = d2.to_json();
        let mut d3 = BytesW1::default();
        d3.from_json(&json).unwrap();
        assert_eq!(d2.to_vec(), d3.to_vec());
    }

    combi_struct! { TestStructWithAddress,
        addr: Address
    }

    #[test]
    fn test_address_bare_base58check() {
        let addr_str = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
        let hex_expected = "00681990afd226b1cbc6c5f085cfdc2092d0843241";

        // Address accepts bare base58check (no b58: prefix)
        let mut addr = Address::default();
        addr.from_json(&format!(r#""{}""#, addr_str)).unwrap();
        assert_eq!(addr.to_hex(), hex_expected);

        // Address also accepts b58: prefix (backward compatibility)
        let mut addr2 = Address::default();
        addr2.from_json(&format!(r#""b58:{}""#, addr_str)).unwrap();
        assert_eq!(addr2.to_hex(), hex_expected);

        // Address ToJSON outputs bare base58check when Base58Check format (no prefix)
        let fmt_58 = JSONFormater { unit: "HAC".to_string(), binary: JSONBinaryFormat::Base58Check };
        let json = addr.to_json_fmt(&fmt_58);
        assert_eq!(json, format!(r#""{}""#, addr_str));

        // Roundtrip: struct with Address field
        let mut s = TestStructWithAddress::default();
        s.addr = Address::from_readable(addr_str).unwrap();
        let json = s.to_json_fmt(&fmt_58);
        assert!(json.contains(addr_str));
        assert!(!json.contains("b58:"));
        let mut s2 = TestStructWithAddress::default();
        s2.from_json(&json).unwrap();
        assert_eq!(s.addr.to_hex(), s2.addr.to_hex());

        // Zero address roundtrip
        let zero_addr = Address::UNKNOWN;
        let json_zero = zero_addr.to_json_fmt(&fmt_58);
        let mut zero_parsed = Address::default();
        zero_parsed.from_json(&json_zero).unwrap();
        assert_eq!(zero_addr.to_hex(), zero_parsed.to_hex());
    }

    #[test]
    fn test_bool_recognition() {
        let mut b = Bool::default();
        b.from_json("1").unwrap();
        assert!(b.check());
        b.from_json("0").unwrap();
        assert!(!b.check());
        assert!(b.from_json("\"1\"").is_err());
    }

    #[test]
    fn test_diamond_list_rejects_duplicates() {
        // from_readable rejects duplicate diamond names
        assert!(DiamondNameListMax200::from_readable("WTYUIA,WTYUIA").is_err());
        assert!(DiamondNameListMax200::from_readable("WTYUIA,HYXYHY,WTYUIA").is_err());
        // valid: no duplicates
        assert!(DiamondNameListMax200::from_readable("WTYUIA,HYXYHY").is_ok());
        // from_list_checked rejects duplicates
        let dup = vec![DiamondName::from(*b"WTYUIA"), DiamondName::from(*b"WTYUIA")];
        assert!(DiamondNameListMax200::from_list_checked(dup).is_err());
    }
}
