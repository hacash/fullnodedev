# Fitsh 智能合约语言手册

面向有智能合约开发经验（如 Solidity）的开发者，帮助快速上手并开发 Fitsh 合约的实用指南。

---

## 1. 设计目标、特点及与 Solidity 的对比

### 1.1 设计目标

- **栈式虚拟机**：Fitsh 编译为在 Hacash VM 上执行的栈式字节码
- **确定性执行**：相同输入产生相同的 gas 和正确性输出
- **Gas 计费**：每条操作消耗 gas，用于链上资源核算
- **IR 优先编译**：源码 → IR（中间表示）→ 字节码；IR 是主要编译目标

### 1.2 核心特点

| 特性 | 说明 |
|------|------|
| **IR 反编译** | IR 字节码可反编译回可读的 Fitsh 源码（可往返稳定） |
| **函数选择** | 4 字节名称哈希；无重载；同名即同选择器 |
| **库绑定** | `lib Name = idx [: address]`；按索引，可选部署地址 |
| **继承** | 通过 `inherit` 组合式继承；无 Solidity 风格类继承 |
| **抽象钩子** | 通过 `abstract` 定义系统支付钩子（如 PayableHACD、PayableAsset） |

### 1.3 与 Solidity 对比

| 方面 | Fitsh | Solidity |
|------|-------|----------|
| 继承 | `inherit`（组合） | 使用 `is` 的类继承 |
| 修饰符 | 无；使用 `assert`/`if` | `modifier` 关键字 |
| 整数类型 | `u8`、`u16`、`u32`、`u64`、`u128` | `uint8`..`uint256`、`int` |
| 字符串/字节 | `bytes`（引号字符串和十六进制） | `string`、`bytes` |
| 参数 | `param { a b c }` 解包到槽位 | 在函数签名中声明 |
| 状态可变性 | `callview`/`callpure` 只读 | `view`/`pure` 修饰符 |
| 底层调用 | `callcode`（必须紧跟 `end`） | `delegatecall` |
| 支付钩子 | `abstract PayableHACD` 等 | `receive()`、`fallback()` |

---

## 2. IR 反编译输出（核心特性）

### 2.1 什么是 IR 反编译

Fitsh 编译为 IR 字节码。该 IR 可通过 `format_ircode_to_lang` 或 `ircode_to_lang` **反编译回 Fitsh 源码**。反编译输出可读，在合适选项下可重新编译为字节一致的字节码。

### 2.2 为何重要

- **审计**：在无原始源码时检查已编译合约
- **调试**：理解 VM 实际执行内容
- **链上验证**：验证已部署字节码与源码一致

### 2.3 反编译选项

| 选项 | 效果 |
|------|------|
| `trim_param_unpack` | 推断参数名时输出 `param { $0 $1 ... }` |
| `hide_default_call_argv` | 无参数时省略 `nil` 或 `""` 占位符 |
| `call_short_syntax` | 有 SourceMap 时优先 `lib.func(...)` 而非 `call idx::0x...(args)` |
| `flatten_array_list` | 输出 `[a, b, c]` 而非 `list { a b c }` |
| `flatten_syscall_cat` | 展开系统调用参数中的嵌套 `++` |
| `recover_literals` | 恢复并输出数字/字节字面量 |

### 2.4 输出形式

- **参数**：`param { owner amount fee }` 或名称不可用时 `param { $0 $1 $2 }`
- **调用**：内部调用 `this.foo(...)`、`self.foo(...)`、`super.foo(...)`；有 SourceMap 时库调用 `Token.balance_of(addr)`
- **原始调用**：库/函数名未知时 `call 1::0xabcdef01(10, 20)`

---

## 3. 合约结构（`contract` 关键字）

### 3.1 顶层语法

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

### 3.2 顶层元素

| 元素 | 用途 |
|------|------|
| `deploy { ... }` | 部署配置（protocol_cost、nonce、construct_argv） |
| `library [ ... ]` | 外部合约的 `Name: Address` 对 |
| `inherit [ ... ]` | 继承链的 `Name: Address` 对 |
| `abstract Name(...) { ... }` | 系统支付钩子；返回 0 表示允许，非零表示拒绝 |

### 3.3 函数声明

```fitsh
function [public|private] [ircode|bytecode] name(param1: type1, param2: type2) -> ret_type { body }
```

- `public`：可外部调用
- `private`：仅内部
- `ircode`：编译为 IR（合约函数默认）
- `bytecode`：编译为原始字节码

---

## 4. 关键字列表与说明

### 4.1 声明与赋值

| 关键字 | 用途 |
|--------|------|
| `var` | 可变局部变量；分配槽位 |
| `let` | 不可变局部变量 |
| `bind` | 宏绑定；无槽位；内联展开 |
| `const` | 编译时常量 |
| `param` | 参数解包到槽位 |
| `lib` | 库绑定 |

### 4.2 控制流

| 关键字 | 用途 |
|--------|------|
| `if` / `else` | 条件 |
| `while` | 循环 |
| `return` | 返回值 |
| `end` | 终止执行 |
| `abort` | 中止 |
| `throw` | 抛出错误 |
| `assert` | 断言 |

### 4.3 调试与日志

| 关键字 | 用途 |
|--------|------|
| `print` | 调试输出 |
| `log` | 日志事件（2..5 个参数） |

### 4.4 调用指令

| 关键字 | 用途 |
|--------|------|
| `call` | 调用外部合约 |
| `callthis` / `callself` / `callsuper` | 内部调用 |
| `callview` / `callpure` | 只读调用 |
| `callcode` | CallCode（无返回值；必须紧跟 `end`） |
| `bytecode` | 原始字节码注入 |

### 4.5 类型与字面量关键字

| 关键字 | 用途 |
|--------|------|
| `as` | 类型转换 |
| `is` | 类型检查 |
| `nil` | Nil 字面量 |
| `list` | 列表字面量 |
| `map` | 映射字面量 |
| `true` / `false` | 布尔字面量 |
| `u8` .. `u128` | 整数类型 |
| `bytes` | 字节类型 |
| `address` | 地址类型 |

---

## 5. 语法与示例

### 5.1 字面量

```fitsh
123                    // 整数
0xABC123               // 十六进制字节
0b11110000             // 二进制字节（8*n 位）
"hello \"world\" \n"   // 字符串（字节）带转义
'A'                    // 字符字面量（字节）
nil                    // Nil
true                   // 布尔
false                  // 布尔
emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS  // 地址
```

### 5.2 数组与列表

```fitsh
[1, 2, 3]              // 数组字面量
[]                     // 空列表
list { 1 2 3 }         // list 关键字（空格分隔）
```

### 5.3 映射

```fitsh
map { "key": "value", 1: addr }
map { }                // 空映射
```

### 5.4 运算符

| 类别 | 运算符 |
|------|--------|
| 算术 | `+`、`-`、`*`、`/`、`%`、`**` |
| 位运算 | `<<`、`>>`、`&`、`|`、`^` |
| 比较 | `==`、`!=`、`<`、`<=`、`>`、`>=` |
| 逻辑 | `&&`、`||`、`!` |
| 连接 | `++` |

优先级（高到低）：`!` → `**` → `*`/`/`/`%` → `+`/`-` → `<<`/`>>` → `>=`/`<=`/`>`/`<` → `==`/`!=` → `&` → `^` → `|` → `&&` → `||` → `++`

### 5.5 复合赋值

```fitsh
x += 1
x -= 1
x *= 2
x /= 2
```

### 5.6 控制流

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

### 5.7 块表达式

块 `{ stmt; stmt; value }` 执行语句并返回最后一个表达式：

```fitsh
var result = {
    var inner = 10
    inner + 1
}
// result == 11
```

---

## 6. 特殊语言结构

### 6.1 `param { ... }`

将列表参数解包到局部槽位 0、1、2、...

```fitsh
param { owner amount fee }
// owner -> 槽位 0, amount -> 槽位 1, fee -> 槽位 2
```

- 必须出现在函数体开头
- 规范 IR：`UPLIST(PICK0, P0)`

### 6.2 `callcode lib_idx::func_sig`

- 无参数；尾调用
- 必须紧跟 `end`
- 用于底层委托

```fitsh
callcode 0::0xabcdef01
end
```

### 6.3 `bytecode { ... }`

按名称或编号注入原始字节码操作码：

```fitsh
bytecode { POP DUP SWAP }
```

### 6.4 `list { ... }`

列表的另一种写法（空格分隔）：

```fitsh
list { 1 2 3 }
```

### 6.5 `map { ... }`

使用 `:` 分隔的键值对：

```fitsh
map { "k": "v", 1: addr }
```

### 6.6 `log { ... }`

记录 2..5 个参数的事件。支持 `()`、`{}`、`[]` 分隔符：

```fitsh
log(1, 2)
log[1, 2, 3, 4, 5]
```

---

## 7. 隐式与显式类型转换

### 7.1 隐式转换

| 上下文 | 允许 | 主要拒绝 |
|--------|------|----------|
| 算术 | 整数拓宽；Bytes→Uint（trim 后 1..16 字节） | 空字节；bytes >16；Bool/Address/Nil |
| 字节操作（连接、切片） | Bool/Uint/Address→Bytes | Nil |
| 分支 | 真值（任意值） | — |
| 调用参数 | 整数拓宽；Bytes↔Address | Bytes→Uint，Bool→任意 |

### 7.2 显式转换

```fitsh
x as u8
x as u16
x as u32
x as u64
x as u128
x as bytes
x as address
```

### 7.3 类型检查

```fitsh
x is nil
x is not nil
x is list
x is map
x is u64
x is bytes
x is address
```

### 7.4 注意事项

1. **算术允许 Bytes→Uint，比较不允许**：`Bytes([0x01]) == U8(1)` 会失败；需显式转换。
2. **Bytes↔Uint 不对称**：Uint→Bytes 是固定宽度；Bytes→Uint 使用 trim + 可变宽度。
3. **空字节**：无法作为零参与算术；如需要则用 `0 as u64` 归一化。

---

## 8. 变量与局部栈槽位

### 8.1 变量类型

| 声明 | 槽位 | 可变性 | 求值 |
|------|------|--------|------|
| `var` | 分配 | 可变 | 立即 |
| `let` | 分配 | 不可变 | 立即 |
| `bind` | 无槽位 | — | 惰性（引用时） |

### 8.2 槽位寻址

- `$0`、`$1`、... 直接引用槽位
- `var x $5 = 10` 显式写入槽位 5
- `param { a b c }` 将 a→0、b→1、c→2

### 8.3 直接槽位引用（`$0`、`$1`、...）

以 `$` 开头、后跟十进制数（0–255）的标识符是**直接槽位引用**。它们不经过符号表，直接绑定到指定索引的局部槽位。

#### 语法

| 形式 | 含义 |
|------|------|
| `$N` | 读取槽位 N（0 ≤ N ≤ 255） |
| `$N = expr` | 直接写入槽位 N |
| `var name $N = expr` | 将名称绑定到槽位 N 并赋值 |

#### 与 `param` 的关系

`param { a b c }` 会将参数解包到槽位 0、1、2。之后：

- `$0` 等同于第一个参数
- `$1` 等同于第二个参数
- `$2` 等同于第三个参数

可以通过 `$N` 读写，而无需使用参数名。

#### 读取与写入

```fitsh
param { owner amount }
$0 = "new owner"       // 写入槽位 0（覆盖 owner）
let first = $0         // 从槽位 0 读取
$4 = 999               // 写入槽位 4（若已分配）
```

#### 在 `var` / `let` 中显式指定槽位

将槽位绑定到名称：

```fitsh
var opt $10 = 123      // 将 "opt" 绑定到槽位 10，赋值为 123
let first_arg = $0     // 将槽位 0 读取到新绑定
```

#### 槽位冲突

- 同一槽位只能被带显式 `$N` 的 `var` 或 `let` 绑定一次
- 避免混用 `$N` 写入与对同一槽位的其他绑定
- 手工槽位通过 `reserve_slot` 预留；重复使用会触发 `slot N already bound`

#### 使用场景

| 场景 | 示例 |
|------|------|
| 覆盖参数 | `$0 = "new owner"` |
| 底层槽位访问 | 用 `$N` 精细控制 |
| 与 `unpack_list` 配合 | 先 `unpack_list(...)`，再使用 `$2`、`$3` 等 |
| 调试 / 检查 | 按索引查看槽位内容 |

#### 注意

- 直接写入会绕过常规检查（如 `let` 的不可变性）
- `$N` 可能覆盖参数或其他局部变量，需谨慎使用
- 槽位需先分配（如通过 `param` 或带显式 `$N` 的 `var`/`let`）才能使用

### 8.4 示例

```fitsh
param { owner amount }
var total = 200        // 自动分配槽位
var opt $10 = 123      // 显式槽位 10
$0 = "new owner"       // 写入槽位 0 (owner)
let first = $0         // 从槽位 0 读取
```

---

## 9. 合约资源空间

合约可使用六种资源空间。理解其作用域、生命周期和适用场景，有助于正确选择资源。

### 9.1 概览

| 资源 | 作用域 | 生命周期 | 键/索引 | 最大容量 | Fitsh API |
|------|--------|----------|---------|----------|-----------|
| **Locals** | 每次函数调用 | 调用期间 | 槽位索引 (0–255) | 256 槽位 | `var`、`let`、`param`、`$N` |
| **Heap** | 每次函数调用 | 调用期间 | 字节偏移 | 64 段 × 256 B | `heap_grow`、`heap_write`、`heap_read` |
| **Memory** | 每个合约 | 交易内 | 键（bytes） | ~16 键 | `memory_put`、`memory_get` |
| **Global** | 交易内全局 | 交易内 | 键（bytes） | ~20 键 | `global_put`、`global_get` |
| **Storage** | 每个合约 | 持久（需租金） | 键（bytes） | 租金决定 | `storage_load`、`storage_save`、`storage_del`、`storage_rest`、`storage_rent` |
| **Log** | 每个合约 | 持久（上链） | — | 每条 2–5 参数 | `log(...)` |

### 9.2 Locals（局部栈槽位）

**作用域**：当前函数调用（帧）。  
**生命周期**：到函数返回为止；随后回收。

**适用场景**：
- 局部变量（`var`、`let`、`param`）
- 中间临时值
- 参数与返回值暂存

**特点**：
- 按槽位索引（0–255）
- 访问快，无需哈希
- 返回时自动回收

**示例**：`param { a b }` → a 在槽位 0，b 在槽位 1；`var x = 1` → 自动分配槽位。

### 9.3 Heap（堆）

**作用域**：当前函数调用（帧）。  
**生命周期**：到函数返回为止。

**适用场景**：
- 单次调用内的大块二进制数据
- 按偏移访问（类似 C 数组）
- 通过 `HeapSlice` 向被调用方传递原始字节

**特点**：
- 字节数组；按段扩展（每段 256 字节）
- `heap_grow(n)` 分配；`heap_write(offset, data)` / `heap_read(offset, len)` 访问
- `heap_read_uint`、`heap_write_x` 用于固定宽度整数
- 最多 64 段（约 16 KB）

**示例**：在单次调用内解析或构建二进制结构。

### 9.4 Memory（合约临时存储）

**作用域**：每个合约（ctxadr）。  
**生命周期**：仅当前交易；交易结束后清空。

**适用场景**：
- 同一交易内多次调用同一合约时传递数据
- 多步骤流程（如 充值 → 兑换 → 提现）
- 交易内跨调用的中间状态

**特点**：
- 键值对（键为 bytes）
- 每个合约有独立内存
- 不持久；仅在交易内有效

**示例**（AMM）：`prepare` 保存 `in_sat`、`in_zhu`；`PayableSAT` / `PayableHAC` 读取并完成流程。

```fitsh
// 步骤 1：prepare
memory_put("in_sat", sat)
memory_put("in_zhu", zhu)

// 步骤 2：PayableHAC（同一交易内后续调用）
var in_zhu = memory_get("in_zhu")
memory_put("in_sat", nil)  // 使用后清空
```

### 9.5 Global（交易全局临时）

**作用域**：整个交易。  
**生命周期**：仅当前交易。

**适用场景**：
- 同一交易内多合约共享数据
- 交易级标志或计数器
- 跨合约协同

**特点**：
- 交易内唯一键值映射
- 所有合约共享同一映射
- 不持久

**示例**：多合约共用的交易级 step 或 id。

```fitsh
global_put("tx_step", 1)
// ... 同一交易内，另一合约
var step = global_get("tx_step")
```

### 9.6 Storage（合约状态）

**作用域**：每个合约（ctxadr）。  
**生命周期**：持久；需支付租金；跨区块保留。

**适用场景**：
- 持久状态（余额、配置、总量等）
- 需跨交易和区块保留的数据

**特点**：
- 键值对；键为 bytes（如 `"b_" ++ addr`）
- 按租金管理；`storage_rent(key, amount)` 支付
- `storage_rest(key)` 查询到期
- 值类型：Nil、Bool、Uint、Address、Bytes
- 单值最大约 1280 字节

**示例**：代币余额、AMM 储备、配置。

```fitsh
bind bk = "b_" ++ addr
var balance = storage_load(bk)
if balance is nil {
    balance = 0 as u64
}
storage_save(bk, balance + amount)
```

### 9.7 Log（事件）

**作用域**：每个合约（ctxadr）。  
**生命周期**：持久（链上事件）。

**适用场景**：
- 向索引器或前端发出事件
- 操作审计记录

**特点**：
- 每条 2–5 个参数
- 上链存储；合约内不可查询

**示例**：`log("Transfer", from, to, amount)`

### 9.8 选择指南

| 需求 | 资源 |
|------|------|
| 局部变量 / 参数 | Locals |
| 单次调用内大块二进制 | Heap |
| 同一合约多次调用间传参 | Memory |
| 同一交易内多合约共享 | Global |
| 持久合约状态 | Storage |
| 发出事件 | Log |

### 9.9 小结

- **Locals / Heap**：调用级；用于单次调用内计算。
- **Memory**：合约级，仅交易内；用于交易内多步骤流程。
- **Global**：交易级；用于跨合约协同。
- **Storage**：合约级，持久；用于长期状态。
- **Log**：合约级，持久；用于事件。

---

## 10. `bind` 宏绑定

### 10.1 行为

- **惰性求值**：声明时不求值；引用时才求值
- **无槽位**：不分配；无 `PUT`/`GET`
- **内联展开**：每次引用时复制表达式模板

### 10.2 使用场景

```fitsh
bind bk = "b_" ++ addr
var balance = storage_load(bk)
storage_save(bk, balance + 100)
```

### 10.3 注意

`bind` 表达式中的副作用（如 `storage_save`、`print`）只有在绑定被**读取**时才会执行。若从不读取，则不会发生。如有副作用请使用 `var` 立即执行。

---

## 11. 其他特色语法

### 11.1 函数调用语法

| 语法 | 操作码 | 用途 |
|------|--------|------|
| `lib.func(...)` | CALL | 状态变更调用 |
| `lib:func(...)` | CALLVIEW | 视图调用 |
| `lib::func(...)` | CALLPURE | 纯调用 |
| `this.func(...)` | CALLTHIS | 当前合约 |
| `self.func(...)` | CALLSELF | 当前合约 |
| `super.func(...)` | CALLSUPER | 继承链父级 |

### 11.2 调用权限与状态访问控制

VM 基于 **ExecMode**（执行模式）和 **in_callcode** 实施权限控制。每种调用类型会切换到特定模式，并限制被调用方可执行的操作。

#### 调用类型 → 被调用方模式

| 调用 | 语法 | 被调用方模式 | 状态读取 | 状态写入 |
|------|------|--------------|----------|----------|
| `call` | `lib.func(...)` | Outer | 允许 | 允许 |
| `callview` | `lib:func(...)` | View | 允许 | 禁止 |
| `callpure` | `lib::func(...)` | Pure | 禁止 | 禁止 |
| `callcode` | `callcode lib::sig` | 继承调用方 | 继承 | 继承 |
| `callthis` | `this.func(...)` | Inner | 允许 | 允许 |
| `callself` | `self.func(...)` | Inner | 允许 | 允许 |
| `callsuper` | `super.func(...)` | Inner | 允许 | 允许 |

**状态** = 存储、全局、内存、日志。

**重要说明**：`callcode` 在**当前帧**中执行并**完全继承调用方的 ExecMode** —— 它**没有独立的状态访问控制逻辑**。callcode 体内的所有状态操作（存储读写、EXTACTION/EXTENV/EXTVIEW、NTFUNC/NTENV）均受继承模式的权限限制。此外，`callcode` 设置 `in_callcode = true`，禁止任何后续嵌套调用（CallInCallcode 错误）。

#### 各入口/执行模式下的允许调用

| 模式 | 允许的调用 | 禁止的调用 |
|------|------------|------------|
| **Main**（交易主入口） | CALL、CALLVIEW、CALLPURE、CALLCODE | CALLTHIS、CALLSELF、CALLSUPER |
| **P2sh**（脚本验证） | CALLVIEW、CALLPURE、CALLCODE | CALL、CALLTHIS、CALLSELF、CALLSUPER |
| **Abst**（支付钩子） | CALLTHIS、CALLSELF、CALLSUPER、CALLVIEW、CALLPURE、CALLCODE | CALL（Outer） |
| **Outer**（嵌套合约） | 全部 | — |
| **Inner**（this/self/super） | 全部 | — |
| **View**（只读） | CALLVIEW、CALLPURE | CALL、CALLTHIS、CALLSELF、CALLSUPER |
| **Pure**（无状态） | 仅 CALLPURE | 其余全部 |
| **in_callcode**（在 CALLCODE 内） | 无 | 全部（禁止嵌套调用） |

**Abst** 禁止 CALL（Outer），防止支付钩子通过外部合约重入。

| 模式/入口 | CALL | CALLVIEW | CALLPURE | CALLCODE | CALLTHIS | CALLSELF | CALLSUPER |
|---|---|---|---|---|---|---|---|
| Main | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| P2sh | ❌ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Abst | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Outer/Inner | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| View | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Pure | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Callcode | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

#### 各模式下的状态访问控制矩阵

**存储/全局/内存/日志访问**：

| 模式 | 存储读取 | 存储写入 | 全局/内存读取 | 全局/内存写入 | 日志 |
|------|----------|----------|---------------|---------------|------|
| Main、P2sh、Abst | 允许 | 允许 | 允许 | 允许 | 允许 |
| Outer、Inner | 允许 | 允许 | 允许 | 允许 | 允许 |
| View | 允许 | 禁止 | 允许 | 禁止 | 禁止 |
| Pure | 禁止 | 禁止 | 禁止 | 禁止 | 禁止 |

**外部调用（EXTACTION / EXTENV / EXTVIEW）**：

| 模式 | EXTACTION | EXTENV | EXTVIEW | 备注 |
|------|-----------|--------|---------|------|
| **Main**（depth==0，不在 in_callcode 中） | ✅ 允许 | ✅ 允许 | ✅ 允许 | 完全访问 |
| **Main**（depth>0 或 in_callcode 中） | ❌ 禁止 | ✅ 允许 | ✅ 允许 | EXTACTION 在嵌套调用/callcode 中被阻止 |
| **P2sh、Abst** | ❌ 禁止 | ✅ 允许 | ✅ 允许 | EXTACTION 仅限入口层 |
| **Outer、Inner** | ❌ 禁止 | ✅ 允许 | ✅ 允许 | EXTACTION 仅限入口层 |
| **View** | ❌ 禁止 | ✅ 允许 | ✅ 允许 | 只读环境访问 |
| **Pure** | ❌ 禁止 | ❌ 禁止 | ❌ 禁止 | 无外部状态访问 |

**原生函数（NTFUNC / NTENV）**：

| 操作码 | 原生调用 | 参数数 | Pure 模式 | View 模式 | Main/Outer/Inner | 功能 |
|--------|----------|--------|-----------|-----------|------------------|------|
| NTENV | `context_address` | 0 | ❌ 禁止（`nsr!`） | ✅ 允许 | ✅ 允许 | 读取 VM 执行状态 |
| NTFUNC | `sha2/sha3/ripemd160` | 1 | ✅ 允许 | ✅ | ✅ | 纯哈希函数 |
| NTFUNC | `hac_to_mei/zhu`、`mei/zhu_to_hac` | 1 | ✅ 允许 | ✅ | ✅ | 纯金额转换 |
| NTFUNC | `address_ptr` | 1 | ✅ 允许 | ✅ | ✅ | 纯地址指针提取 |

**小结**：
- **EXTACTION**（资产转移）：仅 `Main` 模式在 `depth == 0` 且**非** `callcode` 中
- **EXTENV**（`block_height`、`tx_main_address`）：在 `Pure` 中禁止，其他允许
- **EXTVIEW**（`check_signature`、`balance`）：在 `Pure` 中禁止，其他允许 —— 只读链状态查询
- **NTFUNC**（纯计算）：所有模式均允许，包括 `Pure`
- **NTENV**（`context_address`）：在 `Pure` 中禁止（读取 VM 状态），其他允许

#### EXTACTION 限制

| 条件 | EXTACTION 是否允许 |
|------|-------------------|
| mode == Main 且 depth == 0 且未处于 in_callcode | 允许 |
| mode != Main 或 depth > 0 或 in_callcode | 禁止 |

`transfer_hac_to`、`transfer_sat_to` 等只能在顶层主调用中执行。在 `callcode`、抽象/支付钩子及嵌套调用中均禁用。

#### 小结

- **call** → Outer：完全状态访问；被调用方须为 `public`
- **callview** → View：只读；禁止存储/全局/内存/日志写入
- **callpure** → Pure：无状态访问；仅纯计算及嵌套 CALLPURE
- **callcode** → 继承当前模式；禁止嵌套调用；EXTACTION 禁用
- **callthis/callself/callsuper** → Inner：完全状态访问；仅内部调用

### 11.3 函数查找：`this`、`self` 与 `super`

VM 在执行过程中维护两个关键地址：

- **ctxadr**（上下文地址）：存储/日志的所有者 —— 最初被调用的合约（入口）。在嵌套内部调用中保持不变。
- **curadr**（当前地址）：代码所有者 —— 当前正在执行其代码的合约。当解析到的调用指向不同合约时，会随之变化。

| 调用 | 解析范围 | 查找顺序 |
|------|----------|----------|
| `this.func(...)` | ctxadr | DFS：当前合约 → 继承链（按顺序） |
| `self.func(...)` | curadr | DFS：当前合约 → 继承链（按顺序） |
| `super.func(...)` | 仅 curadr 的父级 | DFS：跳过 curadr，在直接继承中查找 → 其继承链 |

**何时会不同？** 当 `super` 或 `self` 将执行转移到父级代码时：`curadr` 变为父级，但 `ctxadr` 仍为子级。此时 `this` 仍在子级（存储上下文）中解析，而 `self` 在父级（当前代码所有者）中解析。

#### 示例 1：直接调用（无继承）

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

此处 `this`、`self`、`super` 都在同一合约中解析。`this.do_transfer(...)` 与 `self.do_transfer(...)` 行为相同。

#### 示例 2：继承 —— `inherit` 链

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

- `Child.run()` 调用 `super.compute()` → 在 Parent 中解析（跳过 Child）。执行 Parent 的 `compute()`。
- **ctxadr** = Child（不变）
- **curadr** = Parent（当前代码所有者）

在 Parent 的 `compute()` 内：

- `this.get_value()` → 在 **ctxadr**（Child）中解析 → Child 的 `get_value()` → **1**
- `self.get_value()` → 在 **curadr**（Parent）中解析 → Parent 的 `get_value()` → **2**
- `super.get_value()` → 跳过 Parent，在 Parent 的继承中查找 → Base 的 `get_value()` → **3**

结果：`1*10000 + 2*100 + 3 = 10203`

#### 示例 3：继承顺序（先匹配优先）

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

`self.f()` 按 C → A → B 顺序查找。A 先定义 `f`，因此结果为 **10**。继承顺序决定优先级。

#### 示例 4：`super` 跳过当前合约

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

`super.helper()` 跳过 Child，仅在 Base 中查找。结果：**100**（Base 的实现）。

#### 示例 5：何时使用各自

| 用途 | 推荐 |
|------|------|
| 调用自身或继承函数，从存储上下文解析 | `this` |
| 从当前代码所有者调用（如 `super` 进入父级后） | `self` |
| 调用父级实现，绕过当前覆盖 | `super` |

**总结**：`this` = 存储上下文；`self` = 当前代码所有者；`super` = 仅父级链。

### 11.4 原生调用

| 函数 | 说明 |
|------|------|
| `context_address()` | 当前执行上下文地址 |
| `block_height()` | 当前区块高度 |
| `sha2(data)` | SHA-256 哈希 |
| `sha3(data)` | SHA3 哈希 |
| `ripemd160(data)` | RIPEMD-160 哈希 |
| `hac_to_mei(n)` | HAC 转 mei |
| `hac_to_zhu(n)` | HAC 转 zhu |
| `mei_to_hac(n)` | Mei 转 HAC |
| `zhu_to_hac(n)` | Zhu 转 HAC |

### 11.5 扩展动作（EXTACTION）

| 函数 | 说明 |
|------|------|
| `transfer_hac_to(addr, amount)` | 转账 HAC |
| `transfer_hac_from(addr, amount)` | 从地址转出 HAC |
| `transfer_hac_from_to(from, to, amount)` | 在地址间转账 HAC |
| `transfer_sat_to`、`transfer_sat_from`、`transfer_sat_from_to` | SAT 转账 |
| `transfer_hacd_single_to`、`transfer_hacd_to` 等 | HACD 转账 |
| `transfer_asset_to`、`transfer_asset_from`、`transfer_asset_from_to` | 资产转账 |

**注意**：`callcode` 上下文中禁用 EXTACTION。

### 11.6 存储函数

| 函数 | 说明 |
|------|------|
| `storage_load(key)` | 加载值 |
| `storage_save(key, value)` | 保存值 |
| `storage_del(key)` | 删除键 |
| `storage_rest(key)` | 获取租期到期 |
| `storage_rent(key, amount)` | 支付租金 |

### 11.7 内存与堆

| 函数 | 说明 |
|------|------|
| `memory_put(key, value)` | 写入内存 |
| `memory_get(key)` | 从内存读取 |
| `global_put(key, value)` | 全局存储 |
| `global_get(key)` | 全局读取 |
| `heap_grow(n)` | 扩展堆 |
| `heap_write(offset, data)` | 写入堆 |
| `heap_read(offset, len)` | 从堆读取 |

### 11.8 数据结构函数

| 函数 | 说明 |
|------|------|
| `length(list)` | 列表长度 |
| `keys(map)` | 映射键 |
| `values(map)` | 映射值 |
| `haskey(map, key)` | 检查键 |
| `head(list)` | 首元素 |
| `back(list)` | 末元素 |
| `append(list, item)` | 追加 |
| `insert(list, index, item)` | 插入 |
| `remove(list, index)` | 移除 |
| `clone(val)` | 克隆 |
| `clear(collection)` | 清空 |

### 11.9 缓冲区函数

| 函数 | 说明 |
|------|------|
| `buf_cut(buf, start, len)` | 切片 |
| `buf_left(n, buf)` | 左侧 n 字节 |
| `buf_right(n, buf)` | 右侧 n 字节 |
| `buf_left_drop(n, buf)` | 丢弃左侧 n 字节 |
| `buf_right_drop(n, buf)` | 丢弃右侧 n 字节 |
| `byte(buf, index)` | 索引处字节 |
| `size(buf)` | 大小 |

### 11.10 其他扩展函数

| 函数 | 说明 |
|------|------|
| `check_signature(addr)` | 验证签名 |
| `balance(addr)` | 余额字节 |

### 11.11 特别注意：可选尾逗号/分号

Fitsh 允许在**语句和表达式末尾省略逗号或分号**。这是与多数类 C 语言不同的语法特征。

- **语句**：`var x = 1` 与 `var x = 1;` 等价
- **表达式序列**：`list { 1 2 3 }` 中元素用空格分隔，末尾无需逗号
- **顶层元素**：`library`、`inherit` 等数组项之间逗号可选

示例：

```fitsh
var a = 1
var b = 2
list { 1 2 3 }
map { "k": "v" }
```

从 Solidity 等语言转来的开发者需注意：Fitsh 不强制要求行尾分号。

---

## 快速参考

- **函数参数上限**：15（pack list）；更多时用 `list`/`map` 包装
- **函数签名**：仅按名称 4 字节哈希；无重载
- **`param`**：必须位于函数体开头
- **`callcode`**：必须紧跟 `end`
- **`bind`**：惰性；有副作用时用 `var`
