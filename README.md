# FrogCalc

**FrogCalc** 是一款纯 Rust 实现的精确优先桌面计算器与计算机代数系统（CAS）。界面使用 **egui/eframe** 原生渲染，不依赖 WebView、React 或第三方数学引擎。

- 任意大整数、精确有理数、50 位可调精度的十进制近似
- 精确复数运算
- 无限循环小数精确解析：`recurring("0.1(3)") → 2/15`
- 自研语言：词法分析、Pratt 语法分析、AST 解释执行
- 符号化简、展开、因式分解（Berlekamp–Zassenhaus）
- 求导、积分、方程求解
- 原生 egui 界面：深色 / 浅色可切换，工作区、历史、输入、设置、持久化
- 命令模式 / 脚本模式 / 绘图模式：单条表达式、整段脚本或 2D/3D 函数图像
- 绘图支持显式函数与隐函数：`y = f(x)`、`z = f(x, y)`、`F(x, y) = 0`、`F(x, y, z) = 0`
- 脚本支持 if/else、while、for、break/continue 与多语句函数，可导入/导出脚本文件
- 分段函数、变量假设驱动的化简（`assume(x > 0)`）
- 内置帮助文档：语法、运算符、数值函数、CAS 函数与示例
- 单 exe、启动快、完全离线

数学原理、算法、精度策略与工程限制统一整理在 [docs/ALGORITHMS.md](docs/ALGORITHMS.md)。

## 快速开始

需要 Rust 1.95+（当前使用 1.100）与 Windows MSVC 链接器。构建工具链安装：

```powershell
rustup default stable
```

```bash
cargo run -p frogcalc-app      # 启动原生界面
cargo test -p frogcalc-core    # 运行内核测试
cargo build --release -p frogcalc-app
```

Release 产物：

```text
target/release/frogcalc.exe
```

当前机器已生成一份可直接运行的副本：

```text
dist/FrogCalc.exe
```

如果 crates.io 访问缓慢，可在 `%USERPROFILE%\.cargo\config.toml` 配置镜像：

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

## 语言设计

FrogCalc 语言追求“接近数学书写、少特殊符号”：

```text
# 变量与精确分数
a = 1 / 3
b = 2 / 3
a + b                 # 1

# 函数定义与隐式乘法
f(x) = x^2 + 1
f(3)                  # 10
2x + 3(x - 1)

# 根式精确化简
sqrt(2) * sqrt(2)     # 2
sqrt(12)              # 2*sqrt(3)

# 复数
(2 + 3i) * (4 - 5i)   # 23 + 2i
i^2                   # -1

# 微积分
diff(x^3 * sin(x), x)
integrate(x * exp(x), x)
integrate(1/(x^2 - 1), x)

# 方程
solve(x^2 - 2x - 3, x)
solve(sin(x) == 0, x)

# 近似
N(pi, 50)

# 分段函数与假设
f(x) = piecewise(x < 0, -x, x)
assume(x > 0)
sqrt(x^2)             # x
```

### 语法要点

| 语法 | 含义 |
| --- | --- |
| `a = 表达式` | 赋值 |
| `f(x) = 表达式` | 定义单表达式函数 |
| `+ - * / ^` | 四则与乘方，`^` 右结合 |
| `2x`、`3(x+1)`、`(x+1)(x-1)` | 隐式乘法 |
| `!` | 后缀阶乘，如 `5!` |
| `==` | 方程等号；在 `if` / `while` 条件中判断相等 |
| `<` `<=` `>` `>=` `!=` | 比较运算，结果为 `true` / `false` |
| `and` `or` `not` | 逻辑运算，`and` / `or` 短路 |
| `if 条件 { ... } else { ... }` | 条件分支，支持 `else if` |
| `while 条件 { ... }` | 条件循环 |
| `break` / `continue` | 跳出循环 / 跳过本次迭代 |
| `for i in range(1, 10) { ... }` | 范围循环，包含终点 |
| `for x in {1, 2, 3} { ... }` | 遍历列表 |
| `piecewise(c1, v1[, c2, v2 ...][, 默认])` | 分段函数，条件按顺序判断 |
| `assume(x > 0)` | 变量假设，支持 `>` `>=` `<` `<=` `!=` |
| `f(x) = { ... }` | 多语句函数，最后一个语句为返回值 |
| `{a, b, c}` | 列表字面量 |
| `recurring("0.1(3)")` | 无限循环小数转精确分数 |
| `# 注释` | 行注释 |
| `;` | 语句分隔，并抑制该句输出 |
| `ans` | 上一次计算结果 |
| 换行 | 分隔语句；括号内允许换行 |

所有数字字面量都进入精确有理数域：`0.1` 存为 `1/10`，`1/3` 保持 `1/3`。**有理数结果统一显示为分数**（如 `1/2`、`3/10`），小数近似显示在结果下方。只有显式调用 `N(...)` 或结果卡中的“近似”才转换为 `BigFloat`。

### 输入模式

顶栏可以在三种模式之间切换：

- **命令模式**：底部单行输入，按 Enter 立即计算，适合逐条探索。
- **脚本模式**：上方是多行脚本编辑器，下方显示运行结果（变量列表与最终结果），不写入历史。按 Ctrl+Enter 或点击“运行”执行整段脚本。
- **绘图模式**：输入 `f(x)` 或 `f(x, y)`，在 2D 曲线与 3D 曲面之间切换；3D 曲面可拖动旋转。
- 顶栏提供「清空工作区」「清空历史」按钮，操作前会二次确认。

脚本按书写顺序执行，变量与函数定义跨行共享；遇到错误会停止后续语句。运行后底部列出当前变量和最后一条语句的结果。脚本内容与模式会随界面状态一起保存。

脚本文件可以导入/导出（仅文本）：脚本模式右上角的「导入」「导出」按钮、快捷键 Ctrl+O / Ctrl+S，或直接把 `.frogcalc` / `.frog` / `.calc` / `.txt` 文件拖进窗口。

```text
a = 1/3
b = 2/3
a + b                  # 1

f(x) = x^2 + 1
diff(f(x), x)          # 2x
N(pi, 40)
```

### 脚本控制流

脚本模式支持 `if` / `else`、`while`、`for ... in ...`。条件使用比较运算与 `and` / `or` / `not`；代码块用 `{ }`，语句之间用换行或 `;` 分隔。

```text
total = 0
for i in range(1, 10) {
    total = total + i
}
total                  # 55

if total == 55 {
    tag = "正确"
} else if total > 40 {
    tag = "接近"
} else {
    tag = "错误"
}
tag                    # 正确
```

- `range(a, b)` 生成从 `a` 到 `b` 的整数列表（包含两端），`range(a, b, step)` 指定步长。
- 也可以直接遍历列表：`for x in {1, 2, 3} { ... }`。
- `break` 结束最内层循环，`continue` 跳过本次迭代的剩余语句。
- 控制流语句本身不产生输出；在脚本末尾写一个表达式查看最终结果。
- 循环变量只在循环期间可见，结束后自动恢复；循环总次数上限为 100000。

```text
total = 0
for i in range(1, 10) {
    if i == 5 {
        continue
    }
    if i > 8 {
        break
    }
    total = total + i
}
total                  # 1+2+3+4+6+7+8 = 31
```

### 函数定义

函数体可以是一个表达式，也可以用 `{ }` 写成多条语句：

```text
f(x) = x^2 + 1

sumto(n) = {
    total = 0
    for i in range(1, n) {
        total = total + i
    }
    total
}

fact(n) = {
    if n <= 1 {
        1
    } else {
        n * fact(n - 1)
    }
}
```

- 多语句函数以最后一个语句的值作为返回值。
- 函数内的赋值是局部的，不会修改同名全局变量；参数只在该次调用内可见。
- 支持递归，调用深度上限 200。
- `f(x) = {1, 2, 3}` 仍按列表处理（含逗号）；要返回列表可先赋值再返回。
- 函数内部目前不能再定义函数；函数体内的循环可以使用 `break` / `continue`，但不能跨函数边界跳出外层循环。

### 分段函数与假设

```text
f(x) = piecewise(x < 0, -x, x >= 0, x)
f(-2)                  # 2
diff(f(x), x)          # piecewise(x < 0, -1, x >= 0, 1)

assume(x > 0)
sqrt(x^2)              # x
abs(x)                 # x
forget(x)              # 删除假设
```

- `piecewise(条件1, 值1[, 条件2, 值2 ...][, 默认值])`：条件依次判断，只有前面的条件都确定为假时才使用默认值；条件无法判定时保留 `piecewise(...)` 形式。
- `assume(x > 0)` 记录变量符号性质；`assumptions()` 查看、`forget(x)` 删除、`clearassumptions()` 全部清除。
- 假设会随工作区一起保存，影响结果展示与 `simplify(...)`（例如 `abs(x)`、`sqrt(x^2)` 的化简），但不会改写工作区里存放的原始表达式。

### 绘图

绘图模式重新采样当前表达式：

- 2D：使用变量 `x`，绘制 `y = f(x)`，自动确定 y 范围（也可关闭自动并手动指定，超出范围的部分会被裁剪并提示），采样点数量可调。
- 3D：使用变量 `x` 与 `y`，绘制 `z = f(x, y)`；`z` 是高度（因变量），不是输入变量。鼠标拖动旋转视角，可在实体曲面与线框之间切换。也可以直接输入 `z = x^2 - y^2`，绘图会取等号右侧。
- **隐函数**：输入 `x^2 + y^2 = 1` 或 `x^2 + y^2 + z^2 = 1` 这样的方程（单个 `=` 或 `==` 都可以），按 `F = 0` 的零等值面绘制。2D 使用 marching triangles，3D 使用 marching tetrahedra；3D 隐函数会显示 `z` 输入范围。
- 输入框下方会实时显示识别结果（「显式曲线」「显式曲面」或「隐函数 F = 0」），并提示当前模式下哪些输入会自动按隐函数处理；在 2D 下输入含 `z` 的方程会自动切换到 3D。
- 隐函数第一次绘制时会按曲面包围盒自动缩放（例如球面默认就落在 `[-10,10]^3` 里），也可以手动点「自动范围」；范围被改乱时点「重置」恢复 `[-10,10]` 再自动适应；找不到等值面时会提示。
- 修改表达式、范围、采样密度等参数后会自动重新采样（2D 约 60 ms、3D 约 220 ms 防抖），无需手动点「绘制」。
- 用户函数、变量、分段函数与角度制设置都会生效；`1/x` 这类无定义点会自动断开。
- 目前不支持参数曲线/曲面（`x(t), y(t)`、`x(u,v), y(u,v), z(u,v)`）。

### 结果显示

- 默认“精确 + 近似”：主结果精确，下方给出约 20 位近似值；若该小数是精确等价的有限小数，使用 `=`，否则使用 `≈`
- 设置中可切换“仅精确”“精确 + 近似”“近似优先”
- 近似精度默认 50 位有效数字，可调 10–500
- 角度制默认弧度，可切换 `deg`
- 结果用 Unicode/纯文本数学排版（`√`、`π`、`x²`、`·`），不依赖 KaTeX

## 内置函数

### 数值

`abs`、`sign`、`floor`、`ceil`、`round`、`min`、`max`、`gcd`、`lcm`、`mod`、`factorial`、`binomial`

### 无限循环小数

| 调用 | 说明 |
| --- | --- |
| `recurring("0.1(3)")` | `0.1333...` → `2/15` |
| `recurring("0.(3)")` | `0.333...` → `1/3` |
| `recurring("1.2(34)")` | `1.2343434...` → `611/495` |
| `recurring("-0.0[3]")` | `-0.0333...` → `-1/30` |
| `recurring("2.5")` | 有限小数也接受 → `5/2` |

循环节用 `()` 或 `[]` 包围，且必须写在小数点之后。结果直接保存为精确有理数：`a = recurring("0.1(3)")` 之后，内存里就是 `2/15`，不保留循环形式，也不会先转成浮点。

### 初等函数

`sqrt`、`cbrt`、`root`、`pow`、`exp`、`ln`、`log`、`log10`、`log2`、`sin`、`cos`、`tan`、`asin`、`acos`、`atan`、`sinh`、`cosh`、`tanh`

`log(x)` 为自然对数；`log(x, b)` 为以 `b` 为底的对数。

### 符号运算

| 函数 | 说明 |
| --- | --- |
| `simplify(expr)` | 合并同类项、通分、约去多项式公因子 |
| `expand(expr)` | 展开乘积与整数幂 |
| `factor(expr)` | 一元多项式因式分解（Berlekamp–Zassenhaus + 平方自由分解） |
| `diff(expr, x[, n])` | 求导，支持高阶导与分段函数 |
| `integrate(expr, x)` | 不定积分（省略常数 C） |
| `integrate(expr, x, a, b)` | 定积分；无法求原函数时退化为数值积分 |
| `solve(expr, x)` | 解方程；表达式视为 `= 0`，或使用 `==` |
| `subs(expr, x, value)` | 符号替换 |
| `piecewise(...)` | 分段函数 |
| `N(expr[, digits])` | 近似求值 |

### 假设

| 函数 | 说明 |
| --- | --- |
| `assume(x > 0)` | 记录变量假设，支持 `>` `>=` `<` `<=` `!=` |
| `assumptions()` | 列出当前所有假设 |
| `forget(x)` | 删除变量 `x` 的假设 |
| `clearassumptions()` | 清除全部假设 |

### 工作区

| 函数 | 说明 |
| --- | --- |
| `help("syntax")` | 语言帮助 |
| `clear()` | 清空工作区 |
| `clear("x")` | 删除变量或函数 `x` |
| `vars()` | 列出工作区 |
| `setprecision(n)` | 设置近似精度 |
| `setangle("deg")` / `setangle("rad")` | 切换角度制 |

## 精确性示例

```text
a = 1 / 3
b = 2 / 3
a + b              → 1

sqrt(2) * sqrt(2)  → 2
sqrt(2)^2          → 2
sqrt(8)            → 2*sqrt(2)
0.1 + 0.2          → 3/10（近似 0.3）
recurring("0.1(3)") → 2/15（近似 0.1333...）

1000!              → 2568 位完整整数
2^10000            → 3011 位完整整数
```

数值内核的数值塔：

1. `BigInt`：任意精度整数
2. `BigRational`：约分后的精确分数
3. `BigFloat`：十进制尾数 + 指数 + 可调有效数字
4. `Complex`：实部/虚部为上述数值
5. `Expr`：`Add` / `Mul` / `Pow` / `Function` 符号树

## 项目结构

```text
Cargo.toml                     Rust workspace
crates/
├─ frogcalc-core/              纯 Rust 计算内核（无 UI 依赖）
│  ├─ numeric/                 BigInt / BigRational / BigFloat
│  ├─ parser/                  词法、Pratt 解析器、AST
│  ├─ symbolic/                表达式树、自动化简、展开/因式分解、输出、变量假设
│  ├─ polynomial.rs            多项式、GCD、平方自由分解、有理根、Durand–Kerner
│  ├─ factor_int.rs            Berlekamp–Zassenhaus 整系数因式分解
│  ├─ calculus/                求导、积分、数值积分
│  ├─ solve.rs                 方程求解与逆运算
│  ├─ plot.rs                  2D / 3D 函数采样
│  └─ eval/                    求值会话、近似求值、结果格式化
└─ frogcalc-app/               egui/eframe 原生界面
docs/ALGORITHMS.md             数学原理与算法文档
```

- 计算内核完全不依赖 UI，可复用于 CLI、服务端或其他前端。
- 界面采用 eframe 的 **wgpu** 后端；中式字体在启动时从系统加载（SimHei / DengXian / Microsoft YaHei）。
- 状态保存在程序所在目录的 `state.json`（便携模式）；首次运行会从旧的 `%APPDATA%\FrogCalc\state.json` 自动迁移，并把旧文件改名为 `state.json.bak`。
- 旧 Electron/React 版本已从仓库移除（其实现已被 `crates/frogcalc-core` 完全取代，需要时可从 git 历史提交 `077f4dc` 中取回）。

## 测试

```bash
cargo test -p frogcalc-core
```

当前 28 个测试覆盖：

- 精确有理数、根式、大整数、复数
- 解析器优先级、隐式乘法、函数调用、列表
- 求导、积分、化简、展开、因式分解、方程
- 50/100 位圆周率与超越函数
- 角度制、分号抑制、`ans`、工作区序列化往返
- `if` / `while` / `for`、`range`、比较与逻辑运算
- 多语句函数、局部作用域、递归与工作区序列化
- Berlekamp–Zassenhaus 因式分解、重根与有理函数积分
- 分段函数、break/continue、变量假设与工作区往返
- 无限循环小数解析、精确存储与非法格式报错
- 2D / 3D 采样与无定义点处理
- 错误处理与未求值回退

## 当前边界

- 符号引擎是规则驱动实现，不追求 Mathematica/SymPy 的研究级完备性
- 积分覆盖积分表、线性换元、常见分部积分、对数导数、含重根的有理函数部分分式；不支持完整的 Risch–Liouville 判定，未识别的情形保留 `integrate(...)`
- 因式分解使用 Berlekamp–Zassenhaus，默认效率区间为次数 ≤ 28；更高次数退回有理根 + Durand–Kerner 数值求根
- 三角/周期方程只给出主值分支，不自动生成 `+ 2πk` 通解
- 矩阵、参数曲线、隐函数绘图、函数内嵌套定义尚未实现
- 绘图只支持单条显式函数；计算与采样都在 UI 线程同步执行，重计算可后续迁移到后台线程

## 后续路线

1. 计算内核迁移到后台线程 / 任务队列
2. 矩阵与线性代数
3. macOS / Linux 打包
4. Windows 深色标题栏与安装包、代码签名
5. 参数曲线、隐函数绘图与多曲线对比
6. 完整 Risch–Liouville 判定与代数扩域上的积分

## 许可

MIT License。详见 `LICENSE`。
