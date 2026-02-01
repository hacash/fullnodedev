use vm::lang::{lang_to_irnode_with_sourcemap, Formater, PrintOption, lang_to_irnode};
use vm::IRNode;

#[test]
fn test_fitsh_comprehensive_roundtrip() {
    let fitsh_code = r##"
// 1. Parameters (Must be at top usually)
param { owner amount fee } // Parameter unpacking

// 2. Declarations and Assignments
const MAX_COUNT = 5000
const APP_NAME = "hacash-vm"
const ADMIN_ADDR = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
lib Token = 1 : emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS // Library declaration
bind PI = 314 // Bind macro
var counter = 100 // Var declaration
var total = 200 // Var declaration
let limit = 300 // Let declaration
counter = 101 // Assignment
total += 10 // Compound assignment ADD
total -= 5  // Compound assignment SUB
total *= 2  // Compound assignment MUL
total /= 3  // Compound assignment DIV
let use_const_num = MAX_COUNT
let use_const_str = APP_NAME
let use_const_addr = ADMIN_ADDR

// 3. Direct slot access (Special test case)
$0 = "new owner" // Write to raw slot 0 (owner)
let first_arg = $0 // Read from raw slot 0
$4 = 999 // Write to raw slot 4 (total)
var opt $10 = 123 // Explicit slot assignment and recovery

// 4. Literals
let hex_data = 0xABC123 // Hex bytes
let bin_data = 0b11110000111100001111000011110000 // Binary bytes (32 bits)
let some_int = 123456 // Integer
let target_addr = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS // Address literal
let message = "hello \"world\" \n" // String literal
let joined_msg = message ++ "!!" // String concatenation ++

// 5. Data Structures
let my_list = [1, 2, 3] // Array literal
let packed_list = list { counter; total; PI } // List keyword
let my_map = map { "key": "value", 1: target_addr } // Map literal
let empty_list = [] // Empty list
let empty_map = map {} // Empty map

// 6. Operators (Arithmetic, Logic, Bitwise, Comparison)
let math_res = (counter + total) * (owner - 10) / 2 % 3 // Arithmetic
let logic_res = (counter > 0 || total < 0) && ! (amount == 0) // Logic
let bit_res = (counter << 1) | (total >> 2) ^ (1 & 0xFF) // Bitwise
let cmp_res = counter >= total && total <= owner && counter != amount // Comparison

// 7. Casts and Type Checks (Nested as)
let cast_res = 123 as u8 as u32 as u128 // Nested as
let bytes_val = "data" as bytes // Cast to bytes
let is_nil = counter is nil // Type check nil
let is_list = my_list is list // Type check list
let is_map = my_map is map // Type check map
let is_u64 = counter is u64 // Type check type_id
let is_not_nil = counter is not nil // Type check not nil

// 8. Control Flow
if counter > 100 {
    print "a is big"
    true
} else if counter > 50 {
    log("a is medium", counter)
    true
} else {
    log("a is small", counter)
    true
}

while total > 0 {
    total -= 1
    if total == 50 {
        // test nested block
        let inner_val = 1
    }
}

// 9. Function Calls
let hash = sha3(message) // Native/Built-in call
let succ = Token.transfer(target_addr, 100) // Lib call (dot)
let bal = Token:balance_of(target_addr) // Lib call (colon)
let info = Token::info() // Lib static call (double colon)
let res = self.internal_func(1, 2) // Inner call
call 1::0x01020304(10, 20) // Direct call (index + hash)
callthis 0::0x11223344(30, 40) // Direct inner call
callpure 2::0x55667788(50) // Direct pure call

// 10. Special Instructions
memory_put(0, "data") // Memory put
let mem_val = memory_get(0) // Memory get
assert fee > 0 // Assert
bytecode { POP DUP SWAP } // Raw bytecode
transfer_hac_to(target_addr, 500) // EXTACTION with multiple args (concat)
Token.transfer(target_addr, 100)  // Contract call (PACKLIST)
Token:balance_of(target_addr)     // Contract call (Single arg no PACKLIST)

// 11. System and Data Structure Functions
let storage_val = storage_load("key")
storage_save("key", "value")
storage_del("key")
let s_rest = storage_rest(target_addr)
storage_rent(target_addr, 100)

global_put(1, 200)
let g_val = global_get(1)

heap_grow(1)
heap_write(0, "data")
let h_val = heap_read(0, 4)
heap_write_x(4, 123)
heap_write_xl(0, 8, 456)
let h_u32 = heap_read_uint(4)
let h_u64 = heap_read_uint_long(0, 8)

let list_len = length(my_list)
let map_keys = keys(my_map)
let map_vals = values(my_map)
let has_k = has_key(my_map, "key")
let first = head(my_list)
let rest = tail(my_list)
append(my_list, 4)
insert(my_list, 0, 0)
remove(my_list, 1)
let cloned = clone(my_list)
clear(my_list)

let part = buf_cut("hello", 1, 3)
let l_buf = buf_left(2, "hello")
let r_buf = buf_right(2, "hello")
let ld_buf = buf_left_drop(1, "hello")
let rd_buf = buf_right_drop(1, "hello")
let b_val = byte("hello", 0)
let s_val = size("hello")

let my_addr = context_address()
let h_mei = hac_to_mei(1)
let h_zhu = hac_to_zhu(1)
let m_hac = mei_to_hac(100000000)
let z_hac = zhu_to_hac(1000000000000)
let hash2 = sha2("data")
let hash3 = sha3("data")
let r_hash = ripemd160("data")

let bigger = max(10, 20)
let smaller = min(10, 20)
let inc_val = increase(1, counter)
let dec_val = decrease(1, counter)
let choice = choise(counter > 0, 1, 0)

let h = block_height()
let m_addr = tx_main_address()
let is_ok = check_signature(target_addr)
let b = balance(target_addr)

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
            opt.hide_default_call_argv = true;
            opt.call_short_syntax = true;
            opt.flatten_call_list = true;
            opt.flatten_syscall_cat = true;
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
            println!("Decompiled Code (opt: {}, smap: {}):\n{}", name, use_smap, decompiled);

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
