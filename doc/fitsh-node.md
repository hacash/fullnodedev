# Fitsh 语言节点规则

1. **`let` 只是语法级别的表达式模板**  
   - `let name = expr` 不会立刻占用局部 slot，仅把 `expr` 保存为该符号的模板。`let` 语句本身返回空值，不会生成 `GET`/`PUT`。
   - 每次引用 `name` 时会动态克隆模板并在使用位置展开，等效于“在使用点重新执行 `expr`”。因为没有 slot，`let` 无法直接绑定到 `$N`，只能通过符号名展开。
   - `let` 可以重复绑定同一个符号，只要之前不是 `var`（`var` 定义后不能被 `let` 或其他 `var` 覆写）。这体现了 `let` 与堆栈/slot 彻底无关的宏式语义。

2. **`var` 定义的变量是唯一的、可写的局部 slot**  
   - `var foo = expr` 会在编译时分配 slot 并生成对应的 `PUT` 指令，之后的读取/写入都通过 slot 访问。
   - `var` 不可重复定义，语义上代表一个固定的可写局部变量；如果再出现同名 `var`，会报错。

3. **输出与反编译一致性**
   - `let` 的反编译结果只会展现克隆后的表达式（例如 `print foo` 反编译输出 `print(expr)`），不会恢复 `let` 声明形式，因为 slot 并未生成。`let` 的语义在反编译中只体现为重复展开同样的模板。
   - `var` 编译为 slot 写入，反编译仍能看到 `var`/`$n` 对应的 slot 操作。

4. **状态/全局/内存/日志访问与调用模式**
   - 解释器用两个宏控制不同调用模式对状态的读写：`nsr!()` 会在 `Static` 模式下直接报 `InstDisabled`（禁止读），`nsw!()` 则在 `Static` 和 `Library` 模式下报错（禁止写）。因此，所有纯读指令如 `SLOAD`、`SREST`、`GGET`、`MGET` 都只允许在 `Outer`/`Inner`/`Library` 下执行，静态调用会被拒绝；写入类指令（`SSAVE`、`SDEL`、`SRENT`、`GPUT`、`MPUT`、`LOG1`-`LOG4`）只允许非静态、非库模式。
   - `SLOAD`/`SREST` 只是读取存储的状态（`sread` 中的自动过期清理是系统维护而非用户写入），`SREST` 也用 `nsr!()` 保护，保证静态调用无法获取剩余有效期。库调用仍可以读取（共享父上下文），但即便读取到了过期项也不会因读而写回，只在实际写操作（`SDEL`/`SSAVE`）才触发落盘。
   - `Library` 模式（`calllib`/`CALLLIB`）因此能读取父合约的 storage/global/memory，但不能修改这些状态，也不能写日志；`Static` 模式（`callstatic`/`CALLSTATIC`）不能读也不能写，适合只做计算的 “接口查询” 中继。真正切换 `ctxadr` 的 `CALL` 会以 callee 為 context 地址，所以所有日志/存储操作落在 callee 上；`CALLINR`/`Inner` 模式沿用继承链上下文，可访问父合约 storage。
