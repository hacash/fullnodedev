# Diamond Programmable Inscription Space (English)

Version: 1.0  
Date: 2026-02-16  
Audience: protocol developers, wallet/indexer engineers, QA and audit teams

---

## 1. Document Purpose

This document provides a complete, theory-first specification of the Diamond programmable inscription space and serves as a single baseline for:
1. Protocol implementation and future upgrades.
2. Regression tests and acceptance criteria.
3. Wallet, explorer, and indexer integration for inscription actions and fee quoting.

The focus is on business semantics and economic rules, with minimal code-level details.

---

## 2. Terms and Normative Language

1. `MUST`: mandatory requirement.  
2. `SHOULD`: recommended requirement.  
3. `MAY`: optional requirement.  
4. `A`: diamond `average_bid_burn` (unit: mei).  
5. `protocol_cost`: user-provided protocol fee field; protocol only enforces a lower bound.

---

## 3. Core Objects and Boundaries

1. Each Diamond has an inscription list with a hard cap of 200 entries.  
2. Supported inscription actions:
1. Append
2. Clear
3. Move (single entry)
4. Drop (single entry)
5. Edit (single entry)
3. Unified cooldown is 200 blocks.  
4. Any inscription state mutation refreshes inscription height and starts a new cooldown period.

---

## 4. Action Rule Matrix

| Action | Business Meaning | Signature Requirements | Ownership / Status Gate | Cooldown | Capacity | Minimum Protocol Cost |
|---|---|---|---|---|---|---|
| Append | Add inscriptions to one or multiple diamonds | `tx.main` must sign | Diamond must belong to `tx.main`; owner must be PRIVAKEY; status must be Normal | Per target diamond | Before append `<200` | Sum of per-diamond Append tier cost |
| Clear | Remove all inscriptions from one or multiple diamonds | `tx.main` must sign | Same as Append | Per target diamond | No growth | Per diamond `A`, summed |
| Move | Move one inscription from source to target | Source owner and target owner must both sign | Both owners must be PRIVAKEY; both diamonds must be Normal; same-diamond move forbidden; owners need not be `tx.main` | Both source and target | Target before append `<200` | Target-side Append tier only |
| Drop | Delete one inscription by index | `tx.main` must sign | Diamond must belong to `tx.main`; owner PRIVAKEY; status Normal | Target diamond | N/A | `A/50` |
| Edit | Edit one inscription by index | `tx.main` must sign | Same as Drop | Target diamond | N/A | `A/100` |

Notes:
1. Move protocol burn is paid by tx main address.  
2. In Move, signer set and fee payer can differ, but both signature and fee constraints must be satisfied.  
3. Drop/Edit/Move are HIP-22 enhanced actions and depend on network activation.

### 4.1 ActLv Constraints and Composability

Runtime context levels:
1. Top-level tx action context: `ctx = 0`
2. AST context: `ctx = 1..AST_TREE_DEPTH_MAX` (current max depth is 6)
3. VM main-call context: `ctx = 100`
4. VM contract-call context: `ctx = 101`

Current inscription action levels:
1. Append: `ActLv::Top`
2. Clear: `ActLv::Top`
3. Move: `ActLv::Ast`
4. Drop: `ActLv::Top`
5. Edit: `ActLv::MainCall`

Effective allowed contexts under current level checker:
1. Append/Clear/Drop (`Top`): only top-level tx actions, cannot run inside AST or VM-call contexts.
2. Move (`Ast`): allowed in top-level and AST contexts (`ctx <= 99`), but not in VM main/contract call contexts.
3. Edit (`MainCall`): allowed in top-level/AST/VM-main contexts (`ctx <= 100`), but not in VM contract-call context (`ctx = 101`).

Practical implication for AST composition:
1. AST nodes can include Move/Edit directly.
2. AST nodes cannot include Append/Clear/Drop when level checks are active.

### 4.2 AST Snapshot/Recover Audit Result (for Diamond Actions)

Audit scope:
1. `AstSelect` child snapshot and whole-node snapshot semantics.
2. `AstIf` condition/branch snapshot semantics.
3. Diamond inscription action side effects under AST success/failure paths.

Findings:
1. No critical snapshot/recover bug was found for current diamond inscription actions under normal execution mode.
2. `AstSelect` performs:
1. per-child savepoint (`ctx_snapshot`) before each child
2. child rollback (`ctx_recover`) on child failure
3. whole-node rollback on `exe_min` not met
3. `AstIf` performs:
1. savepoint around condition
2. rollback of failed side
3. whole-node rollback if selected branch fails
4. Diamond action writes (diamond state, balances, burn counters) are in state and are covered by context snapshot state forks.
5. VM/logs/ctx volatile fields are restored by `ctx_recover`; gas is intentionally not refunded on rollback.

Validated by regression:
1. failed AST child rollback while keeping later successful child behavior
2. successful child rollback when whole AstSelect finally fails (`exe_min` unmet)

Risk note:
1. In `fast_sync` mode, action level checks and tx signature checks are globally relaxed by design, so AST-level safety assumptions should be evaluated only under normal verification mode.

### 4.3 Reasonableness Assessment of Current ActLv Configuration

Pros:
1. Append/Clear/Drop as `Top` is conservative and reduces dynamic branching complexity for high-impact storage mutations.
2. Move as `Ast` enables programmable routing in AST workflows.
3. Edit as `MainCall` enables VM-main-call programmability for inscription updates.

Concerns:
1. Configuration is asymmetric:
1. Move is AST-capable but not VM-main-call-capable
2. Edit is VM-main-call-capable
3. Append/Clear/Drop are top-only
2. This asymmetry increases mental overhead for integrators and smart strategy designers.

Assessment:
1. Current setup is acceptable if the target is "partially programmable inscriptions with conservative append/drop/clear control".
2. If the target is "fully programmable inscription workflows", level policy should be made more uniform (for example, align Move/Edit and document explicit rationale).

---

## 5. Authorization and Security Model

1. All inscription actions require involved owners to be PRIVAKEY addresses.
2. All inscription actions require diamond status to be Normal.
3. Append/Clear/Drop/Edit require diamond ownership by `tx.main` and valid `tx.main` signature.
4. Move discovers owners from on-chain state and enforces dual signatures (source owner + target owner); neither owner is required to be `tx.main`.
5. Move protocol burn fee is always deducted from `tx.main` balance.

---

## 6. Cooldown Model

1. Unified cooldown window: 200 blocks.  
2. Append/Clear/Drop/Edit: cooldown checked on target diamond(s).  
3. Move: cooldown checked on both source and target.  
4. If cooldown is not met, action fails atomically with no partial commit.

---

## 7. Inscription Content Policy

1. Content cannot be empty.  
2. Maximum content length is 64 bytes.  
3. If `engraved_type <= 100`, content must be readable string.  
4. This validation applies to Append and Edit.

---

## 8. Economic Model

### 8.1 Pricing Formulas

Definitions:
1. `A = average_bid_burn (mei)`
2. `n = current inscription count before append`

Minimum protocol costs:
1. `Append`: `F_append(n, A)`
1. `n < 10` -> `0`
2. `10 <= n < 40` -> `A/50`
3. `40 <= n < 100` -> `A/20`
4. `100 <= n < 200` -> `A/10`
2. `Move`: `F_move = F_append(n_target, A_target)` (target-side only)
3. `Edit`: `F_edit = A/100`
4. `Drop`: `F_drop = A/50`
5. `Clear`: `F_clear = A`

### 8.2 protocol_cost Policy

1. Protocol validates `protocol_cost >= minimum required cost`.  
2. Burned amount equals submitted `protocol_cost` (not auto-clamped to minimum).  
3. This design preserves compatibility for potential future fee reduction.  
4. Tradeoff: accidental overpayment is irreversibly burned.

---

## 9. Quote API Specification

### 9.1 Endpoints

1. Generic: `GET /query/diamond/inscription_protocol_cost`
2. Append: `GET /query/diamond/inscription_protocol_cost/append`
3. Move: `GET /query/diamond/inscription_protocol_cost/move`
4. Edit: `GET /query/diamond/inscription_protocol_cost/edit`
5. Drop: `GET /query/diamond/inscription_protocol_cost/drop`

### 9.2 Parameters

1. Common:
1. `unit` (optional, default `fin`)
2. Append:
1. `name` required, supports up to 200 diamonds, returns sum of per-diamond minimums
3. Move:
1. `to` required
2. `from` optional, if provided then `from != to` is enforced
4. Edit:
1. `name` required, single diamond
5. Drop:
1. `name` required, single diamond

### 9.3 Response Semantics

1. Returned `cost` is protocol minimum only, excluding tx base fee.  
2. Quote path and execution path share the same pricing formulas and should remain consistent.  
3. No dedicated Clear quote endpoint yet (can be added when needed). The generic endpoint also does not accept `action=clear`; passing it returns an error. Clear cost is simply `A` (mei) per diamond and can be computed client-side.

Success example:
```json
{
  "ret": 0,
  "action": "append",
  "cost": "12:248"
}
```

Error example:
```json
{
  "ret": 1,
  "err": "cannot find diamond HYXYHY"
}
```

---

## 10. Operational Notes and Caveats

1. Move is target-priced only, which weakens Drop economics and allows transfer-as-deletion behavior.  
2. Move to low-load targets (`<10`) can be zero protocol cost; product UX should expose this clearly.  
3. Diamonds owned by non-PRIVAKEY addresses cannot execute inscription actions.  
4. Wallet UI should show both minimum and submitted protocol_cost to reduce over-burn mistakes.  
5. Unified 200-block cooldown strongly limits high-frequency edit/cleanup workflows.

---

## 11. Testing Baseline (Normative)

The following cases SHOULD remain in long-lived regression suites:
1. Authorization matrix:
1. `tx.main` signature checks for Append/Clear/Drop/Edit
2. dual signature check for Move
2. Ownership and status:
1. non-owner rejection
2. non-Normal rejection
3. non-PRIVAKEY owner rejection
3. Cooldown matrix:
1. cooldown checked by all five action types
2. dual-side cooldown for Move
4. Capacity edges:
1. 0/9/10/39/40/99/100/199/200
5. Pricing edges:
1. Append tier boundaries
2. Move target-tier boundaries
3. Edit A/100
4. Drop A/50
5. Clear A
6. `protocol_cost` behavior:
1. `<min` fails
2. `==min` succeeds
3. `>min` succeeds and burns submitted value
7. Quote consistency:
1. API minimum quote equals on-chain minimum execution requirement
8. Batch behavior:
1. correct per-diamond sum for Append/Clear
9. AST composition rollback semantics:
1. failed AST child recovers isolated side effects
2. whole AstSelect failure (`exe_min` unmet) rolls back previously successful diamond actions

---

## 12. Change Management

Any change to action rules or economics MUST synchronously update:
1. This document.
2. Action execution rules.
3. Quote API behavior (same-source formulas as execution).
4. Regression test boundaries and assertions.

---

## 13. One-Page Summary

The current Diamond inscription model is:
1. Strong authorization: PRIVAKEY owner + Normal status + required signatures.  
2. Strong cooldown: unified 200 blocks, with dual-side checks for Move.  
3. Tiered incremental pricing: cheaper at low load, more expensive near capacity.  
4. Compatibility burn policy: `protocol_cost >= min` and burn exactly what is submitted.  
5. Unified quote and execution formulas: official quote coverage for Append/Move/Edit/Drop.
