# VM Non-Bug Specification

本文档列出的条目均不是 BUG，而是业务设计。

1. `BUG 1 | M | vm/src/interpreter/execute.rs:253-281`
   `extcall` 宏中 base instruction gas 在 `match` 前扣减且不做 `check_gas`，属于既定执行语义；外部状态可见性由交易级原子回滚边界定义。

2. `BUG 2 | L | vm/src/frame/call.rs:80-100`
   `CALLCODE` 复用当前帧且不清空 operand stack / locals / heap，属于既定委托执行模型，允许被调代码访问调用者帧内局部上下文。

3. `BUG 3 | L | vm/src/frame/call.rs:80-82`
   `CALLCODE` 路径直接使用 `plan.code_owner()` 且不额外 `assert KeepState`，属于调用计划构建阶段已保证的不变量约束。

4. `BUG 4 | L | vm/src/frame/frame.rs:121 + vm/src/frame/call.rs:96`
   `ret_check_policy` 的默认赋值后再由 `CALLCODE` 分支覆写，属于允许的冗余赋值路径，不构成行为错误。

5. `BUG 5 | L | vm/src/interpreter/execute.rs:256-258`
   `CALLCODE` 上下文仅禁止 `EXTACTION`，放行 `EXTENV` 与 `EXTVIEW` 属于既定外部交互分层策略。

6. `BUG 1（本轮审计） | H | vm/src/interpreter/execute.rs:18-45`
   `itrbuf/itrparam` 在 release 关闭逐指令边界检查并使用 `unsafe` 读取，属于性能取舍；执行入口要求字节码先通过 `rt::verify_bytecodes` 的结构和参数边界校验。

7. `BUG 2（本轮审计） | H | vm/src/interpreter/execute.rs:88-116, 241-247`
   `jump/ostjump` 与取指路径在执行期不重复做完整上界判断，属于性能取舍；跳转目标合法性依赖前置 `verify_jump_dests` 一次性校验，所有字节码均需先完成该跳转边界检查。
