//! 2D / 3D 函数采样，供界面绘图使用。
//!
//! 采样复用 `Session::eval_sample`，因此用户函数、变量、分段函数和角度制设置都生效。

use crate::eval::session::Session;
use crate::parser::Node;

pub const MAX_2D_SAMPLES: usize = 4000;
pub const MAX_3D_SIDE: usize = 80;

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
    let points = linspace(x0, x1, count)
        .into_iter()
        .map(|x| {
            session
                .eval_sample(node, &[(var, x)])
                .and_then(finite)
                .map(|y| (x, y))
        })
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
    let mut values = Vec::with_capacity(count * count);
    for y in &ys {
        for x in &xs {
            values.push(
                session
                    .eval_sample(node, &[(vx, *x), (vy, *y)])
                    .and_then(finite),
            );
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
