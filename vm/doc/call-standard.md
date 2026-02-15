# Call Standard

Version: Current runtime baseline  
Audience: VM/runtime engineers, reviewers, and test authors  
Style: Design specification and reference manual (not implementation walkthrough)

## 1. Purpose

This document standardizes the semantics of VM "call-like" instructions and runtime extension/native call entries:

1. `CALL`, `CALLTHIS`, `CALLSELF`, `CALLSUPER`, `CALLVIEW`, `CALLPURE`, `CALLCODE`
2. `NTENV`, `NTFUNC`, `EXTENV`, `EXTACTION`, `EXTVIEW`

It defines:

1. Bytecode-level contracts
2. Mode/permission constraints
3. Parameter and return conventions
4. Frame and stack behavior
5. Runtime invariants for testing and auditing

## 2. Scope and Non-Scope

In scope:

1. Invocation semantics and capability boundaries
2. Call graph and frame transitions
3. Call data contracts and return contracts

Out of scope:

1. Full opcode interpretation internals
2. Gas number tuning and pricing formulas
3. Source-language compilation syntax

## 3. Terminology

1. `ctxadr`: execution context address (state scope owner).
2. `curadr`: current code owner address (resolution base for `self/super` semantics).
3. `FnSign`: 4-byte function signature.
4. `Entry mode`: top-level execution mode (`Main`, `P2sh`, `Abst`).
5. `Internal mode`: in-call execution mode (`Outer`, `Inner`, `View`, `Pure`).
6. `CALLSUPER`: internal parent-scope call opcode.

## 4. Quick Reference: Bytecodes and Stack Contracts

## 4.1 Contract call instructions

| Instruction | Byte | Operand bytes | Logical stack contract | Frame behavior |
|---|---:|---:|---|---|
| `CALL` | `0x11` | `libidx(1) + fnsign(4)` | consume 1 call-argument value, produce 1 return value | create new frame (`Outer`) |
| `CALLTHIS` | `0x12` | `fnsign(4)` | consume 1 argument, produce 1 return | create new frame (`Inner`) |
| `CALLSELF` | `0x13` | `fnsign(4)` | consume 1 argument, produce 1 return | create new frame (`Inner`) |
| `CALLSUPER` | `0x14` | `fnsign(4)` | consume 1 argument, produce 1 return | create new frame (`Inner`) |
| `CALLVIEW` | `0x15` | `libidx(1) + fnsign(4)` | consume 1 argument, produce 1 return | create new frame (`View`) |
| `CALLPURE` | `0x16` | `libidx(1) + fnsign(4)` | consume 1 argument, produce 1 return | create new frame (`Pure`) |
| `CALLCODE` | `0x17` | `libidx(1) + fnsign(4)` | no user argument contract; implementation delegation | **no new frame**, in-place code delegation |

## 4.2 Native and extension call instructions

| Instruction | Byte | Operand bytes | Logical stack contract | Return placement |
|---|---:|---:|---|---|
| `NTFUNC` | `0x09` | `id(1)` | consume 1 call-data value | push 1 native result |
| `NTENV` | `0x08` | `id(1)` | consume 0 | push 1 environment value |
| `EXTACTION` | `0x00` | `id(1)` | consume 1 call-data value | no stack return value |
| `EXTENV` | `0x07` | `id(1)` | consume 0 | push 1 typed result |
| `EXTVIEW` | `0x06` | `id(1)` | consume/replace top call-data value | replace top with 1 typed result |

Notes:

1. "Consume 1 call-data value" means the value must be convertible to extension/native call data bytes.
2. `EXTACTION` may return bytes at transport level, but VM stack contract keeps no return item.

## 5. Call Target and Resolution Semantics

## 5.1 `CALL`

Design intent: external contract call by library index.

Resolution model:

1. Resolve library target by index.
2. Resolve function by `FnSign` on that contract's **local user function table only** (no inheritance search for `libidx` calls).
3. Require `public` visibility for external (`Outer`) calls.

Context transition:

1. `ctxadr` switches to target contract.
2. `curadr` switches to target contract.

## 5.2 `CALLTHIS`

Design intent: internal call against the current execution context contract.

Resolution model:

1. Start from `ctxadr`.
2. Resolve through inheritance chain.

Context transition:

1. `ctxadr` remains unchanged.
2. `curadr` is set to resolved owner (or default `ctxadr`).

## 5.3 `CALLSELF`

Design intent: internal call against current code owner contract.

Resolution model:

1. Start from `curadr`.
2. Resolve through inheritance chain.

Context transition:

1. `ctxadr` remains unchanged.
2. `curadr` is set to resolved owner (or default `curadr`).

## 5.4 `CALLSUPER`

Design intent: parent-scope call.

Resolution model:

1. Start from direct parents of `curadr` (skip self).
2. Resolve through parent inheritance chain.

Context transition:

1. `ctxadr` remains unchanged.
2. `curadr` becomes resolved parent owner.

## 5.5 `CALLVIEW`

Design intent: read-only contract call.

Semantics:

1. Uses library-index style addressing like `CALL`.
2. Enters callee with `View` mode.
3. State write operations are forbidden in this mode.
4. Target resolution still follows `libidx` local-table rule (no inheritance search).

## 5.6 `CALLPURE`

Design intent: pure/no-state contract call.

Semantics:

1. Uses library-index style addressing like `CALL`.
2. Enters callee with `Pure` mode.
3. State reads and writes are both forbidden.
4. Target resolution still follows `libidx` local-table rule (no inheritance search).

## 5.7 `CALLCODE`

Design intent: implementation-level delegation without frame expansion.

Core constraints:

1. Target function must have zero parameters.
2. Delegation runs in-place (same frame), not as a normal sub-call.
3. While in callcode state, further call instructions are forbidden.
4. Return value contract is checked against original caller signature.
5. `libidx` target lookup uses local user-function table only (no inheritance search).

Structural rule:

1. Bytecode verification enforces `CALLCODE` to be terminal or immediately followed by `END`.

## 6. Mode Permission Matrix for Call Instructions

Allowed call instructions by current mode:

| Current mode | Allowed call instructions |
|---|---|
| `Main` | `CALL`, `CALLVIEW`, `CALLPURE`, `CALLCODE` |
| `P2sh` | `CALLVIEW`, `CALLPURE`, `CALLCODE` |
| `Abst` | `CALLTHIS`, `CALLSELF`, `CALLSUPER`, `CALLVIEW`, `CALLPURE`, `CALLCODE` |
| `View` | `CALLVIEW`, `CALLPURE` |
| `Pure` | `CALLPURE` |
| `Outer` / `Inner` | unrestricted by mode table (still subject to other runtime checks) |

Global restriction:

1. If currently executing delegated callcode body, no call instruction is allowed.

## 7. Parameter and Return Conventions

## 7.1 Function signature and addressing

1. `FnSign` is fixed-width 4 bytes.
2. Library-index calls carry `libidx + fnsign`.
3. `this/self/super` calls carry only `fnsign`.

Visibility marker semantics:

1. `public` is an **outer-call visibility marker** for `CALL` (`Outer`) path.
2. It does not model a universal "all external forms" permission boundary by itself.
3. If naming causes confusion, implementations may introduce clearer aliases in future versions.

## 7.2 Call argument convention

For non-CALLCODE contract calls:

1. Caller provides one argument value at stack top.
2. Callee type contract validates and may cast this argument against function parameter definition.

For CALLCODE:

1. No user argument is passed.
2. Delegation ABI is fixed for implementation dispatch, not business-level parameter passing.

## 7.3 Return convention

1. Callee return is type-checked against function output contract.
2. For regular calls, return value is propagated back to caller stack.
3. For CALLCODE, return contract is validated against original caller expectation.

Top-level success contract:

1. `nil` or `0` means success for main/p2sh/abstract top-level results.

## 8. Frame and Stack Usage Semantics

## 8.1 Regular calls (`CALL*` except `CALLCODE`)

1. Dispatcher creates a new frame.
2. Caller argument is transferred into callee frame input.
3. Callee executes under its designated mode.
4. On return, value is bubbled back to parent frame.
5. Tail return collapse may skip intermediate frame materialization on unwind.

## 8.2 Delegated call (`CALLCODE`)

1. Dispatcher does not create a frame.
2. Current frame switches code body and call mode marker.
3. Code owner may switch to delegated target owner.
4. After completion, output validation uses caller contract.

## 8.3 Resource accounting perspective

1. Frame creation and contract loading are runtime-observable costs.
2. Delegated call reduces frame churn but does not relax permission constraints.

## 9. Native Call Semantics (`NTFUNC`, `NTENV`)

## 9.1 `NTFUNC`

Design intent: pure deterministic native compute.

Contract:

1. Input: one call-data value.
2. Output: one value returned to stack.
3. Allowed in pure mode.

## 9.2 `NTENV`

Design intent: read VM/environment context.

Contract:

1. Input: none.
2. Output: one environment value to stack.
3. Forbidden in `Pure` mode.

## 10. Extension Call Semantics (`EXTENV`, `EXTACTION`, `EXTVIEW`)

## 10.1 Shared constraints

All extension calls must pass:

1. Extension ID allowlist validation.
2. Mode-based permission validation.
3. Typed result decoding (when applicable).

## 10.2 `EXTENV`

Design intent: environment query through extension channel.

Contract:

1. Input: none.
2. Output: one typed value pushed to stack.
3. Forbidden in `Pure` mode.

## 10.3 `EXTVIEW`

Design intent: read-only query with caller-provided input body.

Contract:

1. Input: top stack call-data body.
2. Output: typed result replacing top stack item.
3. Forbidden in `Pure` mode.

## 10.4 `EXTACTION`

Design intent: state-mutating extension action dispatch.

Contract:

1. Input: top stack call-data body.
2. Output: no stack return value.
3. Allowed only under main-call semantics.
4. Forbidden in delegated callcode context.

Security note:

1. Extension action execution path performs runtime required-signature checks for private-key addresses.

## 11. Error and Failure Semantics

Failure classes:

1. Invalid call target (not found, visibility violation, index overflow).
2. Mode violation (forbidden instruction under current mode).
3. Type contract violation (input/output mismatch).
4. Extension/native ID violation (allowlist miss).
5. Runtime policy violation (e.g., callcode restrictions).

Propagation model:

1. Call failure aborts current path and bubbles as VM/runtime error.
2. Branch frameworks decide merge vs recover at higher level.

## 12. Runtime Invariants for Testing

Recommended invariants for CI/regression:

1. Mode matrix enforcement is stable.
2. `this/self/super/libidx` resolution semantics are stable.
3. External visibility (`public`) is enforced for outer `CALL` paths.
4. CALLCODE never accepts parameterized targets.
5. No nested call is possible in callcode execution state.
6. `EXTACTION` is unavailable outside main-call semantics.
7. `NTENV` and `EXTENV/EXTVIEW` are unavailable in pure mode.
8. Return contracts are always checked before value propagation.

## 13. Suggested Conformance Test Set

1. Positive matrix tests for all call instructions under allowed modes.
2. Negative matrix tests for all forbidden mode combinations.
3. Target resolution tests for `this/self/super/libidx` with inheritance graphs.
4. CALLCODE behavior tests (in-place dispatch, no nested call, return contract inheritance).
5. Native/extension input-output contract tests (typed decode, stack placement).
6. Extension policy tests (`EXTACTION` main-only, pure-mode bans).
7. Visibility and not-found tests for outer calls.
8. Error propagation tests across nested call graphs.

---

This standard defines behavioral contracts for invocation semantics.  
Implementation may evolve internally, but externally observable behavior should remain compliant with this document unless versioned explicitly.
