# Fitsh Smart Contract Language Manual

A practical guide for developers with smart contract experience (e.g., Solidity) to quickly learn and build contracts in Fitsh.

---

## 1. Design Goals, Characteristics, and Comparison with Solidity

### 1.1 Design Goals

- **Stack-based VM**: Fitsh compiles to a stack-based bytecode executed by the Hacash VM
- **Deterministic execution**: Same inputs produce identical outputs for gas and correctness
- **Gas metering**: Each operation consumes gas; used for on-chain resource accounting
- **IR-first compilation**: Source → IR (Intermediate Representation) → Bytecode; IR is the primary compilation target

### 1.2 Key Characteristics

| Feature | Description |
|---------|-------------|
| **IR decompilation** | IR bytecode can be decompiled back to readable Fitsh source (roundtrip stability) |
| **Function selection** | 4-byte name hash; no overloading; same name = same selector |
| **Library binding** | `lib Name = idx [: address]`; index-based with optional deployment address |
| **Inheritance** | Composition-based via `inherit`; no Solidity-style class inheritance |
| **Abstract hooks** | System payment hooks (`PayableHACD`, `PayableAsset`, etc.) via `abstract` |

### 1.3 Comparison with Solidity

| Aspect | Fitsh | Solidity |
|--------|-------|----------|
| Inheritance | `inherit` (composition) | Class inheritance with `is` |
| Modifiers | None; use `assert`/`if` | `modifier` keyword |
| Integer types | `u8`, `u16`, `u32`, `u64`, `u128` | `uint8`..`uint256`, `int` |
| String/Bytes | `bytes` (quoted strings and hex) | `string`, `bytes` |
| Parameters | `param { a b c }` unpacks to slots | Declared in function signature |
| State mutability | `callview`/`callpure` for read-only | `view`/`pure` modifiers |
| Low-level call | `callcode` (must follow with `end`) | `delegatecall` |
| Payment hooks | `abstract PayableHACD` etc. | `receive()`, `fallback()` |

---

## 2. IR Decompilation Output (Core Feature)

### 2.1 What Is IR Decompilation

Fitsh compiles to IR bytecode. This IR can be **decompiled back to Fitsh source** via `format_ircode_to_lang` or `ircode_to_lang`. The decompiled output is human-readable and, with proper options, can be recompiled to byte-identical bytecode.

### 2.2 Why It Matters

- **Auditing**: Inspect compiled contracts without original source
- **Debugging**: Understand what the VM actually executes
- **On-chain verification**: Verify deployed bytecode against source

### 2.3 Decompilation Options

| Option | Effect |
|--------|--------|
| `trim_param_unpack` | Emit `param { $0 $1 ... }` when param names inferred |
| `hide_default_call_argv` | Omit `nil` or `""` placeholder when no args |
| `call_short_syntax` | Prefer `lib.func(...)` over `call idx::0x...(args)` when SourceMap available |
| `flatten_array_list` | Emit `[a, b, c]` instead of `list { a b c }` |
| `flatten_syscall_cat` | Flatten nested `++` in system call args |
| `recover_literals` | Recover and emit numeric/bytes literals |

### 2.4 Output Forms

- **Parameters**: `param { owner amount fee }` or `param { $0 $1 $2 }` when names unavailable
- **Calls**: `this.foo(...)`, `self.foo(...)`, `super.foo(...)` for internal; `Token.balance_of(addr)` for lib when SourceMap present
- **Raw calls**: `call 1::0xabcdef01(10, 20)` when lib/func name unknown

---

## 3. Contract Structure (`contract` Keyword)

### 3.1 Top-Level Syntax

```fitsh
contract ContractName {

    deploy {
        protocol_cost: "1:248",
        nonce: 1,
        construct_argv: "0xaabb2244"
    }

    library [
        Lib1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS,
        Lib2: bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5
    ]

    inherit [
        BaseToken:   emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS,
        TokenHelper: bJKaNA2dLGxJEwp3xSok8g2buv9Bz65H5
    ]

    abstract PayableHACD(from_addr: address, dianum: u32, diamonds: bytes) {
        return 1
    }

    function public transfer_to(addr: address, amt: u64) -> u32 {
        return this.do_transfer(addr, addr, amt)
    }
}
```

### 3.2 Top-Level Elements

| Element | Purpose |
|--------|---------|
| `deploy { ... }` | Deployment config (protocol_cost, nonce, construct_argv) |
| `library [ ... ]` | `Name: Address` pairs for external contracts |
| `inherit [ ... ]` | `Name: Address` pairs for inheritance chain |
| `abstract Name(...) { ... }` | System payment hooks; return 0 = allow, non-zero = reject |

### 3.3 Function Declaration

```fitsh
function [public|private] [ircode|bytecode] name(param1: type1, param2: type2) -> ret_type { body }
```

- `public`: Externally callable
- `private`: Internal only
- `ircode`: Compile to IR (default for contract functions)
- `bytecode`: Compile to raw bytecode

---

## 4. Keyword List and Descriptions

### 4.1 Declaration and Assignment

| Keyword | Purpose |
|---------|---------|
| `var` | Mutable local variable; allocates slot |
| `let` | Immutable local variable |
| `bind` | Macro binding; no slot; inline expansion |
| `const` | Compile-time constant |
| `param` | Parameter unpacking into slots |
| `lib` | Library binding |

### 4.2 Control Flow

| Keyword | Purpose |
|---------|---------|
| `if` / `else` | Conditional |
| `while` | Loop |
| `return` | Return value |
| `end` | Terminate execution |
| `abort` | Abort |
| `throw` | Throw error |
| `assert` | Assertion |

### 4.3 Debug and Logging

| Keyword | Purpose |
|---------|---------|
| `print` | Debug print |
| `log` | Log event (2..5 args) |

### 4.4 Call Instructions

| Keyword | Purpose |
|---------|---------|
| `call` | Call external contract |
| `callthis` / `callself` / `callsuper` | Internal calls |
| `callview` / `callpure` | Read-only calls |
| `callcode` | CallCode (no return; must follow with `end`) |
| `bytecode` | Raw bytecode injection |

### 4.5 Type and Literal Keywords

| Keyword | Purpose |
|---------|---------|
| `as` | Type cast |
| `is` | Type check |
| `nil` | Nil literal |
| `list` | List literal |
| `map` | Map literal |
| `true` / `false` | Boolean literals |
| `u8` .. `u128` | Integer types |
| `bytes` | Bytes type |
| `address` | Address type |

---

## 5. Syntax and Examples

### 5.1 Literals

```fitsh
123                    // Integer
0xABC123               // Hex bytes
0b11110000             // Binary bytes (8*n bits)
"hello \"world\" \n"   // String (bytes) with escapes
'A'                    // Char literal (byte)
nil                    // Nil
true                   // Boolean
false                  // Boolean
emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS  // Address
```

### 5.2 Arrays and Lists

```fitsh
[1, 2, 3]              // Array literal
[]                     // Empty list
list { 1 2 3 }         // List keyword (space-separated)
```

### 5.3 Maps

```fitsh
map { "key": "value", 1: addr }
map { }                // Empty map
```

### 5.4 Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `**` |
| Bitwise | `<<`, `>>`, `&`, `|`, `^` |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Logic | `&&`, `||`, `!` |
| Concatenation | `++` |

Precedence (high to low): `!` → `**` → `*`/`/`/`%` → `+`/`-` → `<<`/`>>` → `>=`/`<=`/`>`/`<` → `==`/`!=` → `&` → `^` → `|` → `&&` → `||` → `++`

### 5.5 Compound Assignment

```fitsh
x += 1
x -= 1
x *= 2
x /= 2
```

### 5.6 Control Flow

```fitsh
if x > 0 {
    print "positive"
} else if x < 0 {
    print "negative"
} else {
    print "zero"
}

while cnt > 0 {
    cnt -= 1
}
```

### 5.7 Block Expressions

A block `{ stmt; stmt; value }` evaluates statements and returns the last expression:

```fitsh
var result = {
    var inner = 10
    inner + 1
}
// result == 11
```

---

## 6. Special Language Structures

### 6.1 `param { ... }`

Unpacks list arguments into local slots 0, 1, 2, ...

```fitsh
param { owner amount fee }
// owner -> slot 0, amount -> slot 1, fee -> slot 2
```

- Must appear at the top of the function body
- Canonical IR: `UPLIST(PICK0, P0)`

### 6.2 `callcode lib_idx::func_sig`

- No arguments; tail call
- Must be followed by `end`
- Used for low-level delegation

```fitsh
callcode 0::0xabcdef01
end
```

### 6.3 `bytecode { ... }`

Injects raw bytecode opcodes by name or number:

```fitsh
bytecode { POP DUP SWAP }
```

### 6.4 `list { ... }`

Alternative form for lists (space-separated):

```fitsh
list { 1 2 3 }
```

### 6.5 `map { ... }`

Key-value pairs with `:` separator:

```fitsh
map { "k": "v", 1: addr }
```

### 6.6 `log { ... }`

Log event with 2..5 arguments. Supports `()`, `{}`, `[]` delimiters:

```fitsh
log(1, 2)
log[1, 2, 3, 4, 5]
```

---

## 7. Implicit and Explicit Type Conversions

### 7.1 Implicit Conversions

| Context | Allowed | Notable Rejects |
|---------|--------|------------------|
| Arithmetic | Integer widening; Bytes→Uint (1..16 bytes after trim) | Empty bytes; bytes >16; Bool/Address/Nil |
| Byte ops (concat, slice) | Bool/Uint/Address→Bytes | Nil |
| Branching | Truthiness (any value) | — |
| Call args | Uint widening; Bytes↔Address | Bytes→Uint, Bool→anything |

### 7.2 Explicit Casts

```fitsh
x as u8
x as u16
x as u32
x as u64
x as u128
x as bytes
x as address
```

### 7.3 Type Checks

```fitsh
x is nil
x is not nil
x is list
x is map
x is u64
x is bytes
x is address
```

### 7.4 Pitfalls

1. **Arithmetic allows Bytes→Uint, comparisons do not**: `Bytes([0x01]) == U8(1)` fails; use explicit cast.
2. **Bytes↔Uint asymmetry**: Uint→Bytes is fixed-width; Bytes→Uint uses trim + variable width.
3. **Empty bytes**: Cannot participate in arithmetic as zero; normalize to `0 as u64` if needed.

---

## 8. Variables and Local Stack Slots

### 8.1 Variable Types

| Declaration | Slot | Mutability | Evaluation |
|-------------|------|------------|------------|
| `var` | Allocates | Mutable | Immediate |
| `let` | Allocates | Immutable | Immediate |
| `bind` | No slot | — | Lazy (when referenced) |

### 8.2 Slot Addressing

- `$0`, `$1`, ... refer to slots directly
- `var x $5 = 10` assigns to slot 5 explicitly
- `param { a b c }` binds a→0, b→1, c→2

### 8.3 Direct Slot References (`$0`, `$1`, ...)

Identifiers starting with `$` followed by a decimal number (0–255) are **direct slot references**. They bypass the symbol table and bind to the local slot at the given index.

#### Syntax

| Form | Meaning |
|------|---------|
| `$N` | Read slot N (0 ≤ N ≤ 255) |
| `$N = expr` | Write to slot N directly |
| `var name $N = expr` | Bind name to slot N and assign |

#### Relationship with `param`

`param { a b c }` unpacks arguments into slots 0, 1, 2. After that:

- `$0` is the same as the first param
- `$1` is the same as the second param
- `$2` is the same as the third param

You can read or write via `$N` without using the param name.

#### Read and write

```fitsh
param { owner amount }
$0 = "new owner"       // write to slot 0 (overwrites owner)
let first = $0         // read from slot 0
$4 = 999               // write to slot 4 (if allocated)
```

#### Explicit slot in `var` / `let`

To bind a slot to a name:

```fitsh
var opt $10 = 123      // bind "opt" to slot 10, assign 123
let first_arg = $0     // read slot 0 into a new binding
```

#### Slot conflicts

- A slot can only be bound once by `var` or `let` with explicit `$N`
- Avoid mixing `$N` writes with other bindings to the same slot
- Manual slots are reserved via `reserve_slot`; duplicate use causes `slot N already bound`

#### Use cases

| Scenario | Example |
|----------|---------|
| Overwrite param | `$0 = "new owner"` |
| Low-level slot access | `$N` for tight control |
| Interop with `unpack_list` | `unpack_list(self.total(), 2)` then `$2`, `$3` |
| Debug / inspection | Inspect slots by index |

#### CAUTION

- Direct writes bypass usual checks (e.g. immutability of `let`).
- `$N` can overwrite params or other locals; use with care.
- Slots must be allocated (e.g. `param` or `var`/`let` with explicit `$N` or earlier auto-allocation) before use.

### 8.4 Example

```fitsh
param { owner amount }
var total = 200        // auto-allocated slot
var opt $10 = 123      // explicit slot 10
$0 = "new owner"       // write to slot 0 (owner)
let first = $0         // read from slot 0
```

---

## 9. Contract Resource Spaces

Contracts can use six resource spaces. Understanding their scope, lifetime, and use cases helps you choose the right one.

### 9.1 Overview

| Resource | Scope | Lifetime | Key/Index | Max size | Fitsh API |
|----------|-------|----------|-----------|----------|-----------|
| **Locals** | Per function call | Call duration | Slot index (0–255) | 256 slots | `var`, `let`, `param`, `$N` |
| **Heap** | Per function call | Call duration | Byte offset | 64 segments × 256 B | `heap_grow`, `heap_write`, `heap_read` |
| **Memory** | Per contract | Transaction | Key (bytes) | ~16 keys | `memory_put`, `memory_get` |
| **Global** | Transaction-wide | Transaction | Key (bytes) | ~20 keys | `global_put`, `global_get` |
| **Storage** | Per contract | Persistent (rent) | Key (bytes) | Rent-based | `storage_load`, `storage_save`, `storage_del`, `storage_rest`, `storage_rent` |
| **Log** | Per contract | Persistent (on chain) | — | 2–5 args per event | `log(...)` |

### 9.2 Locals (Local Stack Slots)

**Scope**: Current function call (frame).  
**Lifetime**: Until the function returns; then reclaimed.

**Use when**:
- Local variables (`var`, `let`, `param`)
- Intermediate values
- Function parameters and return-value staging

**Characteristics**:
- Indexed by slot (0–255)
- Fast; no key hashing
- Automatically reclaimed on return

**Example**: `param { a b }` → a in slot 0, b in slot 1; `var x = 1` → auto-allocated slot.

### 9.3 Heap

**Scope**: Current function call (frame).  
**Lifetime**: Until the function returns.

**Use when**:
- Large buffers or binary data in a single call
- Offset-based access (like C arrays)
- Passing raw bytes to callees via `HeapSlice`

**Characteristics**:
- Byte array; grow by segments (256 bytes each)
- `heap_grow(n)` allocates; `heap_write(offset, data)` / `heap_read(offset, len)` access
- `heap_read_uint`, `heap_write_x` for fixed-width integers
- Max 64 segments (~16 KB)

**Example**: Parsing or building binary structures within one call.

### 9.4 Memory (Contract Temp)

**Scope**: Per contract (ctxadr).  
**Lifetime**: Current transaction only; cleared after tx.

**Use when**:
- Passing data between different calls to the same contract in one tx
- Multi-step flows (e.g. deposit → swap → withdraw)
- Cross-call state within a tx

**Characteristics**:
- Key-value (key is bytes)
- Each contract has its own memory
- Not persisted; tx-scoped

**Example** (AMM): `prepare` saves `in_sat`, `in_zhu`; `PayableSAT` / `PayableHAC` read them to complete the flow.

```fitsh
// Step 1: prepare
memory_put("in_sat", sat)
memory_put("in_zhu", zhu)

// Step 2: PayableHAC (later in same tx)
var in_zhu = memory_get("in_zhu")
memory_put("in_sat", nil)  // clear after use
```

### 9.5 Global (Transaction-Wide Temp)

**Scope**: Entire transaction.  
**Lifetime**: Current transaction only.

**Use when**:
- Sharing data across different contracts in one tx
- Transaction-level flags or counters
- Cross-contract coordination

**Characteristics**:
- Single key-value map for the whole tx
- All contracts see the same map
- Not persisted

**Example**: Shared tx-id or step counter used by multiple contracts.

```fitsh
global_put("tx_step", 1)
// ... later, in another contract
var step = global_get("tx_step")
```

### 9.6 Storage (Contract State)

**Scope**: Per contract (ctxadr).  
**Lifetime**: Persistent; requires rent; survives blocks.

**Use when**:
- Persistent state (balances, config, totals)
- Data that must survive across tx and blocks

**Characteristics**:
- Key-value; key is bytes (e.g. `"b_" ++ addr`)
- Rent-based; `storage_rent(key, amount)` to pay
- `storage_rest(key)` for expiry
- Value types: Nil, Bool, Uint, Address, Bytes
- Max value size ~1280 bytes

**Example**: Token balances, AMM reserves, config.

```fitsh
bind bk = "b_" ++ addr
var balance = storage_load(bk)
if balance is nil {
    balance = 0 as u64
}
storage_save(bk, balance + amount)
```

### 9.7 Log (Events)

**Scope**: Per contract (ctxadr).  
**Lifetime**: Persistent (on-chain events).

**Use when**:
- Emitting events for indexers or UIs
- Audit trail of actions

**Characteristics**:
- 2–5 arguments per log
- Stored on chain; not queryable from contract

**Example**: `log("Transfer", from, to, amount)`

### 9.8 Decision Guide

| Need | Resource |
|------|----------|
| Local variable / parameter | Locals |
| Large binary buffer in one call | Heap |
| Pass data between calls to same contract in one tx | Memory |
| Share data across contracts in one tx | Global |
| Persistent contract state | Storage |
| Emit events | Log |

### 9.9 Summary

- **Locals / Heap**: Call-scoped; for in-call computation.
- **Memory**: Contract-scoped, tx only; for multi-step flows in one tx.
- **Global**: Tx-scoped; for cross-contract coordination.
- **Storage**: Contract-scoped, persistent; for long-term state.
- **Log**: Contract-scoped, persistent; for events.

---

## 10. `bind` Macro Binding

### 10.1 Behavior

- **Lazy evaluation**: Expression is not evaluated at declaration; evaluated when referenced
- **No slot**: Does not allocate; no `PUT`/`GET`
- **Inline expansion**: Each reference clones the expression template

### 10.2 Use Case

```fitsh
bind bk = "b_" ++ addr
var balance = storage_load(bk)
storage_save(bk, balance + 100)
```

### 10.3 Caution

Side effects (e.g. `storage_save`, `print`) in a `bind` expression only run when the binding is **read**. If never read, the effect never happens. Use `var` for immediate execution.

---

## 11. Other Distinctive Features

### 11.1 Function Call Syntax

| Syntax | Opcode | Use |
|--------|--------|-----|
| `lib.func(...)` | CALL | State-changing call |
| `lib:func(...)` | CALLVIEW | View call |
| `lib::func(...)` | CALLPURE | Pure call |
| `this.func(...)` | CALLTHIS | Current contract |
| `self.func(...)` | CALLSELF | Current contract |
| `super.func(...)` | CALLSUPER | Parent in inherit chain |

### 11.2 Call Permission and State Access Control

The VM enforces a permission system based on **ExecMode** (execution mode) and **in_callcode**. Each call type transitions to a specific mode and constrains what the callee can do.

#### Call Type → Callee Mode

| Call | Syntax | Callee mode | State read | State write |
|------|--------|-------------|------------|-------------|
| `call` | `lib.func(...)` | Outer | Yes | Yes |
| `callview` | `lib:func(...)` | View | Yes | No |
| `callpure` | `lib::func(...)` | Pure | No | No |
| `callcode` | `callcode lib::sig` | Inherits caller | Inherits | Inherits |
| `callthis` | `this.func(...)` | Inner | Yes | Yes |
| `callself` | `self.func(...)` | Inner | Yes | Yes |
| `callsuper` | `super.func(...)` | Inner | Yes | Yes |

**State** = Storage, Global, Memory, Log. 

**Important**: `callcode` runs in the **current frame** and **fully inherits the caller's ExecMode** — it has **no independent state access control logic**. All state operations (storage read/write, EXTACTION/EXTENV/EXTVIEW, NTFUNC/NTENV) in the callcode body are governed by the inherited mode's permissions. Additionally, `callcode` sets `in_callcode = true`, which forbids any further nested calls (CallInCallcode error).

#### Allowed Calls per Entry/Execution Mode

| Mode | Allowed calls | Disallowed |
|------|----------------|------------|
| **Main** (tx main entry) | CALL, CALLVIEW, CALLPURE, CALLCODE | CALLTHIS, CALLSELF, CALLSUPER |
| **P2sh** (script verify) | CALLVIEW, CALLPURE, CALLCODE | CALL, CALLTHIS, CALLSELF, CALLSUPER |
| **Abst** (payment hooks) | CALLTHIS, CALLSELF, CALLSUPER, CALLVIEW, CALLPURE, CALLCODE | CALL (Outer) |
| **Outer** (nested contract) | All | — |
| **Inner** (this/self/super) | All | — |
| **View** (read-only) | CALLVIEW, CALLPURE | CALL, CALLTHIS, CALLSELF, CALLSUPER |
| **Pure** (no state) | CALLPURE only | All others |
| **in_callcode** (inside CALLCODE) | None | All (nested calls forbidden) |

**Abst** disallows CALL (Outer) to prevent reentrancy from payment hooks into external contracts.

| ExecMode/Entry | CALL | CALLVIEW | CALLPURE | CALLCODE | CALLTHIS | CALLSELF | CALLSUPER |
|---|---|---|---|---|---|---|---|
| Main | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| P2sh | ❌ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Abst | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Outer/Inner | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| View | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Pure | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Callcode | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

#### State Access Control Matrix by Mode

**Storage/Global/Memory/Log Access**:

| Mode | Storage read | Storage write | Global/Memory read | Global/Memory write | Log |
|------|--------------|---------------|-------------------|---------------------|
| Main, P2sh, Abst | Yes | Yes | Yes | Yes | Yes |
| Outer, Inner | Yes | Yes | Yes | Yes | Yes |
| View | Yes | No | Yes | No | No |
| Pure | No | No | No | No | No |

**Extended Calls (EXTACTION / EXTENV / EXTVIEW)**:

| Mode | EXTACTION | EXTENV | EXTVIEW | Notes |
|------|-----------|--------|---------|-------|
| **Main** (depth==0, not in_callcode) | ✅ Yes | ✅ Yes | ✅ Yes | Full access |
| **Main** (depth>0 or in_callcode) | ❌ No | ✅ Yes | ✅ Yes | EXTACTION blocked in nested calls / callcode |
| **P2sh, Abst** | ❌ No | ✅ Yes | ✅ Yes | EXTACTION entry-only |
| **Outer, Inner** | ❌ No | ✅ Yes | ✅ Yes | EXTACTION entry-only |
| **View** | ❌ No | ✅ Yes | ✅ Yes | Read-only environment access |
| **Pure** | ❌ No | ❌ No | ❌ No | No external state access |

**Native Functions (NTFUNC / NTENV)**:

| Opcode | Native Call | Args | Pure Mode | View Mode | Main/Outer/Inner | Function |
|--------|-------------|------|-----------|-----------|------------------|----------|
| NTENV | `context_address` | 0 | ❌ Forbidden (`nsr!`) | ✅ Allowed | ✅ Allowed | Read VM execution state |
| NTFUNC | `sha2/sha3/ripemd160` | 1 | ✅ Allowed | ✅ | ✅ | Pure hash functions |
| NTFUNC | `hac_to_mei/zhu`, `mei/zhu_to_hac` | 1 | ✅ Allowed | ✅ | ✅ | Pure amount conversion |
| NTFUNC | `address_ptr` | 1 | ✅ Allowed | ✅ | ✅ | Pure address pointer extraction |

**Summary**:
- **EXTACTION** (asset transfers): Only `Main` mode at `depth == 0` and **not** in `callcode`
- **EXTENV** (`block_height`, `tx_main_address`): Forbidden in `Pure`, allowed elsewhere
- **EXTVIEW** (`check_signature`, `balance`): Forbidden in `Pure`, allowed elsewhere — read-only chain state queries
- **NTFUNC** (pure computation): Always allowed in all modes including `Pure`
- **NTENV** (`context_address`): Forbidden in `Pure` (reads VM state), allowed elsewhere

#### EXTACTION Restriction

| Condition | EXTACTION allowed |
|-----------|-------------------|
| mode == Main AND depth == 0 AND not in_callcode | Yes |
| mode != Main OR depth > 0 OR in_callcode | No |

`transfer_hac_to`, `transfer_sat_to`, etc. can only run at the top-level main call. They are disabled in `callcode`, in abstract/payment hooks, and in nested calls.

#### Summary

- **call** → Outer: full state access; callee must be `public`
- **callview** → View: read-only; no storage/global/memory/log writes
- **callpure** → Pure: no state access; only pure computation and nested CALLPURE
- **callcode** → inherits mode; no nested calls; EXTACTION disabled
- **callthis/callself/callsuper** → Inner: full state access; internal-only

### 11.3 Function Lookup: `this`, `self`, and `super`

The VM maintains two key addresses during execution:

- **ctxadr** (context address): The storage/log owner — the contract initially called (entry point). Stays unchanged through nested inner calls.
- **curadr** (current address): The code owner — the contract whose code is currently executing. Changes when a resolved call targets a different contract.

| Call | Resolves in | Search order |
|------|-------------|---------------|
| `this.func(...)` | ctxadr | DFS: current contract → inherits (in order) |
| `self.func(...)` | curadr | DFS: current contract → inherits (in order) |
| `super.func(...)` | curadr's parents only | DFS: skip curadr, search direct inherits → their inherits |

**When do they differ?** When `super` or `self` moves execution into a parent's code: `curadr` becomes the parent, but `ctxadr` stays the child. Then `this` still resolves in the child (storage context), while `self` resolves in the parent (current code owner).

#### Example 1: Direct call (no inheritance)

```fitsh
contract Token {
    function public balance_of(addr: address) -> u64 {
        bind bk = "b_" ++ addr
        var balance = storage_load(bk)
        if balance is nil {
            balance = 0 as u64
        }
        return balance
    }
    function public transfer_to(addr: address, amt: u64) -> u32 {
        return this.do_transfer(addr, addr, amt)
    }
    function do_transfer(from: address, to: address, amt: u64) -> u32 {
        // ...
    }
}
```

Here `this`, `self`, and `super` all resolve in the same contract. `this.do_transfer(...)` and `self.do_transfer(...)` behave identically.

#### Example 2: Inheritance — `inherit` chain

```fitsh
contract Base {
    function get_value() -> u64 { return 3 }
}
contract Parent {
    inherit [Base: 0x...]
    function get_value() -> u64 { return 2 }
    function compute() -> u64 {
        return this.get_value() * 10000 + self.get_value() * 100 + super.get_value()
    }
}
contract Child {
    inherit [Parent: 0x...]
    function get_value() -> u64 { return 1 }
    function public run() -> u32 {
        let v = super.compute()
        assert v == 10203
        return 0
    }
}
```

- `Child.run()` calls `super.compute()` → resolves in Parent (skip Child). We execute Parent's `compute()`.
- **ctxadr** = Child (unchanged)
- **curadr** = Parent (current code owner)

Inside Parent's `compute()`:

- `this.get_value()` → resolves in **ctxadr** (Child) → Child's `get_value()` → **1**
- `self.get_value()` → resolves in **curadr** (Parent) → Parent's `get_value()` → **2**
- `super.get_value()` → skip Parent, search Parent's inherits → Base's `get_value()` → **3**

Result: `1*10000 + 2*100 + 3 = 10203`

#### Example 3: Inheritance order (first match wins)

```fitsh
contract A { function f() -> u64 { return 10 } }
contract B { function f() -> u64 { return 20 } }
contract C {
    inherit [A: 0x..., B: 0x...]
    function public run() -> u64 {
        return self.f()
    }
}
```

`self.f()` searches C → A → B. A defines `f` first, so the result is **10**. Inherit order defines priority.

#### Example 4: `super` skips current contract

```fitsh
contract Base {
    function helper() -> u64 { return 100 }
}
contract Child {
    inherit [Base: 0x...]
    function helper() -> u64 { return 1 }
    function public run() -> u64 {
        return super.helper()
    }
}
```

`super.helper()` skips Child and searches only in Base. Result: **100** (Base's implementation).

#### Example 5: When to use each

| Use case | Prefer |
|----------|--------|
| Call own or inherited function, resolved from storage context | `this` |
| Call from current code owner (e.g. after `super` into parent) | `self` |
| Call parent's implementation, bypassing override | `super` |

**Summary**: `this` = storage context; `self` = current code owner; `super` = parent chain only.

### 11.4 Native Calls

| Function | Description |
|----------|-------------|
| `context_address()` | Current execution context address |
| `block_height()` | Current block height |
| `sha2(data)` | SHA-256 hash |
| `sha3(data)` | SHA3 hash |
| `ripemd160(data)` | RIPEMD-160 hash |
| `hac_to_mei(n)` | HAC to mei conversion |
| `hac_to_zhu(n)` | HAC to zhu conversion |
| `mei_to_hac(n)` | Mei to HAC |
| `zhu_to_hac(n)` | Zhu to HAC |

### 11.5 Extension Actions (EXTACTION)

| Function | Description |
|----------|-------------|
| `transfer_hac_to(addr, amount)` | Transfer HAC |
| `transfer_hac_from(addr, amount)` | Transfer HAC from |
| `transfer_hac_from_to(from, to, amount)` | Transfer HAC between addresses |
| `transfer_sat_to`, `transfer_sat_from`, `transfer_sat_from_to` | SAT transfers |
| `transfer_hacd_single_to`, `transfer_hacd_to`, etc. | HACD transfers |
| `transfer_asset_to`, `transfer_asset_from`, `transfer_asset_from_to` | Asset transfers |

**Note**: EXTACTION is disabled in `callcode` context.

### 11.6 Storage Functions

| Function | Description |
|----------|-------------|
| `storage_load(key)` | Load value |
| `storage_save(key, value)` | Save value |
| `storage_del(key)` | Delete key |
| `storage_rest(key)` | Get rent expiry |
| `storage_rent(key, amount)` | Pay rent |

### 11.7 Memory and Heap

| Function | Description |
|----------|-------------|
| `memory_put(key, value)` | Put into memory |
| `memory_get(key)` | Get from memory |
| `global_put(key, value)` | Global storage |
| `global_get(key)` | Global get |
| `heap_grow(n)` | Grow heap |
| `heap_write(offset, data)` | Write to heap |
| `heap_read(offset, len)` | Read from heap |

### 11.8 Data Structure Functions

| Function | Description |
|----------|-------------|
| `length(list)` | List length |
| `keys(map)` | Map keys |
| `values(map)` | Map values |
| `haskey(map, key)` | Check key |
| `head(list)` | First element |
| `back(list)` | Last element |
| `append(list, item)` | Append |
| `insert(list, index, item)` | Insert |
| `remove(list, index)` | Remove |
| `clone(val)` | Clone |
| `clear(collection)` | Clear |

### 11.9 Buffer Functions

| Function | Description |
|----------|-------------|
| `buf_cut(buf, start, len)` | Slice |
| `buf_left(n, buf)` | Left n bytes |
| `buf_right(n, buf)` | Right n bytes |
| `buf_left_drop(n, buf)` | Drop left n |
| `buf_right_drop(n, buf)` | Drop right n |
| `byte(buf, index)` | Byte at index |
| `size(buf)` | Size |

### 11.10 Other Ext Functions

| Function | Description |
|----------|-------------|
| `check_signature(addr)` | Verify signature |
| `balance(addr)` | Balance bytes |

### 11.11 Notable: Optional Trailing Comma/Semicolon

Fitsh allows **omitting commas or semicolons at the end of statements and expressions**. This is a distinctive syntax feature compared to C-like languages.

- **Statements**: `var x = 1` and `var x = 1;` are equivalent
- **Expression sequences**: In `list { 1 2 3 }`, elements are space-separated; trailing commas are unnecessary
- **Top-level elements**: Commas between `library`, `inherit` array items are optional

Example:

```fitsh
var a = 1
var b = 2
list { 1 2 3 }
map { "k": "v" }
```

Developers coming from Solidity or similar languages should note: Fitsh does not require statement-ending semicolons.

---

## Quick Reference

- **Function args limit**: 15 (pack list); wrap in `list`/`map` for more
- **Function signature**: 4-byte hash of name only; no overloading
- **`param`**: Must be at top of body
- **`callcode`**: Must be followed by `end`
- **`bind`**: Lazy; use `var` for side effects
