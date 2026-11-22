//! 2D / 3D 函数采样，供界面绘图使用。
//!
//! 采样复用 `Session::eval_sample`，因此用户函数、变量、分段函数和角度制设置都生效。

use crate::error::{CalcError, CalcResult};
use crate::eval::session::Session;
use crate::numeric::rat_to_f64;
use crate::parser::{parse, FunctionBody, Node, Statement};
use crate::symbolic::Expr;

pub const MAX_2D_SAMPLES: usize = 4000;
pub const MAX_3D_SIDE: usize = 80;

/// 绘图表达式类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    /// 2D 显式曲线 `y = f(x)`。
    Explicit2D,
    /// 3D 显式曲面 `z = f(x, y)`。
    Explicit3D,
    /// 2D 隐函数曲线 `F(x, y) = 0`。
    Implicit2D,
    /// 3D 隐函数曲面 `F(x, y, z) = 0`。
    Implicit3D,
}

impl PlotKind {
    pub fn is_implicit(self) -> bool {
        matches!(self, PlotKind::Implicit2D | PlotKind::Implicit3D)
    }
}

/// 编译后的绘图目标：`node` 对隐函数是 `lhs - rhs`（零等值面），对显式函数是右侧表达式。
pub struct PlotTarget {
    pub kind: PlotKind,
    pub node: Node,
}

/// 解析绘图输入。
///
/// - `y = f(x)` / `z = f(x, y)` 视为显式函数；
/// - `lhs = rhs`（单个等号）与 `lhs == rhs` 视为隐方程 `lhs - rhs = 0`；
/// - 2D 下含 `y` 的表达式、3D 下含 `z` 的表达式视为隐函数。
pub fn compile_plot(source: &str, three_d: bool) -> CalcResult<PlotTarget> {
    let text = source.trim();
    if text.is_empty() {
        return Err(CalcError::new("绘图需要一个表达式或方程"));
    }
    let (node, equation, forced_explicit) = parse_plot_node(text)?;
    let mut symbols = Vec::new();
    collect_node_symbols(&node, &mut symbols);
    let has_y = symbols.iter().any(|name| name == "y");
    let has_z = symbols.iter().any(|name| name == "z");
    if three_d {
        let implicit = equation || (has_z && !forced_explicit);
        Ok(PlotTarget {
            kind: if implicit {
                PlotKind::Implicit3D
            } else {
                PlotKind::Explicit3D
            },
            node,
        })
    } else {
        if has_z {
            return Err(CalcError::new("表达式包含 z，请切换到 3D 模式绘制"));
        }
        if forced_explicit && has_y {
            return Err(CalcError::new(
                "表达式包含 y，请切换到 3D 模式绘制，或改写成 x、y 的隐方程",
            ));
        }
        let implicit = equation || (has_y && !forced_explicit);
        Ok(PlotTarget {
            kind: if implicit {
                PlotKind::Implicit2D
            } else {
                PlotKind::Explicit2D
            },
            node,
        })
    }
}

fn parse_plot_node(text: &str) -> CalcResult<(Node, bool, bool)> {
    match parse(text) {
        Ok(program) => {
            let mut statements = program.statements.into_iter();
            let statement = statements
                .next()
                .ok_or_else(|| CalcError::new("绘图需要一个表达式或方程"))?;
            if statements.next().is_some() {
                return Err(CalcError::new("绘图只接受一个表达式或方程"));
            }
            match statement {
                Statement::Expr(Node::Eq(lhs, rhs, pos), _) => {
                    Ok((Node::Binary('-', lhs, rhs, pos), true, false))
                }
                Statement::Expr(node, _) => Ok((node, false, false)),
                Statement::Assign(_, node, _) => Ok((node, false, true)),
                Statement::FuncDef(name, _, _, _) => Err(CalcError::new(format!(
                    "绘图请直接写表达式；已定义的函数 {name} 可以直接调用，例如 {name}(x)"
                ))),
                other => Err(CalcError::at(
                    "绘图需要一个表达式或方程",
                    statement_position(&other),
                )),
            }
        }
        Err(parse_error) => {
            // `lhs = rhs` 不是合法语句，先把单个等号拆成两侧表达式。
            let Some((lhs_text, rhs_text)) = split_single_equals(text) else {
                return Err(parse_error);
            };
            let lhs = parse_single_expression(lhs_text, "方程左侧")?;
            let rhs = parse_single_expression(rhs_text, "方程右侧")?;
            Ok((
                Node::Binary('-', Box::new(lhs), Box::new(rhs), 0),
                true,
                false,
            ))
        }
    }
}

fn parse_single_expression(text: &str, side: &str) -> CalcResult<Node> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CalcError::new(format!("{side}不能为空")));
    }
    let program = parse(text)?;
    let mut statements = program.statements.into_iter();
    let statement = statements
        .next()
        .ok_or_else(|| CalcError::new(format!("{side}不能为空")))?;
    if statements.next().is_some() {
        return Err(CalcError::new(format!("{side}只能有一个表达式")));
    }
    match statement {
        Statement::Expr(node, _) => Ok(node),
        Statement::Assign(_, node, _) => Ok(node),
        other => Err(CalcError::at(
            format!("{side}需要一个表达式"),
            statement_position(&other),
        )),
    }
}

/// 找出不在比较运算符/字符串里的单个 `=`，返回左右两段文本。
fn split_single_equals(text: &str) -> Option<(&str, &str)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut found = None;
    for (index, (byte, ch)) in chars.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *ch == '\\' {
                escaped = true;
            } else if *ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '=' if depth == 0 => {
                let previous = index.checked_sub(1).map(|i| chars[i].1).unwrap_or(' ');
                let next = chars.get(index + 1).map(|(_, c)| *c).unwrap_or(' ');
                let operator_char = matches!(previous, '=' | '<' | '>' | '!');
                if !operator_char && next != '=' {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(*byte);
                }
            }
            _ => {}
        }
    }
    found.map(|position| (&text[..position], &text[position + 1..]))
}

fn statement_position(statement: &Statement) -> usize {
    match statement {
        Statement::Expr(node, _) | Statement::Assign(_, node, _) => node.pos(),
        Statement::FuncDef(_, _, body, _) => match body {
            FunctionBody::Expr(node) => node.pos(),
            FunctionBody::Block(statements) => {
                statements.first().map(statement_position).unwrap_or(0)
            }
        },
        Statement::If { pos, .. }
        | Statement::While { pos, .. }
        | Statement::For { pos, .. }
        | Statement::Break { pos }
        | Statement::Continue { pos } => *pos,
    }
}

fn collect_node_symbols(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Ident(name, _) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Node::Unary(_, arg, _) => collect_node_symbols(arg, out),
        Node::Binary(_, left, right, _) => {
            collect_node_symbols(left, out);
            collect_node_symbols(right, out);
        }
        Node::Call(_, args, _) => {
            for arg in args {
                collect_node_symbols(arg, out);
            }
        }
        Node::Eq(left, right, _) => {
            collect_node_symbols(left, out);
            collect_node_symbols(right, out);
        }
        Node::List(items, _) => {
            for item in items {
                collect_node_symbols(item, out);
            }
        }
        _ => {}
    }
}

pub struct Sampled2D {
    pub points: Vec<Option<(f64, f64)>>,
}

pub struct SampledSurface {
    /// 行优先：第 `j` 行是 y = y_j 的一条曲线，含 `nx` 个点。
    pub values: Vec<Option<f64>>,
    pub nx: usize,
    pub ny: usize,
    pub y0: f64,
    pub y1: f64,
}

fn linspace(start: f64, end: f64, count: usize) -> Vec<f64> {
    if count <= 1 {
        return vec![start];
    }
    let step = (end - start) / (count - 1) as f64;
    (0..count).map(|i| start + step * i as f64).collect()
}

fn finite(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// 先用一次符号求值把用户函数/全局变量展开，再用轻量 f64 求值器采样。
fn compile_plot_expression(session: &mut Session, node: &Node) -> Option<Expr> {
    session.compile_plot_expr(node).ok()
}

fn eval_point(
    session: &mut Session,
    node: &Node,
    compiled: Option<&Expr>,
    bindings: &[(&str, f64)],
) -> Option<f64> {
    if let Some(expr) = compiled {
        if let Some(value) = eval_f64(expr, bindings).and_then(finite) {
            return Some(value);
        }
    }
    session.eval_sample(node, bindings).and_then(finite)
}

fn lookup(bindings: &[(&str, f64)], name: &str) -> Option<f64> {
    bindings
        .iter()
        .find_map(|(key, value)| (*key == name).then_some(*value))
}

/// 只支持实数的轻量表达式求值器；遇到不支持的结构返回 `None`。
pub fn eval_f64(expr: &Expr, bindings: &[(&str, f64)]) -> Option<f64> {
    match expr {
        Expr::Num(value) => Some(rat_to_f64(value)),
        Expr::Float(value) => Some(value.to_f64()),
        Expr::Sym(name) => match name.as_str() {
            "pi" => Some(std::f64::consts::PI),
            "e" => Some(std::f64::consts::E),
            _ => lookup(bindings, name),
        },
        Expr::Add(terms) => {
            let mut sum = 0.0;
            for term in terms {
                sum += eval_f64(term, bindings)?;
            }
            Some(sum)
        }
        Expr::Mul(factors) => {
            let mut product = 1.0;
            for factor in factors {
                product *= eval_f64(factor, bindings)?;
            }
            Some(product)
        }
        Expr::Pow(base, exponent) => {
            let base = eval_f64(base, bindings)?;
            let exponent = eval_f64(exponent, bindings)?;
            if base < 0.0 && exponent.fract() != 0.0 {
                return None;
            }
            Some(base.powf(exponent))
        }
        Expr::Call(name, args) => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(eval_f64(arg, bindings)?);
            }
            let first = values.first().copied().unwrap_or(f64::NAN);
            let second = values.get(1).copied().unwrap_or(f64::NAN);
            match name.to_lowercase().as_str() {
                "sin" => Some(first.sin()),
                "cos" => Some(first.cos()),
                "tan" => Some(first.tan()),
                "asin" => Some(first.asin()),
                "acos" => Some(first.acos()),
                "atan" => Some(first.atan()),
                "sinh" => Some(first.sinh()),
                "cosh" => Some(first.cosh()),
                "tanh" => Some(first.tanh()),
                "exp" => Some(first.exp()),
                "ln" => Some(first.ln()),
                "log10" => Some(first.log10()),
                "log2" => Some(first.log2()),
                "sqrt" => Some(first.sqrt()),
                "cbrt" => Some(first.cbrt()),
                "abs" => Some(first.abs()),
                "sign" => Some(first.signum()),
                "floor" => Some(first.floor()),
                "ceil" => Some(first.ceil()),
                "round" => Some(first.round()),
                "pow" => Some(first.powf(second)),
                "mod" => {
                    if second == 0.0 {
                        None
                    } else {
                        Some(first - second * (first / second).floor())
                    }
                }
                "min" => values.iter().copied().reduce(f64::min),
                "max" => values.iter().copied().reduce(f64::max),
                "lt" => Some(if first < second { 1.0 } else { 0.0 }),
                "le" => Some(if first <= second { 1.0 } else { 0.0 }),
                "gt" => Some(if first > second { 1.0 } else { 0.0 }),
                "ge" => Some(if first >= second { 1.0 } else { 0.0 }),
                "neq" => Some(if first != second { 1.0 } else { 0.0 }),
                "eq" => Some(if first == second { 1.0 } else { 0.0 }),
                "and" => Some(if first != 0.0 && second != 0.0 {
                    1.0
                } else {
                    0.0
                }),
                "or" => Some(if first != 0.0 || second != 0.0 {
                    1.0
                } else {
                    0.0
                }),
                "not" => Some(if first == 0.0 { 1.0 } else { 0.0 }),
                "piecewise" => {
                    let pairs = values.len() / 2;
                    for index in 0..pairs {
                        if values[2 * index] != 0.0 {
                            return Some(values[2 * index + 1]);
                        }
                    }
                    if values.len() % 2 == 1 {
                        values.last().copied()
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        Expr::Eq(lhs, rhs) => {
            let left = eval_f64(lhs, bindings)?;
            let right = eval_f64(rhs, bindings)?;
            Some(if (left - right).abs() <= 1e-12 {
                1.0
            } else {
                0.0
            })
        }
        _ => None,
    }
}

pub fn sample_2d(
    session: &mut Session,
    node: &Node,
    var: &str,
    x0: f64,
    x1: f64,
    samples: usize,
) -> Sampled2D {
    let count = samples.clamp(16, MAX_2D_SAMPLES);
    let (x0, x1) = if x0 == x1 {
        (x0 - 1.0, x1 + 1.0)
    } else {
        (x0, x1)
    };
    let compiled = compile_plot_expression(session, node);
    let points = linspace(x0, x1, count)
        .into_iter()
        .map(|x| eval_point(session, node, compiled.as_ref(), &[(var, x)]).map(|y| (x, y)))
        .collect();
    Sampled2D { points }
}

pub fn sample_surface(
    session: &mut Session,
    node: &Node,
    vx: &str,
    vy: &str,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    side: usize,
) -> SampledSurface {
    let count = side.clamp(8, MAX_3D_SIDE);
    let (x0, x1) = if x0 == x1 {
        (x0 - 1.0, x1 + 1.0)
    } else {
        (x0, x1)
    };
    let (y0, y1) = if y0 == y1 {
        (y0 - 1.0, y1 + 1.0)
    } else {
        (y0, y1)
    };
    let xs = linspace(x0, x1, count);
    let ys = linspace(y0, y1, count);
    let compiled = compile_plot_expression(session, node);
    let mut values = Vec::with_capacity(count * count);
    for y in &ys {
        for x in &xs {
            values.push(eval_point(
                session,
                node,
                compiled.as_ref(),
                &[(vx, *x), (vy, *y)],
            ));
        }
    }
    SampledSurface {
        values,
        nx: count,
        ny: count,
        y0,
        y1,
    }
}

/// 判断相邻采样点之间是否应断开（跳变或落在定义域外）。
pub fn should_break(previous: Option<f64>, current: Option<f64>, y_span: f64) -> bool {
    match (previous, current) {
        (Some(a), Some(b)) => {
            if !a.is_finite() || !b.is_finite() {
                return true;
            }
            let span = y_span.abs().max(1e-9);
            (b - a).abs() > span * 2.0
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// 隐函数：F(x, y) = 0 / F(x, y, z) = 0
// ---------------------------------------------------------------------------

pub struct SampledImplicit2D {
    pub segments: Vec<[(f64, f64); 2]>,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

pub struct SampledImplicit3D {
    pub triangles: Vec<[[f64; 3]; 3]>,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    pub z0: f64,
    pub z1: f64,
}

fn normalize_range(a: f64, b: f64) -> (f64, f64) {
    if (a - b).abs() < 1e-12 {
        (a - 1.0, b + 1.0)
    } else if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn lerp2(a: (f64, f64), b: (f64, f64), va: f64, vb: f64) -> (f64, f64) {
    let t = if (vb - va).abs() < 1e-15 {
        0.5
    } else {
        -va / (vb - va)
    };
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn lerp3(a: [f64; 3], b: [f64; 3], va: f64, vb: f64) -> [f64; 3] {
    let t = if (vb - va).abs() < 1e-15 {
        0.5
    } else {
        -va / (vb - va)
    };
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn triangle_segment(points: [(f64, f64); 3], values: [f64; 3]) -> Option<[(f64, f64); 2]> {
    let mut crossings = Vec::with_capacity(2);
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        let inside_i = values[i] < 0.0;
        let inside_j = values[j] < 0.0;
        if inside_i != inside_j {
            crossings.push(lerp2(points[i], points[j], values[i], values[j]));
        }
    }
    match crossings.len() {
        2 => Some([crossings[0], crossings[1]]),
        _ => None,
    }
}

/// 把立方体拆成 6 个四面体（共享 0–7 主对角线）。
const CUBE_CORNERS: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [0, 1, 0],
    [1, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [0, 1, 1],
    [1, 1, 1],
];

const CUBE_TETRAHEDRA: [[usize; 4]; 6] = [
    [0, 1, 3, 7],
    [0, 1, 5, 7],
    [0, 2, 3, 7],
    [0, 2, 6, 7],
    [0, 4, 5, 7],
    [0, 4, 6, 7],
];

fn tetra_triangles(points: [[f64; 3]; 4], values: [f64; 4], out: &mut Vec<[[f64; 3]; 3]>) {
    let inside: Vec<usize> = (0..4).filter(|i| values[*i] < 0.0).collect();
    let edges: Vec<(usize, usize)> = match inside.len() {
        0 | 4 => return,
        1 => {
            let a = inside[0];
            (0..4).filter(|i| *i != a).map(|i| (a, i)).collect()
        }
        3 => {
            let outside = (0..4).find(|i| !inside.contains(i)).unwrap_or(0);
            inside.iter().map(|i| (outside, *i)).collect()
        }
        _ => {
            let a = inside[0];
            let b = inside[1];
            let mut others = (0..4).filter(|i| !inside.contains(i));
            let c = others.next().unwrap_or(0);
            let d = others.next().unwrap_or(c);
            vec![(a, c), (a, d), (b, d), (b, c)]
        }
    };
    let crossings: Vec<[f64; 3]> = edges
        .iter()
        .map(|(i, j)| lerp3(points[*i], points[*j], values[*i], values[*j]))
        .collect();
    match crossings.len() {
        3 => out.push([crossings[0], crossings[1], crossings[2]]),
        4 => {
            out.push([crossings[0], crossings[1], crossings[2]]);
            out.push([crossings[0], crossings[2], crossings[3]]);
        }
        _ => {}
    }
}

/// 对隐函数 `F(x, y) = 0` 做 marching triangles，返回线段。
pub fn sample_implicit_2d(
    session: &mut Session,
    node: &Node,
    vx: &str,
    vy: &str,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    side: usize,
) -> SampledImplicit2D {
    let count = side.clamp(20, 320);
    let (x0, x1) = normalize_range(x0, x1);
    let (y0, y1) = normalize_range(y0, y1);
    let xs = linspace(x0, x1, count);
    let ys = linspace(y0, y1, count);
    let compiled = compile_plot_expression(session, node);
    let mut values = vec![None; count * count];
    for (j, y) in ys.iter().enumerate() {
        for (i, x) in xs.iter().enumerate() {
            values[i + count * j] =
                eval_point(session, node, compiled.as_ref(), &[(vx, *x), (vy, *y)]);
        }
    }
    let mut segments = Vec::new();
    for j in 0..count - 1 {
        for i in 0..count - 1 {
            let corners = [
                (xs[i], ys[j], values[i + count * j]),
                (xs[i + 1], ys[j], values[i + 1 + count * j]),
                (xs[i + 1], ys[j + 1], values[i + 1 + count * (j + 1)]),
                (xs[i], ys[j + 1], values[i + count * (j + 1)]),
            ];
            if corners.iter().any(|(_, _, value)| value.is_none()) {
                continue;
            }
            let points = [
                (corners[0].0, corners[0].1),
                (corners[1].0, corners[1].1),
                (corners[2].0, corners[2].1),
                (corners[3].0, corners[3].1),
            ];
            let scalars = [
                corners[0].2.unwrap_or(0.0),
                corners[1].2.unwrap_or(0.0),
                corners[2].2.unwrap_or(0.0),
                corners[3].2.unwrap_or(0.0),
            ];
            for triangle in [[0usize, 1, 2], [0, 2, 3]] {
                let p = [
                    points[triangle[0]],
                    points[triangle[1]],
                    points[triangle[2]],
                ];
                let v = [
                    scalars[triangle[0]],
                    scalars[triangle[1]],
                    scalars[triangle[2]],
                ];
                if let Some(segment) = triangle_segment(p, v) {
                    segments.push(segment);
                }
            }
        }
    }
    SampledImplicit2D {
        segments,
        x0,
        x1,
        y0,
        y1,
    }
}

/// 对隐函数 `F(x, y, z) = 0` 做 marching tetrahedra，返回三角网格。
pub fn sample_implicit_3d(
    session: &mut Session,
    node: &Node,
    vx: &str,
    vy: &str,
    vz: &str,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    z0: f64,
    z1: f64,
    side: usize,
) -> SampledImplicit3D {
    let count = side.clamp(8, 64);
    let (x0, x1) = normalize_range(x0, x1);
    let (y0, y1) = normalize_range(y0, y1);
    let (z0, z1) = normalize_range(z0, z1);
    let xs = linspace(x0, x1, count);
    let ys = linspace(y0, y1, count);
    let zs = linspace(z0, z1, count);
    let compiled = compile_plot_expression(session, node);
    let mut values = vec![None; count * count * count];
    for (k, z) in zs.iter().enumerate() {
        for (j, y) in ys.iter().enumerate() {
            for (i, x) in xs.iter().enumerate() {
                values[i + count * (j + count * k)] = eval_point(
                    session,
                    node,
                    compiled.as_ref(),
                    &[(vx, *x), (vy, *y), (vz, *z)],
                );
            }
        }
    }
    let mut triangles = Vec::new();
    for k in 0..count - 1 {
        for j in 0..count - 1 {
            for i in 0..count - 1 {
                let mut points = [[0.0f64; 3]; 8];
                let mut scalars = [0.0f64; 8];
                let mut valid = true;
                for (corner, offsets) in CUBE_CORNERS.iter().enumerate() {
                    let ii = i + offsets[0];
                    let jj = j + offsets[1];
                    let kk = k + offsets[2];
                    let value = values[ii + count * (jj + count * kk)];
                    match value {
                        Some(value) => {
                            points[corner] = [xs[ii], ys[jj], zs[kk]];
                            scalars[corner] = value;
                        }
                        None => {
                            valid = false;
                            break;
                        }
                    }
                }
                if !valid {
                    continue;
                }
                for tetrahedron in CUBE_TETRAHEDRA {
                    tetra_triangles(
                        [
                            points[tetrahedron[0]],
                            points[tetrahedron[1]],
                            points[tetrahedron[2]],
                            points[tetrahedron[3]],
                        ],
                        [
                            scalars[tetrahedron[0]],
                            scalars[tetrahedron[1]],
                            scalars[tetrahedron[2]],
                            scalars[tetrahedron[3]],
                        ],
                        &mut triangles,
                    );
                }
            }
        }
    }
    SampledImplicit3D {
        triangles,
        x0,
        x1,
        y0,
        y1,
        z0,
        z1,
    }
}
