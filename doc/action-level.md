# Action Level Specification Manual

Version: 1.0  
Date: 2026-02-16  
Audience: protocol developers, wallet/gateway developers, audit and QA teams

---

## 1. Document Purpose

This document defines the unified on-chain Action Level rules and is intended to:
1. Serve as a user manual that clarifies in which execution context an action is allowed.
2. Serve as an implementation specification for new action design, review, and regression testing.
3. Serve as an audit baseline to prevent mismatches between permission semantics and actual execution scope.

This document focuses on standards and logic, not implementation-level code details.

---

## 2. Terms and Conventions

1. `MUST`: mandatory requirement.
2. `SHOULD`: recommended requirement; deviations must have explicit justification.
3. `MAY`: optional behavior.
4. `Context Level`: the action execution context level, represented by numeric ranges.
5. `ActLv`: the declared action-level strategy for where an action can execute.

---

## 3. Context Model

### 3.1 Base Levels

1. `TOP`: transaction top-level action context, level = `0`.
2. `AST`: AST condition/select execution context, level = `1..99`.
3. `MAIN_CALL`: VM main-call context, level = `100`.
4. `CONTRACT_CALL`: VM contract-call context, level = `101`.

### 3.2 Depth Constraints

1. AST depth has an upper bound (currently 6).
2. Exceeding the depth limit MUST fail, to prevent runaway complexity from recursive constructs.

### 3.3 Validation Model

The current model is "upper-bound check + special rule checks":
1. Most `ActLv` values are validated by the maximum allowed context level.
2. A few `ActLv` values have extra structural constraints (for example transaction uniqueness, Guard composition constraints, and call-context lower-bound constraints).

This means an `ActLv` name describes a capability upper bound, not an exact single context.

---

## 4. ActLv Semantic Specification

| ActLv | Intended Semantics | Allowed Contexts | Additional Constraints |
|---|---|---|---|
| `TopOnly` | Single unique top-level action | `TOP` only | A transaction may contain only this one action |
| `TopOnlyWithGuard` | Top-level primary action with Guard prefix | `TOP` only | There must be exactly one non-Guard action |
| `TopUnique` | Top-level and unique by kind | `TOP` only | Same kind cannot appear more than once in one transaction |
| `Guard` | Protection/constraint action | `TOP + AST` | Typically used for environment constraints; should not form a standalone transaction |
| `Top` | Standard top-level action | `TOP` only | No additional uniqueness requirement |
| `Ast` | AST-composable action | `TOP + AST` | Not allowed in VM call contexts |
| `MainCall` | Action allowed in VM main call | `TOP + AST + MAIN_CALL` | Not allowed in contract-call context |
| `ContractCall` | Action allowed in contract call | `TOP + AST + MAIN_CALL + CONTRACT_CALL` | No extra structural constraints |
| `AnyInCall` | Call-context-only action | `MAIN_CALL + CONTRACT_CALL` | Must be in call context (`level >= 100`) |
| `Any` | No context restriction | All contexts | SHOULD be used with caution |

---

## 5. Transaction-Level Rules (Linked to Action Level)

1. Transaction action count MUST be within valid bounds (non-zero and not above the max limit).
2. In non-fast-sync mode, action level checks MUST be enforced.
3. Guard actions should not form an "all-Guard transaction"; such transactions MUST be rejected.
4. For `TopOnly`, `TopOnlyWithGuard`, and `TopUnique`, structural constraints MUST be enforced before execution.

---

## 6. AST Composition Semantics (Relation to Levels)

1. AST nodes SHOULD support per-branch snapshots and rollback on failure.
2. If an AST node fails as a whole, state changes from previously successful child branches within that node MUST be rolled back.
3. Gas policy may be decoupled from state rollback (for example failed-branch gas is not refunded), but the policy MUST be explicit and consistent.
4. `ActLv` only controls whether an action may enter a context; it does not replace AST atomicity guarantees.

---

## 7. Design and Implementation Guidance

### 7.1 Level Selection Principles

1. New actions SHOULD choose the least-privilege `ActLv` (the narrowest execution scope).
2. If an action involves asset movement, state growth, or external programmability, conservative levels SHOULD be preferred.
3. Query/system-function-style actions MAY use call-context levels (for example `AnyInCall`).

### 7.2 Exact-Context Requirements

When business logic requires "only one exact context":
1. Do not rely only on the `ActLv` name semantics.
2. Add explicit context assertions (exact match) or introduce finer-grained level semantics.

### 7.3 Composability Consistency

1. Action sets in the same business domain SHOULD keep composability policy consistent, to reduce integration and mental-model complexity.
2. If asymmetry is necessary, risks, boundaries, and rationale MUST be documented.

---

## 8. fast_sync Mode Notes

1. `fast_sync` is performance/recovery-oriented mode, not standard security-validation mode.
2. Under `fast_sync`, some strict checks are intentionally relaxed.
3. Any security, economic, or permission-model conclusions SHOULD be based on normal verification mode.

---

## 9. Integration Recommendations for Clients

1. Wallets/gateways SHOULD pre-check Action Level rules before transaction assembly, to reduce on-chain failure rates.
2. When actions include AST/VM composition, clients SHOULD perform both "context reachability" and "final structural constraint" checks.
3. Error messages SHOULD map to user-understandable semantics, for example:
   - "Top-level only"
   - "Call-context only"
   - "This action must be unique in one transaction"

---

## 10. Change Governance Recommendations

1. Any Action Level rule change MUST update this manual first, then implementation and tests.
2. Each upgrade SHOULD provide:
   - migration impact summary (which actions are affected),
   - regression checklist (expected pass/fail behavior),
   - behavior differences under `fast_sync`.

This manual can be used as a baseline for protocol behavior acceptance and code audits.

