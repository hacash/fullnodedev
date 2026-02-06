# Value Casting Rules
===

This document describes **all implicit (automatic)** and **explicit (manual)** type conversion rules used by the Hacash VM value system, focusing on:

- `u8 ~ u128` integer arithmetic and comparisons
- integer ↔ bytes conversions used by arithmetic and byte-manipulation opcodes
- function-call argument casting rules

The goal is to make conversions predictable for contract developers, and to highlight places where the rules may be surprising or inconsistent.


## Value Types (Runtime)

The VM value types relevant to casting are:

- `Nil`
- `Bool`
- Unsigned integers: `U8`, `U16`, `U32`, `U64`, `U128`
- `Bytes`
- `Address`

There are other internal types (e.g. `HeapSlice`, `Compo`) that appear in the runtime, but their casting behavior is mostly “not allowed” (except for truthiness, see below).


## Implicit (Automatic) Conversions

Implicit conversions happen **without any explicit cast opcode**. They are triggered by specific operations.


### 1) Arithmetic Operands: integer widening + Bytes→Uint

Arithmetic opcodes (and some bit opcodes, because they are executed through the same arithmetic wrapper) apply an implicit normalization step before the real operation runs.

**Trigger path**

- `binop_arithmetic` / `locop_arithmetic` → `cast_arithmetic(x, y)` → run the operation

**Rule A — Integer widening**

If both operands are unsigned integers of different widths, the smaller one is **promoted to the larger width** (no range checks needed because it is widening).

Examples:

- `U8 + U16` → `U16 + U16`
- `U32 ^ U64` → `U64 ^ U64`
- `U64 << U8` → `U64 << U64` (shift-count is promoted too)

**Rule B — Bytes→Uint for arithmetic**

If one or both operands are `Bytes`, they are first converted to an unsigned integer via **big-endian interpretation with leading-zero trimming**:

1. Drop left (most-significant) zero bytes.
   - Note: for a non-empty buffer, trimming keeps at least 1 byte (so “all-zero bytes” becomes `U8(0)`).
2. Choose the target integer width based on remaining length:
   - 1 byte  → `U8`
   - 2 bytes → `U16`
   - 3–4     → `U32` (left padded to 4)
   - 5–8     → `U64` (left padded to 8)
   - 9–16    → `U128` (left padded to 16)
3. Then apply **Rule A** (widening) to make both operands the same width.

Examples:

- `Bytes([0x01]) + U8(2)` → `U8(1) + U8(2)`
- `Bytes([0x00, 0x01]) + U8(2)` → `U16(1) + U16(2)`
- `Bytes([0x01,0x02,0x03])` becomes `U32(0x00010203)`

**Failure cases**

- `Bytes` is empty (`Bytes([])`) → arithmetic cast fails.
- `Bytes` length after trimming is `> 16` → arithmetic cast fails.
- Any non-uint, non-bytes types (e.g. `Bool`, `Address`, `Nil`) → arithmetic cast fails.

**Note: some arithmetic ops still have extra restrictions**

Even after casting, the operation itself may reject the operand width. The most notable case is `POW`:

- `POW` only supports `(U8,U8)`, `(U16,U16)`, `(U32,U32)`.
- Therefore `U64 POW U8` will be promoted to `(U64,U64)` and then **fail** because `U64` is unsupported for pow.


### 2) Bytes Consumers: Uint/Bool/Address → Bytes

Many byte operations accept non-`Bytes` values and implicitly view them as bytes.

**Trigger**

Operations that need “byte-like” inputs call a helper that accepts several types and returns a byte buffer.

**Rule**

When a value is consumed as bytes:

- `Bool(true)`  → `[0x01]`, `Bool(false)` → `[0x00]`
- `U8`..`U128`  → **fixed-width big-endian** encoding (`1/2/4/8/16` bytes)
- `Bytes`       → unchanged (clone)
- `Address`     → address raw bytes (fixed length, chain-defined)
- Other types   → error

Examples:

- `CAT` can concatenate `U32(1)` with `Bytes("abc")` by converting `U32(1)` to `00 00 00 01` first.
- `JOIN` can join a list of `U8` and `Bytes` values into one byte buffer.

**Important consequence (non-symmetry)**

This encoding is *not* the inverse of arithmetic `Bytes→Uint`:

- Uint→Bytes uses **fixed width**.
- Bytes→Uint uses **trim + variable width**.

So `U16(1)` → bytes `00 01`, but `Bytes(00 01)` → `U16(1)` (ok), while `Bytes(01)` → `U8(1)`.


### 3) Truthiness (used by branching and some operators)

The VM defines a “truthiness” check that is used by:

- Branching opcodes (e.g. `BR*`)
- Conditional selection (e.g. `CHOISE`)
- Logical operators (`AND`, `OR`, `NOT`)

Rule:

- `Nil` → false
- `Bool(b)` → `b`
- `U*` → `n != 0`
- `Bytes` → true iff **any** byte is non-zero
- Other types (e.g. `Address`, `Compo`) → true

This is **not** a cast; it is a boolean interpretation.


### 4) Comparisons and Equality (logic operands)

The VM’s comparison operators are **not** unified with arithmetic casting. In particular, `Bytes→Uint` does *not* happen automatically for comparisons.

**Equality / inequality (`==`, `!=`)**

Supported pairs:

- `Nil` vs `Nil` → true; `Nil` vs non-`Nil` → false (and vice-versa)
- `Bool` vs `Bool` → compare booleans
- `Address` vs `Address` → compare addresses
- `Bytes` vs `Bytes` → compare raw bytes (byte-for-byte)
- Unsigned integers (`U8..U128`) vs unsigned integers → compare numerically with integer widening

All other pairs are rejected (type mismatch error).

**Ordering (`<`, `<=`, `>`, `>=`)**

- Only unsigned integers (`U8..U128`) are supported.
- Mixed widths are compared by widening to a chosen target width.
- `Bytes`, `Bool`, `Address`, `Nil`, etc. are rejected.


### 5) Call Argument Casting (ABI-level)

When a function expects a specific parameter type, the VM may accept some “nearby” types and cast automatically during argument checking.

Allowed implicit casts in parameter checking:

- Integer widening only:
  - `U8 → U16 → U32 → U64 → U128`
- `Bytes ↔ Address` (mutual)

Everything else is rejected as “argument type mismatch”.

Notably:

- `Bytes → Uint` is **not** allowed at this layer (even though arithmetic allows it).
- `Bool ↔ Uint` is **not** allowed.


## Explicit (Manual) Conversions

Explicit conversions are performed by dedicated cast operations (cast opcodes) that mutate the top stack value.

Note: the `NOT` opcode is implemented as “cast to bool using truthiness, then invert”. So `NOT` always produces a `Bool` even if the input is not a `Bool`.


### A) Cast to Bool

Manual cast-to-bool sets the value to `Bool(check_true(value))`.

This means it accepts essentially any value:

- `Nil` becomes `false`
- Numeric `0` becomes `false`, non-zero becomes `true`
- `Bytes([0,0])` becomes `false`, `Bytes([0,1])` becomes `true`
- `Address` becomes `true` (always)
- `Compo` becomes `true` (always)

This is powerful but easy to misuse: casting an `Address` to bool does **not** check “is non-zero address”.


### B) Cast to Unsigned Integers (`U8..U128`)

Manual integer casts are stricter than arithmetic implicit casts.

**From another integer type**

- Widening is always allowed (`U8 → U128`, etc.).
- Narrowing is allowed only if the value fits the target range (checked).

**From `Bytes`**

Manual integer casts interpret bytes as a big-endian integer, with flexible length handling:

- If `Bytes` is **shorter** than the target width: left-pad with zeros.
- If `Bytes` is **longer** than the target width: drop left zeros first; if it still doesn’t fit, fail.
- Then decode using `from_be_bytes` of the target width.

Examples:

- Cast-to-`U32`:
  - `Bytes([0x01])` → `U32(1)` (pads to `00 00 00 01`)
  - `Bytes([0x00,0x00,0x01,0x02])` → `U32(258)`
  - `Bytes([0x00,0x01,0x02,0x03,0x04])` → may fit after dropping zeros; otherwise fails

**Invalid sources**

- `Nil`, `Bool`, `Address`, `Compo`, `HeapSlice` cannot be cast to integer directly.


### C) Cast to Bytes

Manual cast-to-bytes follows the same encoding described in “Bytes consumers”:

- `Bool` → `[0x00]` or `[0x01]`
- `U8..U128` → fixed-width big-endian
- `Address` → raw address bytes
- `Bytes` → no-op

Invalid sources:

- `Nil`, `Compo`, `HeapSlice` (and others) fail.


### D) Cast to Address

Manual cast-to-address works in two steps:

1. Cast-to-bytes (see above)
2. Parse bytes as an `Address`

In practice, this means only byte buffers of the expected address length will succeed.

Common valid cases:

- `Bytes(addr_bytes)` where `len == Address::SIZE`
- `Address` (roundtrips through bytes)

Common invalid cases:

- Integer types (they become fixed-width 1/2/4/8/16 bytes, which will not match address length)
- `Bytes` with wrong length


## “Valid vs Invalid” Summary Tables


### Implicit conversions summary (by context)

| Context | Allowed implicit conversions | Notable rejects |
|---|---|---|
| Arithmetic (+,-,*,/,%,pow,max,min and bit ops via arithmetic wrapper) | Uint widening; Bytes→Uint (length 1..16 after trim) | Empty bytes; bytes >16; Bool/Address/Nil |
| Byte operations (concat/cat/join, slicing helpers) | Bool/Uint/Address→Bytes | Nil/Compo/HeapSlice |
| Branching / truthiness | Interprets any value as boolean (truthy/falsey) | (no cast; interpretation only) |
| Call argument checking | Uint widening; Bytes↔Address | Bytes→Uint, Bool→anything |


### Explicit cast summary (source → target)

| Target | Valid sources | Common invalid sources |
|---|---|---|
| Bool | Any (uses truthiness) | (none) |
| U8..U128 | Uint types (widen always; narrow if fits), Bytes (big-endian with padding/trim-to-fit) | Nil/Bool/Address/Compo/HeapSlice |
| Bytes | Bool, Uint, Bytes, Address | Nil/Compo/HeapSlice |
| Address | Bytes of correct length; Address | Integers (wrong length), Bytes wrong length |


## Known Pitfalls / “Possibly Unreasonable” Parts

These are not necessarily bugs, but they are areas developers should treat as sharp edges:

1. **Arithmetic allows Bytes→Uint implicitly, but comparisons do not.**
   - `Bytes([0x01]) + U8(1)` may work (after cast)
   - but `Bytes([0x01]) == U8(1)` will fail (type mismatch), unless you explicitly cast.

2. **Bytes↔Uint conversions are not canonical.**
   - Uint→Bytes is fixed-width; Bytes→Uint is trim+variable width.
   - Multiple byte encodings can represent the same numeric value in arithmetic, which can be surprising for hashing, storage keys, or equality checks.

3. **Empty bytes are a special “error value” for arithmetic.**
   - `PNBUF` (empty bytes) exists as a constant, but cannot participate in arithmetic as zero.
   - If developers expect empty bytes to mean numeric zero, they must explicitly normalize it (e.g. to `U8(0)`) in contract logic.

4. **Casting to Bool treats complex types as always-true.**
   - `Address` and `Compo` become `true` when cast to bool, which may not match developer intent.


## Practical Guidance for Contract Developers

- For anything security-sensitive (conditions, auth, invariants), **do not rely on implicit casting**. Use explicit cast opcodes or write explicit normalization logic.
- If you want numeric semantics on bytes, choose a convention:
  - Either always use fixed-width numeric bytes (e.g. always 16 bytes for `U128`)
  - Or always cast bytes to a specific integer width before comparing/operating
- Avoid mixing `Bytes` and `Uint` in comparisons unless you explicitly cast one side to match the other.
