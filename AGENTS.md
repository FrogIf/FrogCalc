# AGENTS.md

## Repository shape

- Rust workspace: `crates/frogcalc-core` (pure math/CAS, no UI deps) + `crates/frogcalc-app` (egui/eframe native UI).
- All math, parsing, CAS, formatting logic belongs in `frogcalc-core`; the app only handles UI, persistence, and display.
- `README.md` documents the language and build flow; `docs/ALGORITHMS.md` is the algorithm/reference document. Keep both in sync when changing behavior.

## Commands

```bash
cargo run -p frogcalc-app                 # dev UI (debug build keeps a console for logs)
cargo test -p frogcalc-core               # 27 core tests
cargo check -p frogcalc-app               # faster UI compile check
cargo fmt --all                           # requires rustfmt component
cargo build --release -p frogcalc-app     # release GUI, no console
```

Single test examples:

```bash
cargo test -p frogcalc-core --test session exact_arithmetic
cargo test -p frogcalc-core --test numeric pi_50_digits
```

Release workflow: after `cargo build --release -p frogcalc-app`, copy the binary because `dist/` is gitignored but is the delivered artifact:

```powershell
Copy-Item target\release\frogcalc.exe dist\FrogCalc.exe -Force
```

No CI, pre-commit, lint, or typecheck config exists. Do not invent one.

## Toolchain / Windows quirks

- Requires Rust >= 1.95 for eframe 0.37; current machine has 1.100 + MSVC build tools.
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` in `crates/frogcalc-app/src/main.rs` must stay. Removing it makes the release exe open a console window.
- Keep eframe's `wgpu` feature. Switching to `glow` rendered a black window on this machine.
- eframe 0.37 uses new APIs: implement `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`, and use `egui::Panel::top/left/bottom`, not the old `TopBottomPanel`/`SidePanel` names.
- System CJK fonts are loaded at runtime (`simhei.ttf`, `Deng.ttf`, `msyh.ttc`, `simsun.ttc`); non-Windows ports need a bundled font.
- Windows file icon: `crates/frogcalc-app/build.rs` generates a `.rc` and uses the `embed-resource` build dependency to embed `resources/icon.ico` (multi-size 16–256) into the exe. The runtime window/taskbar icon still comes from `resources/icon.png` via `viewport.with_icon`. Both must be kept in sync when the logo changes.

## Exactness invariants (do not regress)

- Every numeric literal becomes an exact `BigRational`; `BigFloat` is only for explicit approximation / `N(...)` / the preview line.
- Exact rational results display as fractions (`rat_display`: `1/2`, `3/10`), never as terminating decimals.
- The preview line uses `=` when the finite decimal is exact (`1/2 = 0.5`) and `≈` otherwise (`1/3 ≈ 0.333...`). This is driven by `approx_exact`.
- `sqrt(2) * sqrt(2)` must stay `2`; automatic radical/power simplification lives in `symbolic/expr.rs`.
- Core output must remain usable without the UI; `Session::execute` returns per-statement display data and notes, and `Session::workspace()` returns the current workspace state.

## Non-obvious code facts

- `crates/frogcalc-core/src/symbolic/format_expr.rs` is included with `include!("format_expr.rs")` inside `symbolic/expr.rs`. It is one module; edit both together.
- `symbolic/transform.rs` handles expand/simplify/factor; `factor_int.rs` implements Berlekamp–Zassenhaus (mod-p Berlekamp + Hensel lifting via linear congruences + Zassenhaus recombination); `polynomial.rs` handles square-free decomposition, rational roots, quadratic formulas and Durand–Kerner; `calculus/integrate.rs` is rule-based (partial fractions with multiplicities, log-derivative matching) and returns an unevaluated `integrate(...)` call when unsupported.
- `symbolic/assume.rs` stores per-symbol sign facts. `Session::format_value` and the `simplify/expand/factor` builtins apply `Assumptions::rewrite`; stored workspace expressions are never rewritten. Assumptions round-trip through `workspace_source` as `assume(x > 0)` lines.
- `plot.rs` samples expressions through `Session::compile_expression` + `Session::eval_sample` (bindings become `Float`), so user functions and `piecewise` work in plots; the app renders 2D polylines and 3D height fields with the egui painter (no extra crates).
- `numeric/recurring.rs` parses recurring-decimal text (`recurring("0.1(3)")`) with the geometric-series formula `I + N/10^n + R/(10^n(10^m-1))`, returning an exact `BigRational`; `()` and `[]` mark the repeating block, and `recurring` is a reserved builtin that also accepts an ordinary numeric literal.
- Parser semantics: `=` is assignment/function definition, `==` is an equation for `solve` (and equality in conditions); implicit multiplication is supported; `;` separates statements and suppresses that statement's output; newlines are allowed inside `()`/`{}`.
- Script control flow: `if`/`else if`/`else`, `while`, `for x in range(...) | {list} { ... }`, plus `break`/`continue`. `eval_statement_value` returns an internal `Flow` enum (`Value`/`Break`/`Continue`); `Session.loop_depth` rejects `break` outside loops and is reset to 0 across function-call boundaries. Conditions use `< <= > >= != ==` and short-circuiting `and`/`or`/`not`. Comparisons are parsed into `lt/le/gt/ge/neq/and/or/not` calls and printed back as infix, so they round-trip through `workspace_source`. Control statements return `None` from `eval_statement` (no display result) and loop iterations are capped at 100000.
- `piecewise(cond1, value1[, cond2, value2 ...][, default])` is a lazy special form in `eval_call` (unevaluable conditions keep a symbolic `piecewise`); `diff` differentiates branches only, and the approximation path learns `lt/le/...`, `and/or/not` and `piecewise`.
- Function definitions: `f(x) = expr` and `f(x) = { statements }`. Block bodies push a local scope (`Session.scopes`); the last statement value is returned, and assignments inside functions never touch globals. Nested function definitions are rejected; recursion uses the existing 200-call depth guard. `{1, 2, 3}` after `=` is still parsed as a list via `brace_starts_block`.
- Tests live in `crates/frogcalc-core/tests/{numeric,session,features}.rs`; there are no unit tests inside `lib.rs`.
- App persistence path is `<exe 所在目录>\state.json` (portable; history, settings, workspace source, mode, script text, plot state). On first run it migrates `%APPDATA%\FrogCalc\state.json` and renames the old file to `state.json.bak`. Delete the local `state.json` to reset history/workspace/theme during manual testing.
- Three modes share one `Session`: `command` uses the single-line `input` + Enter; `script` uses the multiline `script` buffer + Ctrl+Enter, keeps the latest run in `last_script` (no history entries), and lists variables + the final result in the bottom results panel; `plot` re-samples the `plot` expression into `plot_data` (cached, recomputed on 绘制 / mode switch) and renders 2D or 3D. `Session::execute` already runs multi-statement text and stops at the first error.
- Script import/export uses an in-app file dialog (`FileDialog`) plus drag-and-drop; there is no native file-picker dependency. Accepted suffixes: `.frogcalc`, `.frog`, `.calc`, `.txt`. Shortcuts: Ctrl+O / Ctrl+S.
- Theme colors are global functions backed by `LIGHT_THEME` (`AtomicBool`). Keep using `bg()`, `panel()`, `text()`, etc.; avoid new hardcoded colors. Theme changes must call `install_style`, which also sets the global light flag.
- Help content is static in `help_content` in `crates/frogcalc-app/src/main.rs`; update it when adding syntax/functions.
- History timestamps are currently hardcoded UTC+8 in `time_now`.

## Conventions

- Comments and user-facing strings are Chinese; identifiers are English.
- Keep dependencies minimal. The core intentionally uses only `num-bigint`, `num-rational`, `num-traits`, `num-integer`; do not introduce a third-party CAS/math engine.
- If a dependency changes, update `Cargo.lock` and verify both `cargo test -p frogcalc-core` and `cargo check -p frogcalc-app`.
- UI changes should be verified with the actual native window; screenshots/positioning can differ under Windows DPI scaling.
