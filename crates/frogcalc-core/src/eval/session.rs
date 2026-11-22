use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use num_traits::{Signed, ToPrimitive, Zero};

use crate::calculus::{diff, integrate, integrate_definite};
use crate::error::{CalcError, CalcResult};
use crate::eval::approx::{approx_preview, to_approx_expr};
use crate::numeric::{rat, rat_exact_decimal, rat_to_f64};
use crate::parser::{parse, FunctionBody, Node, Statement};
use crate::solve::solve_equation;
use crate::symbolic::transform::{expand, factor, simplify};
use crate::symbolic::{
    add, call_expr, collect_symbols, div, eq_expr, from_complex, has_float, integer, is_constant,
    list, mul, neg, node_count, num, pow, sub, sym, to_plain, to_unicode, zero, Expr,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleMode {
    Rad,
    Deg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Exact,
    Both,
    Approx,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub precision: usize,
    pub angle_mode: AngleMode,
    pub display_mode: DisplayMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            precision: 50,
            angle_mode: AngleMode::Rad,
            display_mode: DisplayMode::Both,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: FunctionBody,
}

#[derive(Clone, Debug, Default)]
pub struct ExecResult {
    pub ok: bool,
    pub kind: String,
    pub name: Option<String>,
    pub plain: String,
    pub unicode: String,
    pub approx: Option<String>,
    pub approx_unicode: Option<String>,
    pub approx_exact: bool,
    pub value_type: String,
    pub note: Option<String>,
    pub error: Option<String>,
    pub error_pos: Option<usize>,
    pub elapsed_ms: f64,
    pub suppressed: bool,
}

#[derive(Clone, Debug)]
pub struct WorkspaceItem {
    pub name: String,
    pub kind: String,
    pub signature: Option<String>,
    pub value_type: String,
    pub plain: String,
    pub unicode: String,
}

const RESERVED: &[&str] = &[
    "pi",
    "e",
    "i",
    "ans",
    "inf",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "sinh",
    "cosh",
    "tanh",
    "exp",
    "ln",
    "log",
    "log10",
    "log2",
    "sqrt",
    "cbrt",
    "root",
    "pow",
    "abs",
    "sign",
    "floor",
    "ceil",
    "round",
    "factorial",
    "binomial",
    "gcd",
    "lcm",
    "mod",
    "min",
    "max",
    "eq",
    "diff",
    "integrate",
    "solve",
    "simplify",
    "expand",
    "factor",
    "n",
    "approx",
    "help",
    "clear",
    "vars",
    "subs",
    "substitute",
    "setprecision",
    "setangle",
    "true",
    "false",
    "lt",
    "le",
    "gt",
    "ge",
    "neq",
    "and",
    "or",
    "not",
    "range",
    "if",
    "else",
    "while",
    "for",
    "in",
];

fn is_reserved(name: &str) -> bool {
    RESERVED.iter().any(|n| *n == name.to_lowercase())
}

pub struct Session {
    pub settings: Settings,
    variables: BTreeMap<String, Expr>,
    functions: BTreeMap<String, UserFunction>,
    scopes: Vec<HashMap<String, Expr>>,
    ans: Expr,
    notes: Vec<String>,
    steps: usize,
    loop_steps: usize,
    call_depth: usize,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            variables: BTreeMap::new(),
            functions: BTreeMap::new(),
            scopes: Vec::new(),
            ans: zero(),
            notes: Vec::new(),
            steps: 0,
            loop_steps: 0,
            call_depth: 0,
        }
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = Settings {
            precision: settings.precision.clamp(10, 500),
            angle_mode: settings.angle_mode,
            display_mode: settings.display_mode,
        };
    }

    pub fn execute(&mut self, source: &str) -> Vec<ExecResult> {
        let program = match parse(source) {
            Ok(program) => program,
            Err(error) => return vec![self.error_result(error, 0.0)],
        };
        let mut results = Vec::new();
        for statement in program.statements {
            let started = Instant::now();
            self.notes.clear();
            self.steps = 0;
            self.loop_steps = 0;
            match self.eval_statement(&statement) {
                Ok(Some((kind, name, value, suppress))) => {
                    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                    if suppress {
                        results.push(ExecResult {
                            ok: true,
                            kind,
                            name,
                            elapsed_ms: elapsed,
                            suppressed: true,
                            ..Default::default()
                        });
                        continue;
                    }
                    let mut result = self.format_value(&value, &kind);
                    result.ok = true;
                    result.kind = kind;
                    result.name = name;
                    result.elapsed_ms = elapsed;
                    results.push(result);
                }
                Ok(None) => {}
                Err(error) => {
                    results
                        .push(self.error_result(error, started.elapsed().as_secs_f64() * 1000.0));
                    break;
                }
            }
        }
        results
    }

    fn eval_statement(
        &mut self,
        statement: &Statement,
    ) -> CalcResult<Option<(String, Option<String>, Expr, bool)>> {
        match statement {
            Statement::Expr(node, suppress) => {
                let value = self.eval_node(node, None, 0)?;
                self.check_size(&value)?;
                if self.scopes.is_empty() {
                    self.ans = value.clone();
                }
                Ok(Some(("expr".into(), None, value, *suppress)))
            }
            Statement::Assign(name, node, suppress) => {
                if is_reserved(name) {
                    return Err(CalcError::at(
                        format!("不能给保留名称 “{name}” 赋值"),
                        node.pos(),
                    ));
                }
                let value = self.eval_node(node, None, 0)?;
                self.check_size(&value)?;
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.clone(), value.clone());
                } else {
                    self.variables.insert(name.clone(), value.clone());
                    self.functions.remove(name);
                    self.ans = value.clone();
                }
                Ok(Some((
                    "assign".into(),
                    Some(name.clone()),
                    value,
                    *suppress,
                )))
            }
            Statement::FuncDef(name, params, body, suppress) => {
                if is_reserved(name) {
                    return Err(CalcError::at(
                        format!("不能重新定义保留名称 “{name}”"),
                        body_pos(body),
                    ));
                }
                if !self.scopes.is_empty() {
                    return Err(CalcError::at("不能在函数内部再定义函数", body_pos(body)));
                }
                let unique: BTreeSet<_> = params.iter().collect();
                if unique.len() != params.len() {
                    return Err(CalcError::at("函数参数名不能重复", body_pos(body)));
                }
                self.functions.insert(
                    name.clone(),
                    UserFunction {
                        name: name.clone(),
                        params: params.clone(),
                        body: body.clone(),
                    },
                );
                self.variables.remove(name);
                let display = match body {
                    FunctionBody::Expr(node) => eq_expr(
                        call_expr(name, params.iter().map(|p| sym(p)).collect()),
                        self.node_to_expr(node),
                    ),
                    FunctionBody::Block(_) => Expr::Str(format!(
                        "{}({}) = {}",
                        name,
                        params.join(", "),
                        self.function_body_source(body)
                    )),
                };
                Ok(Some((
                    "funcdef".into(),
                    Some(name.clone()),
                    display,
                    *suppress,
                )))
            }
            Statement::If { .. } | Statement::While { .. } | Statement::For { .. } => {
                self.eval_statement_value(statement)?;
                Ok(None)
            }
        }
    }

    fn eval_statement_value(&mut self, statement: &Statement) -> CalcResult<Expr> {
        match statement {
            Statement::If {
                cond,
                then_block,
                else_block,
                pos,
                ..
            } => {
                let value = self.eval_node(cond, None, 0)?;
                if self.truthy(&value, *pos)? {
                    self.eval_block_value(then_block)
                } else {
                    self.eval_block_value(else_block)
                }
            }
            Statement::While { cond, body, .. } => {
                let mut last = zero();
                loop {
                    self.tick_loop(cond.pos())?;
                    let value = self.eval_node(cond, None, 0)?;
                    if !self.truthy(&value, cond.pos())? {
                        break;
                    }
                    last = self.eval_block_value(body)?;
                }
                Ok(last)
            }
            Statement::For {
                var, iter, body, ..
            } => {
                let iterable = self.eval_node(iter, None, 0)?;
                let items = match iterable {
                    Expr::List(items) => items,
                    _ => {
                        return Err(CalcError::at(
                            "for 需要一个列表或 range(...)，例如 for i in range(1, 10)",
                            iter.pos(),
                        ))
                    }
                };
                let previous = self.var_scope_remove(var);
                let outcome = (|| -> CalcResult<Expr> {
                    let mut last = zero();
                    for item in items {
                        self.tick_loop(iter.pos())?;
                        self.var_scope_set(var, item);
                        last = self.eval_block_value(body)?;
                    }
                    Ok(last)
                })();
                match previous {
                    Some(value) => self.var_scope_set(var, value),
                    None => {
                        let _ = self.var_scope_remove(var);
                    }
                }
                outcome
            }
            _ => {
                let outcome = self.eval_statement(statement)?;
                Ok(outcome.map(|(_, _, value, _)| value).unwrap_or_else(zero))
            }
        }
    }

    fn eval_block_value(&mut self, statements: &[Statement]) -> CalcResult<Expr> {
        let mut last = zero();
        for statement in statements {
            last = self.eval_statement_value(statement)?;
        }
        Ok(last)
    }

    fn var_scope_set(&mut self, name: &str, value: Expr) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        } else {
            self.variables.insert(name.to_string(), value);
        }
    }

    fn var_scope_remove(&mut self, name: &str) -> Option<Expr> {
        if let Some(scope) = self.scopes.last_mut() {
            scope.remove(name)
        } else {
            self.variables.remove(name)
        }
    }

    fn tick_loop(&mut self, pos: usize) -> CalcResult<()> {
        self.loop_steps += 1;
        if self.loop_steps > 100_000 {
            return Err(CalcError::at(
                "循环执行次数过多，已中止（上限 100000 次）",
                pos,
            ));
        }
        Ok(())
    }

    fn truthy(&self, value: &Expr, pos: usize) -> CalcResult<bool> {
        match value {
            Expr::Num(v) => Ok(!v.is_zero()),
            Expr::Float(v) => Ok(v.to_f64() != 0.0),
            Expr::Sym(name) if name == "true" => Ok(true),
            Expr::Sym(name) if name == "false" => Ok(false),
            Expr::List(items) => Ok(!items.is_empty()),
            Expr::Eq(left, right) => Ok(values_equal(left, right)),
            _ => Err(CalcError::at(
                "条件需要一个可判断真假的表达式，例如 x > 0、a and b",
                pos,
            )),
        }
    }

    fn node_to_expr(&self, node: &Node) -> Expr {
        match node {
            Node::Number(v, _) => num(v.clone()),
            Node::String(v, _) => Expr::Str(v.clone()),
            Node::Ident(name, _) => sym(name),
            Node::Unary(op, arg, _) => {
                if *op == '-' {
                    neg(self.node_to_expr(arg))
                } else {
                    self.node_to_expr(arg)
                }
            }
            Node::Binary(op, left, right, _) => {
                let l = self.node_to_expr(left);
                let r = self.node_to_expr(right);
                match op {
                    '+' => add(vec![l, r]),
                    '-' => sub(l, r),
                    '*' => mul(vec![l, r]),
                    '/' => div(l, r),
                    '^' => pow(l, r),
                    _ => self.node_to_expr(left),
                }
            }
            Node::Call(name, args, _) => {
                call_expr(name, args.iter().map(|a| self.node_to_expr(a)).collect())
            }
            Node::Eq(l, r, _) => eq_expr(self.node_to_expr(l), self.node_to_expr(r)),
            Node::List(items, _) => {
                Expr::List(items.iter().map(|i| self.node_to_expr(i)).collect())
            }
        }
    }

    fn node_source(&self, node: &Node) -> String {
        if let Node::Call(name, args, _) = node {
            let op = match name.as_str() {
                "lt" => Some("<"),
                "le" => Some("<="),
                "gt" => Some(">"),
                "ge" => Some(">="),
                "neq" => Some("!="),
                "and" => Some("and"),
                "or" => Some("or"),
                _ => None,
            };
            if let (Some(op), 2) = (op, args.len()) {
                return format!(
                    "({} {op} {})",
                    self.node_source(&args[0]),
                    self.node_source(&args[1])
                );
            }
            if name == "not" && args.len() == 1 {
                return format!("not ({})", self.node_source(&args[0]));
            }
        }
        to_plain(&self.node_to_expr(node))
    }

    fn function_body_source(&self, body: &FunctionBody) -> String {
        match body {
            FunctionBody::Expr(node) => self.node_source(node),
            FunctionBody::Block(statements) => {
                if statements.is_empty() {
                    "{\n}".to_string()
                } else {
                    format!("{{\n{}\n}}", self.statements_source(statements, 1))
                }
            }
        }
    }

    fn statements_source(&self, statements: &[Statement], indent: usize) -> String {
        let pad = "    ".repeat(indent);
        let mut lines = Vec::new();
        for statement in statements {
            let text = match statement {
                Statement::Expr(node, _) => format!("{pad}{}", self.node_source(node)),
                Statement::Assign(name, node, _) => {
                    format!("{pad}{name} = {}", self.node_source(node))
                }
                Statement::FuncDef(name, params, body, _) => format!(
                    "{pad}{name}({}) = {}",
                    params.join(", "),
                    self.function_body_source(body)
                ),
                Statement::If {
                    cond,
                    then_block,
                    else_block,
                    ..
                } => {
                    let mut text = format!(
                        "{pad}if {} {{\n{}",
                        self.node_source(cond),
                        self.statements_source(then_block, indent + 1)
                    );
                    if else_block.is_empty() {
                        text.push_str(&format!("\n{pad}}}"));
                    } else {
                        text.push_str(&format!(
                            "\n{pad}}} else {{\n{}",
                            self.statements_source(else_block, indent + 1)
                        ));
                        text.push_str(&format!("\n{pad}}}"));
                    }
                    text
                }
                Statement::While { cond, body, .. } => format!(
                    "{pad}while {} {{\n{}\n{pad}}}",
                    self.node_source(cond),
                    self.statements_source(body, indent + 1)
                ),
                Statement::For {
                    var, iter, body, ..
                } => format!(
                    "{pad}for {var} in {} {{\n{}\n{pad}}}",
                    self.node_source(iter),
                    self.statements_source(body, indent + 1)
                ),
            };
            lines.push(text);
        }
        lines.join("\n")
    }

    fn eval_node(
        &mut self,
        node: &Node,
        locals: Option<&HashMap<String, Expr>>,
        depth: usize,
    ) -> CalcResult<Expr> {
        self.steps += 1;
        if self.steps > 500_000 {
            return Err(CalcError::at(
                "计算步骤过多，已中止（可能存在过大的展开或递归）",
                node.pos(),
            ));
        }
        if depth > 400 {
            return Err(CalcError::at("表达式嵌套过深", node.pos()));
        }
        match node {
            Node::Number(v, _) => Ok(num(v.clone())),
            Node::String(v, _) => Ok(Expr::Str(v.clone())),
            Node::Ident(name, _) => {
                if let Some(locals) = locals {
                    if let Some(value) = locals.get(name) {
                        return Ok(value.clone());
                    }
                }
                for scope in self.scopes.iter().rev() {
                    if let Some(value) = scope.get(name) {
                        return Ok(value.clone());
                    }
                }
                if name == "ans" {
                    return Ok(self.ans.clone());
                }
                if let Some(value) = self.variables.get(name) {
                    return Ok(value.clone());
                }
                if matches!(name.as_str(), "pi" | "e" | "i") {
                    return Ok(sym(name));
                }
                Ok(sym(name))
            }
            Node::Unary(op, arg, _) => {
                let value = self.eval_node(arg, locals, depth + 1)?;
                if *op == '-' {
                    Ok(neg(value))
                } else {
                    Ok(value)
                }
            }
            Node::Binary(op, left, right, pos) => {
                let l = self.eval_node(left, locals, depth + 1)?;
                let r = self.eval_node(right, locals, depth + 1)?;
                match op {
                    '+' => Ok(add(vec![l, r])),
                    '-' => Ok(sub(l, r)),
                    '*' => {
                        if let (Some(a), Some(b)) = (
                            crate::symbolic::to_exact_complex(&l),
                            crate::symbolic::to_exact_complex(&r),
                        ) {
                            return Ok(from_complex(&crate::symbolic::multiply_complex(&a, &b)));
                        }
                        Ok(mul(vec![l, r]))
                    }
                    '/' => {
                        if matches!(&r, Expr::Num(v) if v.is_zero()) {
                            return Err(CalcError::at("除数不能为零", *pos));
                        }
                        if let (Some(a), Some(b)) = (
                            crate::symbolic::to_exact_complex(&l),
                            crate::symbolic::to_exact_complex(&r),
                        ) {
                            if let Some(value) = crate::symbolic::divide_complex(&a, &b) {
                                return Ok(from_complex(&value));
                            }
                        }
                        Ok(div(l, r))
                    }
                    '^' => {
                        if let Expr::Num(v) = &r {
                            if v.is_integer() {
                                if let Some(n) = v.numer().to_i64() {
                                    if let Some(a) = crate::symbolic::to_exact_complex(&l) {
                                        if let Some(value) = crate::symbolic::pow_complex(&a, n) {
                                            return Ok(from_complex(&value));
                                        }
                                    }
                                }
                            }
                        }
                        Ok(pow(l, r))
                    }
                    _ => Err(CalcError::at("未知运算符", *pos)),
                }
            }
            Node::Eq(l, r, _) => Ok(eq_expr(
                self.eval_node(l, locals, depth + 1)?,
                self.eval_node(r, locals, depth + 1)?,
            )),
            Node::List(items, _) => {
                let values: CalcResult<Vec<Expr>> = items
                    .iter()
                    .map(|i| self.eval_node(i, locals, depth + 1))
                    .collect();
                Ok(list(values?))
            }
            Node::Call(name, args, pos) => match name.as_str() {
                "and" if args.len() == 2 => {
                    let left = self.eval_node(&args[0], locals, depth + 1)?;
                    if !self.truthy(&left, *pos)? {
                        return Ok(bool_expr(false));
                    }
                    let right = self.eval_node(&args[1], locals, depth + 1)?;
                    Ok(bool_expr(self.truthy(&right, *pos)?))
                }
                "or" if args.len() == 2 => {
                    let left = self.eval_node(&args[0], locals, depth + 1)?;
                    if self.truthy(&left, *pos)? {
                        return Ok(bool_expr(true));
                    }
                    let right = self.eval_node(&args[1], locals, depth + 1)?;
                    Ok(bool_expr(self.truthy(&right, *pos)?))
                }
                "not" if args.len() == 1 => {
                    let value = self.eval_node(&args[0], locals, depth + 1)?;
                    Ok(bool_expr(!self.truthy(&value, *pos)?))
                }
                _ => self.eval_call(name, args, *pos, locals, depth),
            },
        }
    }

    fn eval_call(
        &mut self,
        name: &str,
        args: &[Node],
        pos: usize,
        locals: Option<&HashMap<String, Expr>>,
        depth: usize,
    ) -> CalcResult<Expr> {
        let lower = name.to_lowercase();
        match lower.as_str() {
            "lt" | "le" | "gt" | "ge" | "neq" => {
                if args.len() != 2 {
                    return Err(CalcError::at(
                        format!("{lower} 需要 2 个参数，例如 {lower}(a, b)"),
                        pos,
                    ));
                }
                let left = self.eval_node(&args[0], locals, depth + 1)?;
                let right = self.eval_node(&args[1], locals, depth + 1)?;
                let value = if lower == "neq" {
                    !values_equal(&left, &right)
                } else {
                    let ordering = compare_expr(&left, &right, pos)?;
                    match lower.as_str() {
                        "lt" => ordering == Ordering::Less,
                        "le" => ordering != Ordering::Greater,
                        "gt" => ordering == Ordering::Greater,
                        "ge" => ordering != Ordering::Less,
                        _ => unreachable!(),
                    }
                };
                Ok(bool_expr(value))
            }
            "range" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(CalcError::at(
                        "range 需要 2 到 3 个参数：range(起点, 终点[, 步长])",
                        pos,
                    ));
                }
                let mut bounds = Vec::with_capacity(args.len());
                for arg in args {
                    bounds.push(self.eval_node(arg, locals, depth + 1)?);
                }
                let start = int_expr(&bounds[0], pos)?;
                let end = int_expr(&bounds[1], pos)?;
                let step = if bounds.len() == 3 {
                    int_expr(&bounds[2], pos)?
                } else {
                    1
                };
                if step == 0 {
                    return Err(CalcError::at("range 的步长不能为 0", pos));
                }
                let count = (end.saturating_sub(start) / step)
                    .saturating_abs()
                    .saturating_add(1);
                if count > 100_000 {
                    return Err(CalcError::at("range 生成的列表过大（上限 100000 项）", pos));
                }
                let mut items = Vec::new();
                let mut value = start;
                loop {
                    if (step > 0 && value > end) || (step < 0 && value < end) {
                        break;
                    }
                    items.push(integer(value));
                    match value.checked_add(step) {
                        Some(next) => value = next,
                        None => break,
                    }
                }
                Ok(list(items))
            }
            "diff" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(CalcError::at(
                        "diff 需要 2 到 3 个参数：diff(表达式, 变量[, 阶数])",
                        pos,
                    ));
                }
                let expr = self.eval_node(&args[0], locals, depth + 1)?;
                let var = self.resolve_var_name(&args[1])?;
                let order = if args.len() == 3 {
                    let value = self.eval_node(&args[2], locals, depth + 1)?;
                    match value {
                        Expr::Num(v) if v.is_integer() && v.is_positive() => {
                            v.numer().to_usize().unwrap_or(1)
                        }
                        _ => return Err(CalcError::at("求导阶数必须是非负整数", pos)),
                    }
                } else {
                    1
                };
                diff(&expr, &var, order).map_err(|e| with_pos(e, pos))
            }
            "integrate" => {
                if args.len() < 2 || args.len() > 4 {
                    return Err(CalcError::at(
                        "integrate 需要 2 到 4 个参数：integrate(表达式, 变量[, 下限, 上限])",
                        pos,
                    ));
                }
                let expr = self.eval_node(&args[0], locals, depth + 1)?;
                let var = self.resolve_var_name(&args[1])?;
                if args.len() == 2 {
                    let result = integrate(&expr, &var);
                    if contains_call(&result, "integrate") {
                        self.notes
                            .push("未能找到初等原函数，已保留未求值形式".into());
                    } else {
                        self.notes.push("不定积分（省略常数 C）".into());
                    }
                    Ok(result)
                } else if args.len() == 4 {
                    let a = self.eval_node(&args[2], locals, depth + 1)?;
                    let b = self.eval_node(&args[3], locals, depth + 1)?;
                    let result = integrate_definite(&expr, &var, &a, &b);
                    if contains_call(&result, "integrate") {
                        self.notes.push("定积分未能求出，已保留未求值形式".into());
                    } else {
                        self.notes.push("定积分已代入上下限".into());
                    }
                    Ok(result)
                } else {
                    Err(CalcError::at("integrate 的下限和上限必须同时给出", pos))
                }
            }
            "solve" => {
                if args.len() != 2 {
                    return Err(CalcError::at("solve 需要 2 个参数：solve(方程, 变量)", pos));
                }
                let expr = self.eval_node(&args[0], locals, depth + 1)?;
                let var = self.resolve_var_name(&args[1])?;
                let result = solve_equation(&expr, &var);
                if result.unsupported {
                    self.notes.push("未能解析求解，已保留未求值形式".into());
                    return Ok(call_expr("solve", vec![expr, sym(&var)]));
                }
                if let Some(note) = result.note {
                    self.notes.push(note);
                }
                if result.solutions.is_empty() {
                    self.notes.push("没有找到解".into());
                }
                Ok(list(result.solutions))
            }
            "simplify" | "expand" | "factor" => {
                if args.len() != 1 {
                    return Err(CalcError::at(format!("{name} 需要 1 个参数"), pos));
                }
                let value = self.eval_node(&args[0], locals, depth + 1)?;
                match lower.as_str() {
                    "simplify" => Ok(simplify(&value)),
                    "expand" => expand(&value).map_err(|e| with_pos(e, pos)),
                    _ => Ok(factor(&value)),
                }
            }
            "n" | "approx" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(CalcError::at("N 需要 1 到 2 个参数", pos));
                }
                let value = self.eval_node(&args[0], locals, depth + 1)?;
                let digits = if args.len() == 2 {
                    let d = self.eval_node(&args[1], locals, depth + 1)?;
                    match d {
                        Expr::Num(v) if v.is_integer() => {
                            let n = v.numer().to_usize().unwrap_or(0);
                            if !(10..=500).contains(&n) {
                                return Err(CalcError::at("精度位数应在 10 到 500 之间", pos));
                            }
                            n
                        }
                        _ => return Err(CalcError::at("N 的第二个参数必须是整数位数", pos)),
                    }
                } else {
                    self.settings.precision
                };
                to_approx_expr(&value, digits).map_err(|e| with_pos(e, pos))
            }
            "help" => {
                if args.len() != 1 {
                    return Err(CalcError::at("help 需要 1 个参数，例如 help(\"sin\")", pos));
                }
                let name = match &args[0] {
                    Node::String(value, _) => value.clone(),
                    Node::Ident(value, _) => value.clone(),
                    _ => {
                        let value = self.eval_node(&args[0], locals, depth + 1)?;
                        match value {
                            Expr::Sym(name) => name,
                            Expr::Str(value) => value,
                            _ => String::new(),
                        }
                    }
                };
                Ok(Expr::Str(self.help_text(&name)))
            }
            "clear" => {
                if args.is_empty() {
                    self.clear_all();
                    return Ok(Expr::Str("工作区已清空".into()));
                }
                let mut names = Vec::new();
                for arg in args {
                    match arg {
                        Node::Ident(name, _) => names.push(name.clone()),
                        Node::String(name, _) => names.push(name.clone()),
                        _ => {
                            let value = self.eval_node(arg, locals, depth + 1)?;
                            match value {
                                Expr::Sym(name) => names.push(name),
                                Expr::Str(name) => names.push(name),
                                _ => {
                                    return Err(CalcError::at(
                                        "clear 的参数应为变量名或字符串",
                                        pos,
                                    ))
                                }
                            }
                        }
                    }
                }
                for name in &names {
                    self.variables.remove(name);
                    self.functions.remove(name);
                }
                Ok(Expr::Str(format!("已清除：{}", names.join(", "))))
            }
            "vars" => Ok(Expr::Str(self.workspace_summary())),
            "subs" | "substitute" => {
                if args.len() != 3 {
                    return Err(CalcError::at(
                        "subs 需要 3 个参数：subs(表达式, 变量, 值)",
                        pos,
                    ));
                }
                let target = self.eval_node(&args[0], locals, depth + 1)?;
                let var = self.resolve_var_name(&args[1])?;
                let value = self.eval_node(&args[2], locals, depth + 1)?;
                Ok(crate::symbolic::substitute(&target, &var, &value))
            }
            "setprecision" => {
                if args.len() != 1 {
                    return Err(CalcError::at("setprecision 需要 1 个参数", pos));
                }
                let value = self.eval_node(&args[0], locals, depth + 1)?;
                if let Expr::Num(v) = value {
                    if v.is_integer() {
                        if let Some(n) = v.numer().to_usize() {
                            if (10..=500).contains(&n) {
                                self.settings.precision = n;
                                return Ok(Expr::Str(format!("精度已设置为 {n} 位有效数字")));
                            }
                        }
                    }
                }
                Err(CalcError::at("setprecision 需要一个 10 到 500 的整数", pos))
            }
            "setangle" => {
                if args.len() != 1 {
                    return Err(CalcError::at("setangle 需要 1 个参数", pos));
                }
                let value = self.eval_node(&args[0], locals, depth + 1)?;
                let text = match value {
                    Expr::Str(v) => v.to_lowercase(),
                    _ => String::new(),
                };
                match text.as_str() {
                    "deg" => {
                        self.settings.angle_mode = AngleMode::Deg;
                        Ok(Expr::Str("角度制已启用".into()))
                    }
                    "rad" => {
                        self.settings.angle_mode = AngleMode::Rad;
                        Ok(Expr::Str("弧度制已启用".into()))
                    }
                    _ => Err(CalcError::at("setangle 的参数应为 \"deg\" 或 \"rad\"", pos)),
                }
            }
            _ => {
                if let Some(user_fn) = self.functions.get(name).cloned() {
                    if user_fn.params.len() != args.len() {
                        return Err(CalcError::at(
                            format!(
                                "函数 {name} 需要 {} 个参数，实际收到 {} 个",
                                user_fn.params.len(),
                                args.len()
                            ),
                            pos,
                        ));
                    }
                    if self.call_depth >= 200 {
                        return Err(CalcError::at("函数调用层数过深（可能存在无限递归）", pos));
                    }
                    let values: CalcResult<Vec<Expr>> = args
                        .iter()
                        .map(|a| self.eval_node(a, locals, depth + 1))
                        .collect();
                    let values = values?;
                    let mut frame = HashMap::new();
                    for (param, value) in user_fn.params.iter().zip(values) {
                        frame.insert(param.clone(), value);
                    }
                    self.call_depth += 1;
                    let result = match &user_fn.body {
                        FunctionBody::Expr(node) => self.eval_node(node, Some(&frame), depth + 1),
                        FunctionBody::Block(statements) => {
                            self.scopes.push(frame);
                            let result = self.eval_block_value(statements);
                            self.scopes.pop();
                            result
                        }
                    };
                    self.call_depth -= 1;
                    return result;
                }
                let values: CalcResult<Vec<Expr>> = args
                    .iter()
                    .map(|a| self.eval_node(a, locals, depth + 1))
                    .collect();
                let values = values?;
                if self.variables.contains_key(name) && values.len() == 1 {
                    return Ok(mul(vec![
                        self.variables.get(name).cloned().unwrap_or_else(zero),
                        values[0].clone(),
                    ]));
                }
                let values = if matches!(lower.as_str(), "sin" | "cos" | "tan") {
                    values
                        .into_iter()
                        .map(|v| {
                            if self.settings.angle_mode == AngleMode::Deg {
                                self.deg_to_rad(v)
                            } else {
                                v
                            }
                        })
                        .collect()
                } else {
                    values
                };
                let result = call_expr(&lower, values);
                let result = if matches!(lower.as_str(), "asin" | "acos" | "atan")
                    && self.settings.angle_mode == AngleMode::Deg
                {
                    self.rad_to_deg(result)
                } else {
                    result
                };
                Ok(result)
            }
        }
    }

    fn resolve_var_name(&self, node: &Node) -> CalcResult<String> {
        match node {
            Node::Ident(name, _) => Ok(name.clone()),
            Node::String(name, _) => Ok(name.clone()),
            _ => Err(CalcError::at("这里需要变量名，例如 x", node.pos())),
        }
    }

    fn deg_to_rad(&self, value: Expr) -> Expr {
        mul(vec![value, num(rat(1, 180)), sym("pi")])
    }

    fn rad_to_deg(&self, value: Expr) -> Expr {
        mul(vec![value, num(rat(180, 1)), pow(sym("pi"), integer(-1))])
    }

    fn format_value(&self, value: &Expr, kind: &str) -> ExecResult {
        let plain = match value {
            Expr::Str(text) => text.clone(),
            _ => to_plain(value),
        };
        let unicode = match value {
            Expr::Str(text) => text.clone(),
            _ => to_unicode(value),
        };
        let mut approx = None;
        let mut approx_unicode = None;
        let mut approx_exact = false;
        if self.settings.display_mode != DisplayMode::Exact
            && !matches!(value, Expr::List(_) | Expr::Eq(_, _) | Expr::Str(_))
            && !has_float(value)
        {
            let preview_precision = if self.settings.display_mode == DisplayMode::Approx {
                self.settings.precision
            } else {
                self.settings.precision.min(20)
            };
            if let Some(preview) = approx_preview(value, preview_precision) {
                if preview != plain {
                    approx_exact = exact_decimal_preview(value, preview_precision).as_deref()
                        == Some(preview.as_str());
                    approx = Some(preview);
                    if let Ok(expr) = to_approx_expr(value, preview_precision) {
                        approx_unicode = Some(to_unicode(&expr));
                    }
                }
            }
        }
        ExecResult {
            ok: true,
            kind: kind.to_string(),
            name: None,
            plain,
            unicode,
            approx,
            approx_unicode,
            approx_exact,
            value_type: if kind == "funcdef" {
                "函数定义".into()
            } else {
                type_label(value)
            },
            note: if self.notes.is_empty() {
                None
            } else {
                Some(self.notes.join("；"))
            },
            error: None,
            error_pos: None,
            elapsed_ms: 0.0,
            suppressed: false,
        }
    }

    fn check_size(&self, value: &Expr) -> CalcResult<()> {
        if node_count(value) > 300_000 {
            Err(CalcError::new("结果表达式过大，已中止"))
        } else {
            Ok(())
        }
    }

    pub fn workspace(&self) -> Vec<WorkspaceItem> {
        let mut items = Vec::new();
        for (name, value) in &self.variables {
            items.push(WorkspaceItem {
                name: name.clone(),
                kind: "variable".into(),
                signature: None,
                value_type: type_label(value),
                plain: to_plain(value),
                unicode: to_unicode(value),
            });
        }
        for (name, function) in &self.functions {
            items.push(WorkspaceItem {
                name: name.clone(),
                kind: "function".into(),
                signature: Some(format!("{name}({})", function.params.join(", "))),
                value_type: "函数".into(),
                plain: format!("{name}({})", function.params.join(", ")),
                unicode: format!("{name}({})", function.params.join(", ")),
            });
        }
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    }

    pub fn workspace_source(&self) -> String {
        let mut lines = Vec::new();
        for (name, value) in &self.variables {
            match value {
                Expr::Eq(lhs, rhs) => {
                    lines.push(format!("{name} = eq({}, {})", to_plain(lhs), to_plain(rhs)))
                }
                other => lines.push(format!("{name} = {}", to_plain(other))),
            }
        }
        for (name, function) in &self.functions {
            lines.push(format!(
                "{name}({}) = {}",
                function.params.join(", "),
                self.function_body_source(&function.body)
            ));
        }
        lines.join("\n")
    }

    pub fn remove_workspace_item(&mut self, name: &str) {
        self.variables.remove(name);
        self.functions.remove(name);
    }

    pub fn clear_all(&mut self) {
        self.variables.clear();
        self.functions.clear();
        self.ans = zero();
    }

    fn workspace_summary(&self) -> String {
        let mut lines = Vec::new();
        for (name, value) in &self.variables {
            lines.push(format!("{name} = {}", to_plain(value)));
        }
        for (name, function) in &self.functions {
            lines.push(format!("{name}({}) = 用户函数", function.params.join(", ")));
        }
        if lines.is_empty() {
            "工作区为空".into()
        } else {
            lines.join("\n")
        }
    }

    fn help_text(&self, name: &str) -> String {
        let key = name.to_lowercase();
        match key.as_str() {
            "syntax" => [
                "FrogCalc 语言：",
                "  a = 1 / 3           赋值，除以整数得到精确分数",
                "  f(x) = x^2 + 1      定义函数",
                "  2x + 3(x - 1)       支持隐式乘法",
                "  sqrt(2) * sqrt(2)   精确化简为 2",
                "  diff / integrate / solve / simplify / expand / factor",
                "  N(表达式, 位数)      近似求值",
                "  help(\"函数名\")       查看帮助",
                "  # 这是注释",
                "  分号 ; 可分隔语句并抑制输出",
            ]
            .join("\n"),
            "diff" => "diff(表达式, 变量[, 阶数])：求导。例如 diff(x^3*sin(x), x)".into(),
            "integrate" => "integrate(表达式, 变量[, 下限, 上限])：积分。\n例如 integrate(x*exp(x), x) 或 integrate(1/(x^2-1), x)。\n找不到初等原函数时保留未求值形式。".into(),
            "solve" => "solve(方程或表达式, 变量)：解方程。\n例如 solve(x^2 - 2x - 3, x) 或 solve(sin(x) == 0, x)。".into(),
            "simplify" => "simplify(表达式)：化简、通分、约去公因子。".into(),
            "expand" => "expand(表达式)：展开多项式与乘积。".into(),
            "factor" => "factor(表达式)：对一元多项式做因式分解。".into(),
            "sqrt" => "sqrt(x)：平方根。对完全平方数自动精确化简。".into(),
            "exp" => "exp(x)：自然指数函数。".into(),
            "ln" => "ln(x)：自然对数。".into(),
            "log" => "log(x) 为自然对数；log(x, b) 为以 b 为底的对数。".into(),
            "sin" => "sin(x)：正弦。默认弧度制，可用 setangle(\"deg\") 切换。".into(),
            "cos" => "cos(x)：余弦。".into(),
            "tan" => "tan(x)：正切。".into(),
            "abs" => "abs(x)：绝对值。".into(),
            "factorial" => "factorial(n) 或 n!：阶乘。".into(),
            "N" => "N(表达式[, 位数])：按指定位数做近似求值。".into(),
            "help" => "help(\"名称\")：显示帮助。".into(),
            "clear" => "clear() 清空工作区；clear(\"x\") 删除变量 x。".into(),
            "vars" => "vars()：列出工作区变量与函数。".into(),
            "subs" => "subs(表达式, 变量, 值)：符号替换。".into(),
            "setprecision" => "setprecision(n)：设置近似计算的精度位数。".into(),
            "setangle" => "setangle(\"deg\") 或 setangle(\"rad\")：切换角度制。".into(),
            _ => format!("没有找到 “{name}” 的帮助。试试 help(\"syntax\")。"),
        }
    }

    fn error_result(&self, error: CalcError, elapsed_ms: f64) -> ExecResult {
        ExecResult {
            ok: false,
            kind: "error".into(),
            error: Some(error.message),
            error_pos: error.pos,
            elapsed_ms,
            ..Default::default()
        }
    }
}

fn with_pos(error: CalcError, pos: usize) -> CalcError {
    if error.pos.is_some() {
        error
    } else {
        CalcError::at(error.message, pos)
    }
}

fn contains_call(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Call(n, args) => {
            n.to_lowercase() == name || args.iter().any(|a| contains_call(a, name))
        }
        Expr::Add(terms) => terms.iter().any(|t| contains_call(t, name)),
        Expr::Mul(factors) => factors.iter().any(|f| contains_call(f, name)),
        Expr::Pow(b, e) => contains_call(b, name) || contains_call(e, name),
        Expr::Eq(l, r) => contains_call(l, name) || contains_call(r, name),
        Expr::List(items) => items.iter().any(|i| contains_call(i, name)),
        _ => false,
    }
}

fn exact_decimal_preview(expr: &Expr, max_digits: usize) -> Option<String> {
    let complex = crate::symbolic::to_exact_complex(expr)?;
    let re = rat_exact_decimal(&complex.re, max_digits)?;
    let im = rat_exact_decimal(&complex.im, max_digits)?;
    if im == "0" {
        return Some(re);
    }
    let negative = im.starts_with('-');
    let im_abs = if negative { &im[1..] } else { im.as_str() };
    let im_text = if im_abs == "1" {
        "i".to_string()
    } else {
        format!("{im_abs}i")
    };
    if re == "0" {
        Some(if negative {
            format!("-{im_text}")
        } else {
            im_text
        })
    } else {
        Some(format!(
            "{re} {} {im_text}",
            if negative { "-" } else { "+" }
        ))
    }
}

fn body_pos(body: &FunctionBody) -> usize {
    match body {
        FunctionBody::Expr(node) => node.pos(),
        FunctionBody::Block(statements) => statements.first().map(statement_pos).unwrap_or(0),
    }
}

fn statement_pos(statement: &Statement) -> usize {
    match statement {
        Statement::Expr(node, _) | Statement::Assign(_, node, _) => node.pos(),
        Statement::FuncDef(_, _, body, _) => body_pos(body),
        Statement::If { pos, .. } | Statement::While { pos, .. } | Statement::For { pos, .. } => {
            *pos
        }
    }
}

fn bool_expr(value: bool) -> Expr {
    sym(if value { "true" } else { "false" })
}

fn values_equal(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Num(a), Expr::Num(b)) => a == b,
        (Expr::Float(a), Expr::Float(b)) => a.to_f64() == b.to_f64(),
        (Expr::Num(a), Expr::Float(b)) | (Expr::Float(b), Expr::Num(a)) => {
            rat_to_f64(a) == b.to_f64()
        }
        _ => crate::symbolic::equals(left, right),
    }
}

fn compare_expr(left: &Expr, right: &Expr, pos: usize) -> CalcResult<Ordering> {
    let float_cmp = |a: f64, b: f64| {
        a.partial_cmp(&b)
            .ok_or_else(|| CalcError::at("无法比较这两个数", pos))
    };
    match (left, right) {
        (Expr::Num(a), Expr::Num(b)) => Ok(a.cmp(b)),
        (Expr::Float(a), Expr::Float(b)) => float_cmp(a.to_f64(), b.to_f64()),
        (Expr::Num(a), Expr::Float(b)) => float_cmp(rat_to_f64(a), b.to_f64()),
        (Expr::Float(a), Expr::Num(b)) => float_cmp(a.to_f64(), rat_to_f64(b)),
        _ => Err(CalcError::at("无法比较含符号的表达式（需要具体数值）", pos)),
    }
}

fn int_expr(value: &Expr, pos: usize) -> CalcResult<i64> {
    match value {
        Expr::Num(v) if v.is_integer() => v
            .numer()
            .to_i64()
            .ok_or_else(|| CalcError::at("range 的边界必须是普通整数", pos)),
        _ => Err(CalcError::at("range 的边界和步长必须是整数", pos)),
    }
}

fn type_label(value: &Expr) -> String {
    match value {
        Expr::Sym(name) if name == "true" || name == "false" => "布尔值".into(),
        Expr::Str(_) => "文本".into(),
        Expr::List(_) => "解集".into(),
        Expr::Eq(_, _) => "方程".into(),
        Expr::Float(_) => "实数".into(),
        Expr::Num(v) => {
            if v.is_integer() {
                "整数".into()
            } else {
                "有理数".into()
            }
        }
        _ => {
            let mut symbols = Vec::new();
            collect_symbols(value, &mut symbols);
            if symbols.iter().any(|s| s == "i") {
                "复数".into()
            } else if is_constant(value) {
                "精确常数".into()
            } else if has_float(value) {
                "实数".into()
            } else {
                "符号式".into()
            }
        }
    }
}

// 保持模块入口简洁
