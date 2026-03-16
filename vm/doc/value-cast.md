# Value Conversion and Normalization Spec

This document is the single source of truth for `Value` conversion, normalization, and comparison behavior.

## 1. Scope

`Value` variants:

- Scalar/value domain: `Nil`, `Bool`, `U8`, `U16`, `U32`, `U64`, `U128`, `Bytes`, `Address`
- Special domain: `HeapSlice`, `Compo`

Unless explicitly stated, all conversion and comparison rules only apply to the scalar/value domain.

## 2. API Orthogonality

- `canbe_*`: convert to Rust/native type, does not mutate `self`
- `to_*`: convert to normalized `Value`, does not mutate `self`
- `cast_*`: explicit cast in place, mutates `self`

This separation is strict and must remain stable.

## 3. Canonical Normalization Primitives

## 3.1 Byte normalization (`extract_bytes_ec`)

Output type: `Vec<u8>`.

- `Bool` -> `[0x01]` or `[0x00]`
- `U8/U16/U32/U64/U128` -> fixed-width big-endian bytes (leading zero bytes preserved)
- `Bytes` -> raw bytes
- `Address` -> raw address bytes
- Other types (`Nil`, `HeapSlice`, `Compo`) -> error

## 3.2 Bool normalization (`extract_bool`)

Output type: `bool`.

- `Bool(b)` -> `b`
- `Nil` -> `false`
- `U*` -> `n != 0`
- `Bytes` -> any non-zero byte => `true`, otherwise `false`
- `Address` -> any non-zero byte => `true`, otherwise `false`
- `HeapSlice`, `Compo` -> error

## 3.3 Minimal Uint normalization (`to_uint`)

Output type: `Value` (`U8/U16/U32/U64/U128`).

- `U*` -> keep current uint value/type
- `Nil` -> `U8(0)`
- `Bool` -> `U8(0|1)`
- `Bytes` / `Address`:
  - drop leading zero bytes
  - map remaining byte length to minimal uint width:
    - `0` -> `U8(0)`
    - `1` -> `U8`
    - `2` -> `U16`
    - `3..=4` -> `U32`
    - `5..=8` -> `U64`
    - `9..=16` -> `U128`
    - `>16` -> error
- `HeapSlice`, `Compo` -> error

## 3.4 Numeric scalar normalization (`to_u128`)

Output type: `u128`.

- `U*` -> value widening to `u128`
- `Nil` -> `0`
- `Bool` -> `0|1`
- `Bytes` / `Address`:
  - drop leading zero bytes
  - remaining length `<=16` => parse as big-endian `u128`
  - remaining length `>16` => error
- `HeapSlice`, `Compo` -> error

## 4. Explicit Cast Rules (`cast_*` / `CU*` / `CTO`)

## 4.1 `cast_bool` / `CTO Bool`

- Uses `extract_bool`
- Same acceptance/failure set as section 3.2

## 4.2 `cast_u8/u16/u32/u64/u128` / `CU8..CU128`

For target width `W` bits (`N = W/8` bytes):

- If source is `Bytes`:
  - if `len <= N`: left-pad zeros to `N` and parse
  - if `len > N`: only allowed when truncated prefix bytes are all zero; otherwise error
- If source is not `Bytes`:
  - normalize by `to_u128`
  - check target range (`u8/u16/u32/u64/u128`)
  - overflow => error

So explicit uint cast and implicit numeric normalization share the same scalar domain (`Nil/Bool/U*/Address` are all normalized by numeric rules), while `Bytes` keeps width-aware cast semantics.

## 4.3 `cast_buf` / `CBUF`

- Uses byte normalization from section 3.1
- `Nil`, `HeapSlice`, `Compo` are rejected

## 4.4 `cast_addr`

- First `cast_buf`
- Then parse exact `Address` byte length
- Length mismatch or invalid address bytes => error

## 4.5 `cast_to` / `CTO <type>`

- Dispatches to `cast_bool`, `cast_u*`, `cast_buf`, `cast_addr`
- `CTO` target type param must be in the explicit-cast set: `Bool/U8/U16/U32/U64/U128/Bytes/Address`
- `CTO` target outside this set (`Nil`, `HeapSlice`, `Compo`) => `InstParamsErr`
- `TIS` type param accepts all declared `ValueTy` (including `Nil/HeapSlice/Compo`) for type checking
- `TIS` unknown or reserved type id => `InstParamsErr`
- `CTO` unknown or reserved type id => `InstParamsErr`

## 5. Implicit Conversion Trigger Map

## 5.1 Bool context (implicit bool normalization)

Uses section 3.2 (`extract_bool`):

- Logic ops: `AND`, `OR`, `NOT`
- Control-flow condition: `CHOOSE`, `BRL`, `BRS`, `BRSL`, `BRSLN`, `AST`
- `CTO Bool`

## 5.2 Numeric arithmetic context (implicit uint normalization)

Uses: `to_uint(x)`, `to_uint(y)`, then width promotion (`cast_arithmetic`) to the wider uint type.

Applied by:

- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `POW`, `MAX`, `MIN`
- Bit ops: `BSHL`, `BSHR`, `BAND`, `BOR`, `BXOR`
- Unary numeric: `INC`, `DEC` (single operand normalized by `to_uint` when needed)

Failure in either operand normalization or width/range checks => immediate error.

## 5.3 Equality context (`EQ`, `NEQ`)

- If both operands are uint types (`U8/U16/U32/U64/U128`):
  - convert both to `u128`
  - compare numeric value
- Otherwise, operands must have the same runtime type; cross-type compare fails.
- Same-type compare rules:
  - `Nil == Nil`
  - `Bool` compares bool value
  - `Bytes` compares raw bytes
  - `Address` compares raw address bytes
  - `HeapSlice` compares `(start, len)`
  - `Compo` compares pointer identity (`ptr_eq`)

## 5.4 Ordered compare context (`LT`, `LE`, `GT`, `GE`)

- Ordered comparison is `uint`-only.
- Both operands must already be uint types: `U8/U16/U32/U64/U128`.
- No implicit `to_u128` conversion is applied for `Nil/Bool/Bytes/Address`.
- Operands are compared as numeric uint values.
- Any non-uint operand => immediate type error.

## 5.5 Byte-handle context

Uses section 3.1 (`extract_bytes_ec`):

- `CAT`, `JOIN`, `BYTE`, `CUT`, `LEFT`, `RIGHT`, `LDROP`, `RDROP`

`Nil`, `HeapSlice`, `Compo` are rejected in this path.

## 5.6 External/native call data context

Uses `extract_call_data`:

- `Nil` => `[]`
- `HeapSlice(start, len)` => read bytes from heap slice
- Otherwise => `extract_bytes_ec`

This is the only allowed conversion path where `HeapSlice` participates.

## 6. Function Param/Return Type Check Rules

## 6.1 Signature-level constraints (`ValueTy`)

- Param type cannot be `Nil`, `HeapSlice`, `Tuple`
- Return type cannot be `Nil`, `HeapSlice`

## 6.2 Runtime checked cast (`check_param_type`)

- If source and target are identical: pass
- Allowed checked casts are uint-family only:
  - `U8/U16/U32/U64/U128 -> U8/U16/U32/U64/U128`
  - narrowing succeeds only when the value fits the target width
- All other source/target combinations: fail

Note: `check_param_type` is the non-mutating wrapper over `cast_param`.

## 7. Deterministic Edge Cases (Normative)

- `U16(1) == U8(1)` is `true` (both uint; compare as `u128`)
- `U32(5) != U8(4)` is `true` (both uint; compare as `u128`)
- `Nil == Bool(false)` fails (cross-type compare is not allowed)
- `Bytes([0,1]) == U8(1)` fails (cross-type compare is not allowed)
- `Bytes([0,1]) == Bytes([1])` is `false` (same-type raw-bytes compare)
- Ordered compare with non-uint values (`Nil`, `Bool`, `Bytes`, `Address`, `HeapSlice`, `Compo`) always fails

## 8. U256 Reservation Contract (Forward Compatibility)

This section defines mandatory constraints before enabling `U256`, to prevent compatibility breaks and normalization drift.

- Type id `7` and keyword `u256` are reserved and currently disabled.
- Any decode/build path receiving type id `7` must fail explicitly (not silently downgraded).
- Any type parser receiving `u256` must fail explicitly with a reserved/not-enabled error.
- Activation must be version-gated (e.g. codeconf/height fork switch), not implicit by parser/runtime fallback.
- `U256` activation is all-or-nothing for conversion paths:
  - update scalar domain (`Value`, `ValueTy`, serialization/parse)
  - update explicit cast family (`cast_u256`, `CU256`, `CTO U256`)
  - update implicit numeric normalization (`to_uint`, arithmetic width promotion, ordered compare)
  - update equality uint numeric-compare path to include `U256` as uint operand
- While `U256` is disabled, numeric scalar normalization remains bounded by current active max width (`u128` / 16 bytes).
- No mixed mode is allowed: enabling parser keyword without runtime normalization support is forbidden.
