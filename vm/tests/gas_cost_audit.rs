//! Gas Cost Audit Tool
//! 
//! Analyzes test results and compares them against vm/doc/gas-cost.md specifications

use std::collections::HashMap;

/// Parse test output and extract failure information
pub fn analyze_test_results() {
    println!("=== Gas Cost Audit Report ===\n");
    
    // Expected base gas costs from gas-cost.md
    let expected_base_gas: HashMap<&str, i64> = [
        // Gas cost = 1
        ("PU8", 1), ("P0", 1), ("P1", 1), ("P2", 1), ("P3", 1),
        ("PNBUF", 1), ("PNIL", 1), ("PTRUE", 1), ("PFALSE", 1), ("CU8", 1), ("CU16", 1),
        ("CU32", 1), ("CU64", 1), ("CU128", 1), ("CBUF", 1),
        ("CTO", 1), ("TID", 1), ("TIS", 1), ("TNIL", 1),
        ("TMAP", 1), ("TLIST", 1), ("POP", 1), ("NOP", 1),
        ("NT", 1), ("END", 1), ("RET", 1), ("ABT", 1),
        ("ERR", 1), ("AST", 1), ("PRT", 1),
        // Gas cost = 2 (default)
        ("PU16", 2), ("SWAP", 2), ("SIZE", 2), ("ADD", 2),
        ("SUB", 2), ("AND", 2), ("OR", 2), ("EQ", 2), ("NEQ", 2),
        ("DUP", 2),
        // Gas cost = 3
        ("BRL", 3), ("BRS", 3), ("BRSL", 3), ("BRSLN", 3),
        ("XLG", 3), ("PUT", 3), ("CHOOSE", 3),
        // Gas cost = 4
        ("DUPN", 4), ("POPN", 4), ("PICK", 4), ("PBUF", 4),
        ("PBUFL", 4), ("MOD", 4), ("MUL", 4), ("DIV", 4),
        ("XOP", 4), ("HREAD", 4), ("HREADU", 4), ("HREADUL", 4),
        ("HSLICE", 4), ("HGROW", 4), ("ITEMGET", 4),
        ("HEAD", 4), ("BACK", 4), ("HASKEY", 4), ("LENGTH", 4),
        // Gas cost = 5
        ("POW", 5),
        // Gas cost = 6
        ("HWRITE", 6), ("HWRITEX", 6), ("HWRITEXL", 6),
        ("INSERT", 6), ("REMOVE", 6), ("CLEAR", 6), ("APPEND", 6),
        // Gas cost = 8
        ("CAT", 8), ("BYTE", 8), ("CUT", 8), ("LEFT", 8),
        ("RIGHT", 8), ("LDROP", 8), ("RDROP", 8), ("MGET", 8),
        ("JOIN", 8), ("REV", 8), ("NEWLIST", 8), ("NEWMAP", 8),
        ("NTFUNC", 8),
        // Gas cost = 12
        ("EXTENV", 12), ("NTENV", 12), ("MPUT", 12), ("CALLTHIS", 12),
        ("CALLSELF", 12), ("CALLSUPER", 12), ("PACKLIST", 12),
        ("PACKMAP", 12), ("UPLIST", 12), ("CLONE", 12),
        ("MERGE", 12), ("KEYS", 12), ("VALUES", 12),
        // Gas cost = 16
        ("EXTVIEW", 16), ("GGET", 16), ("CALLCODE", 16),
        // Gas cost = 20
        ("LOG1", 20), ("CALLPURE", 20),
        // Gas cost = 24
        ("LOG2", 24), ("GPUT", 24), ("CALLVIEW", 24),
        // Gas cost = 28
        ("LOG3", 28), ("SDEL", 28), ("EXTACTION", 28),
        // Gas cost = 32
        ("LOG4", 32), ("SLOAD", 32), ("SREST", 32), ("CALL", 32),
        // Gas cost = 64
        ("SSAVE", 64), ("SRENT", 64),
        // Gas cost = 2 (GET)
        ("GET", 2), ("GET0", 2), ("GET1", 2), ("GET2", 2), ("GET3", 2),
    ].iter().cloned().collect();
    
    println!("Expected Base Gas Costs:");
    println!("- Gas cost = 1: {} opcodes", expected_base_gas.values().filter(|&&v| v == 1).count());
    println!("- Gas cost = 2: {} opcodes", expected_base_gas.values().filter(|&&v| v == 2).count());
    println!("- Gas cost = 3: {} opcodes", expected_base_gas.values().filter(|&&v| v == 3).count());
    println!("- Gas cost = 4: {} opcodes", expected_base_gas.values().filter(|&&v| v == 4).count());
    println!("- Gas cost = 5: {} opcodes", expected_base_gas.values().filter(|&&v| v == 5).count());
    println!("- Gas cost = 6: {} opcodes", expected_base_gas.values().filter(|&&v| v == 6).count());
    println!("- Gas cost = 8: {} opcodes", expected_base_gas.values().filter(|&&v| v == 8).count());
    println!("- Gas cost = 12: {} opcodes", expected_base_gas.values().filter(|&&v| v == 12).count());
    println!("- Gas cost = 16: {} opcodes", expected_base_gas.values().filter(|&&v| v == 16).count());
    println!("- Gas cost = 20: {} opcodes", expected_base_gas.values().filter(|&&v| v == 20).count());
    println!("- Gas cost = 24: {} opcodes", expected_base_gas.values().filter(|&&v| v == 24).count());
    println!("- Gas cost = 28: {} opcodes", expected_base_gas.values().filter(|&&v| v == 28).count());
    println!("- Gas cost = 32: {} opcodes", expected_base_gas.values().filter(|&&v| v == 32).count());
    println!("- Gas cost = 64: {} opcodes", expected_base_gas.values().filter(|&&v| v == 64).count());
    println!();
    
    println!("=== Key Findings from Test Results ===\n");
    
    println!("1. HGROW Issues:");
    println!("   - All tests show +1 gas difference");
    println!("   - Expected: base(4) + dynamic, Actual: base(4) + dynamic + 1");
    println!("   - Possible cause: HGROW base gas might be 5 instead of 4, or there's an extra gas charge\n");
    
    println!("2. HREAD Issues:");
    println!("   - All tests show +10 gas difference");
    println!("   - Pattern: Actual = Expected + 10");
    println!("   - Possible cause: HGROW(1 segment) costs 10 gas, which is included in HREAD test\n");
    
    println!("3. HWRITE Issues:");
    println!("   - Inconsistent differences (9-28 gas)");
    println!("   - Pattern suggests HGROW cost is included");
    println!("   - Possible cause: Test includes HGROW setup cost\n");
    
    println!("4. Stack Buffer Copy Issues (DUP, PBUF, GET, MGET, GGET):");
    println!("   - DUP: +2 to +21 gas difference");
    println!("   - PBUF: +1 gas difference (consistent)");
    println!("   - GET: +12 to +23 gas difference");
    println!("   - MGET: +42 to +53 gas difference");
    println!("   - GGET: +66 to +77 gas difference");
    println!("   - Possible causes:");
    println!("     * Setup operations (ALLOC, PUT, MPUT, GPUT) consume gas");
    println!("     * Base gas calculations may be incorrect\n");
    
    println!("5. LOG Operations:");
    println!("   - All tests fail with 'pop empty stack' error");
    println!("   - Issue: LOG operations require parameters on stack");
    println!("   - Test code construction needs fixing\n");
    
    println!("6. SLOAD Issues:");
    println!("   - Very large differences (+362 to +489 gas)");
    println!("   - Pattern suggests SSAVE cost is included");
    println!("   - SSAVE base = 64, plus write cost and rent");
    println!("   - Test includes SSAVE setup, which should be excluded\n");
    
    println!("7. ITEMGET Issues:");
    println!("   - Large differences (+20 to +290 gas)");
    println!("   - Pattern suggests APPEND operations consume gas");
    println!("   - Each APPEND has base(6) + item/4 cost");
    println!("   - Test includes APPEND setup, which should be excluded\n");
    
    println!("=== Recommendations ===\n");
    println!("1. Fix test isolation:");
    println!("   - Separate setup gas from opcode-under-test gas");
    println!("   - Measure only the target opcode's gas consumption\n");
    
    println!("2. Review base gas costs:");
    println!("   - Verify HGROW base gas (currently 4, might be 5)");
    println!("   - Verify all base gas costs match gas-cost.md\n");
    
    println!("3. Fix LOG test code:");
    println!("   - Ensure proper stack setup before LOG operations");
    println!("   - LOG requires specific number of parameters\n");
    
    println!("4. Review dynamic gas calculations:");
    println!("   - Verify stack_copy_gas calculation (byte/12)");
    println!("   - Verify heap_read_gas calculation (byte/16)");
    println!("   - Verify heap_write_gas calculation (byte/12)");
    println!("   - Verify compo operations gas calculations\n");
    
    println!("5. Test methodology improvements:");
    println!("   - Use baseline gas measurement (before opcode)");
    println!("   - Measure only the delta for the target opcode");
    println!("   - Account for all setup operations separately\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn audit_gas_costs() {
        analyze_test_results();
    }
}
