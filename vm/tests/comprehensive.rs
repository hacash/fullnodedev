use vm::lang::{lang_to_irnode_with_sourcemap, Formater, PrintOption, lang_to_irnode};
use vm::IRNode;

#[test]
fn test_fitsh_comprehensive_roundtrip() {
    let fitsh_code = r##"
// 1. Parameters (Must be at top usually)
param { $0 $1 $2 } // Parameter unpacking

// 2. Declarations and Assignments
lib Token = 1 : emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS // Library declaration
bind PI = 314 // Bind macro
var $3 = 100 // Var declaration (mutable slot)
var $4 = 200 // Var declaration (mutable slot)
let $5 = 300 // Let declaration (immutable slot)
$3 = 101 // Assignment
$4 += 10 // Compound assignment ADD
$4 -= 5  // Compound assignment SUB
$4 *= 2  // Compound assignment MUL
$4 /= 3  // Compound assignment DIV

// 3. Parameters direct access
let $6 = $0 // Direct slot access

// 4. Literals
let $7 = 0xABC123 // Hex bytes
let $8 = 0b11110000111100001111000011110000 // Binary bytes (32 bits)
let $9 = 123456 // Integer
let $10 = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS // Address literal
let $11 = "hello \"world\" \n" // String literal

// 5. Data Structures
let $12 = [1, 2, 3] // Array literal
let $13 = list { $3; $4; PI } // List keyword
let $14 = map { "key": "value", 1: $10 } // Map literal
let $141 = [] // Empty list
let $142 = map {} // Empty map

// 6. Operators (Arithmetic, Logic, Bitwise, Comparison)
let $15 = ($3 + $4) * ($0 - 10) / 2 % 3 // Arithmetic
let $16 = ($3 > 0 || $4 < 0) && ! ($1 == 0) // Logic
let $17 = ($3 << 1) | ($4 >> 2) ^ (1 & 0xFF) // Bitwise
let $18 = $3 >= $4 && $4 <= $0 && $3 != $1 // Comparison

// 7. Casts and Type Checks (Nested as)
let $19 = 123 as u8 as u32 as u128 // Nested as
let $20 = "data" as bytes // Cast to bytes
let $21 = $3 is nil // Type check nil
let $22 = $12 is list // Type check list
let $23 = $14 is map // Type check map
let $24 = $3 is u64 // Type check type_id
let $25 = $3 is not nil // Type check not nil

// 8. Control Flow
if $3 > 100 {
    print "a is big"
    true
} else if $3 > 50 {
    log("a is medium", $3)
    true
} else {
    log("a is small", $3)
    true
}

while $4 > 0 {
    $4 -= 1
    if $4 == 50 {
        // test nested block
        let $26 = 1
    }
}

// 9. Function Calls
let $27 = sha3($11) // Native/Built-in call
let $28 = Token.transfer($10, 100) // Lib call (dot)
let $29 = Token:balance_of($10) // Lib call (colon)
let $30 = Token::info() // Lib static call (double colon)
let $31 = self.internal_func(1, 2) // Inner call
call 1::0x01020304(10, 20) // Direct call (index + hash)
callinr 0x11223344(30, 40) // Direct inner call
callstatic 2::0x55667788(50) // Direct static call

// 10. Special Instructions
memory_put(0, "data") // Memory put
let $32 = memory_get(0) // Memory get
assert $2 > 0 // Assert
bytecode { POP DUP SWAP } // Raw bytecode
print "end of test" // Print
return true // Return
"##;

    // 1. Compile to IR and SourceMap
    let (ir1, smap1) = lang_to_irnode_with_sourcemap(fitsh_code).expect("First compilation failed");
    let bin1 = ir1.serialize();

    // 2. Test combinations of PrintOptions
    let options_to_test = vec![
        ("canonical", {
            let mut opt = PrintOption::new("  ", 0);
            opt.trim_root_block = true;
            opt.trim_head_alloc = true;
            opt.trim_param_unpack = true;
            opt.call_short_syntax = true;
            opt.flatten_array_list = true;
            opt.recover_literals = true;
            opt
        }),
    ];

    for (name, mut opt) in options_to_test {
        println!("--- Testing PrintOption: {} ---", name);
        
        // Test with and without sourcemap
        for use_smap in [true, false] {
            println!("  Use SourceMap: {}", use_smap);
            opt.clear_all_slot_puts(); // Fix: Clear state between runs
            if use_smap {
                opt.map = Some(&smap1);
            } else {
                opt.map = None;
            }

            // Decompile
            let decompiled = Formater::new(&opt).print(&ir1);
            // println!("Decompiled Code (opt: {}, smap: {}):\n{}", name, use_smap, decompiled);

            // Recompile
            let ir2 = lang_to_irnode(&decompiled).expect(&format!("Recompilation failed for opt: {}, use_smap: {}:\n{}", name, use_smap, decompiled));
            let bin2 = ir2.serialize();

            // Compare Binary
            if bin1 != bin2 {
                println!("BINARY MISMATCH for opt: {}, use_smap: {}", name, use_smap);
                println!("Original Binary Length: {}", bin1.len());
                println!("Recompiled Binary Length: {}", bin2.len());
                // Find first difference
                for i in 0..bin1.len().min(bin2.len()) {
                    if bin1[i] != bin2[i] {
                        println!("First diff at byte {}: original=0x{:02x}, recompiled=0x{:02x}", i, bin1[i], bin2[i]);
                        break;
                    }
                }
                panic!("Roundtrip failed binary equality test!");
            }
        }
    }
    
    println!("Comprehensive Fitsh Roundtrip Audit Passed!");
}
