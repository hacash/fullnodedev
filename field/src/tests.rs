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
        
        // hex
        d.from_json(r#""0x010203""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3]);

        // base64
        d.from_json(r#""AQIDBA==""#).unwrap();
        assert_eq!(d.to_vec(), vec![1, 2, 3, 4]);

        // base58check address
        let addr = "1AVRuFXNFi3rdMrPH4hdqSgFrEBnWisWaS";
        let mut f21 = Fixed21::default();
        f21.from_json(&format!(r#""{}""#, addr)).unwrap();
        assert_eq!(f21.to_hex(), "00681990afd226b1cbc6c5f085cfdc2092d0843241");
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
}
