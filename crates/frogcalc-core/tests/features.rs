use frogcalc_core::eval::session::Session;

fn plain(input: &str) -> String {
    let mut session = Session::new();
    let results = session.execute(input);
    let last = results.last().expect("no result");
    assert!(last.ok, "{input}: {:?}", last.error);
    last.plain.clone()
}

#[test]
fn zassenhaus_factorization() {
    fn check(expression: &str) {
        let factored = plain(&format!("factor({expression})"));
        let expanded = plain(&format!("expand({factored})"));
        let original = plain(&format!("expand({expression})"));
        assert_eq!(expanded, original, "factor({expression}) = {factored}");
    }
    check("x^4 + 4");
    check("x^5 + x + 1");
    check("2x^2 + 5x + 2");
    check("(x - 1)^2 * (x + 3)");
    check("x^6 - 1");
    check("x^8 + x^4 + 1");
    check("x^12 - 1");
    check("6x^4 + 13x^3 + 6x^2 - 4x - 4");
    check("x^24 - 1");
    check("x^24 - x - 1");
    check("x^8 + x + 1");
    check("-2x^2 + 2");
    check("(1/2)*x^2 - 1/2");
    check("3x^3 - 3x");
    let twelfth = plain("factor(x^12 - 1)");
    assert!(twelfth.contains("x^2 + 1"));
    assert!(twelfth.contains("x^4 - x^2 + 1"));
    assert_eq!(plain("expand(factor(x^12 - 1))"), plain("expand(x^12 - 1)"));
    assert!(plain("factor(x^4 + 4)").contains("x^2"));
    assert_eq!(
        plain("expand(factor(x^5 + x + 1))"),
        plain("expand(x^5 + x + 1)")
    );
    assert_eq!(plain("solve(x^4 - 5x^2 + 4, x)"), "{-2, -1, 1, 2}");
    let cubic = plain("solve(x^3 - 2, x)");
    assert!(cubic.starts_with('{'));
    assert!(cubic.contains('i') || cubic.contains("1.2599"));
}

#[test]
fn assumptions_drive_simplification() {
    let mut s = Session::new();
    s.execute("assume(x > 0)");
    assert_eq!(s.execute("simplify(abs(x))")[0].plain, "x");
    assert_eq!(s.execute("sqrt(x^2)")[0].plain, "x");
    assert_eq!(s.execute("sqrt(x^2)")[0].unicode, "x");
    assert_eq!(s.execute("assume(y < 0)")[0].ok, true);
    assert_eq!(s.execute("abs(y)")[0].plain, "-y");
    assert_eq!(s.execute("sqrt(y^2)")[0].plain, "-y");
    assert!(s.execute("assumptions()")[0].plain.contains('x'));
    s.execute("forget(x)");
    assert_eq!(s.execute("abs(x)")[0].plain, "abs(x)");
    // 假设随工作区序列化往返
    s.execute("assume(x >= 0)");
    let source = s.workspace_source();
    assert!(source.contains("assume(x >= 0)"));
    let mut restored = Session::new();
    restored.execute(&source);
    assert_eq!(restored.execute("sqrt(x^2)")[0].plain, "x");
    assert!(restored.execute("assumptions()")[0].plain.contains('x'));
}

#[test]
fn rational_and_extended_integrals() {
    fn check(integrand: &str) {
        let mut s = Session::new();
        let result = s
            .execute(&format!("integrate({integrand}, x)"))
            .last()
            .unwrap()
            .plain
            .clone();
        assert!(
            !result.contains("integrate("),
            "integrate({integrand}) = {result}"
        );
        let derivative = s
            .execute(&format!("diff({result}, x)"))
            .last()
            .unwrap()
            .plain
            .clone();
        let left = s
            .compile_expression(&derivative)
            .expect("derivative parses");
        let right = s.compile_expression(integrand).expect("integrand parses");
        for x in [0.5f64, 1.3, 1.7] {
            let a = s.eval_sample(&left, &[("x", x)]).expect("derivative value");
            let b = s.eval_sample(&right, &[("x", x)]).expect("integrand value");
            assert!(
                (a - b).abs() < 1e-7,
                "integrate({integrand}) = {result}: d/dx = {derivative}, 在 x = {x} 处 {a} != {b}"
            );
        }
    }
    check("1/x");
    check("1/(x^2 - 1)");
    check("1/(x^2 + 1)");
    check("1/(x^2 + 1)^2");
    check("x/(x^2 + 1)^3");
    check("1/(x*(x + 1))");
    check("1/(x*ln(x))");
    check("x/(x^2 + 1)");
    check("1/sqrt(4 - x^2)");
    check("sqrt(4 - x^2)");
}

#[test]
fn recurring_decimal_parsing() {
    use frogcalc_core::numeric::{rat, rat_from_recurring};
    assert_eq!(rat_from_recurring("0.1(3)").unwrap(), rat(2, 15));
    assert_eq!(rat_from_recurring("0.(3)").unwrap(), rat(1, 3));
    assert_eq!(rat_from_recurring("0.[3]").unwrap(), rat(1, 3));
    assert_eq!(rat_from_recurring("1.2(34)").unwrap(), rat(611, 495));
    assert_eq!(rat_from_recurring("0.0(3)").unwrap(), rat(1, 30));
    assert_eq!(rat_from_recurring("-0.0(3)").unwrap(), rat(-1, 30));
    assert_eq!(rat_from_recurring("+0.0[3]").unwrap(), rat(1, 30));
    assert_eq!(rat_from_recurring("2.5").unwrap(), rat(5, 2));
    assert_eq!(rat_from_recurring("12").unwrap(), rat(12, 1));
    assert_eq!(rat_from_recurring(".5").unwrap(), rat(1, 2));
    assert_eq!(rat_from_recurring("5.").unwrap(), rat(5, 1));
    assert_eq!(rat_from_recurring("0.9(9)").unwrap(), rat(1, 1));
    for bad in [
        "0.1()", "abc", "0.(3)5", "3(3)", "0.1(3", "0.1(3]", "", "  ", "0..1", "0.1(2)3",
    ] {
        assert!(rat_from_recurring(bad).is_err(), "{bad}");
    }
}

#[test]
fn recurring_decimal_in_session() {
    let mut s = Session::new();
    let result = s.execute("recurring(\"0.1(3)\")")[0].clone();
    assert_eq!(result.plain, "2/15");
    assert!(result.approx.unwrap_or_default().starts_with("0.1333"));
    assert!(!result.approx_exact);
    assert!(result.note.unwrap_or_default().contains("循环小数"));
    assert_eq!(s.execute("recurring(\"0.(3)\") * 3")[0].plain, "1");
    assert_eq!(s.execute("recurring(\"2.5\")")[0].plain, "5/2");
    assert_eq!(s.execute("recurring(0.25)")[0].plain, "1/4");
    s.execute("pattern = \"0.(6)\"");
    assert_eq!(s.execute("recurring(pattern) * 6")[0].plain, "4");

    // 存入工作区后保持精确分数
    s.execute("a = recurring(\"1.2(34)\")");
    assert_eq!(s.execute("a * 495")[0].plain, "611");
    let source = s.workspace_source();
    assert!(source.contains("a = 611/495"));
    let mut restored = Session::new();
    restored.execute(&source);
    assert_eq!(restored.execute("a")[0].plain, "611/495");

    for bad in [
        "recurring(\"0.1()\")",
        "recurring(\"abc\")",
        "recurring(\"0.(3)5\")",
        "recurring(\"3(3)\")",
        "recurring(1, 2)",
        "recurring(true)",
    ] {
        let results = s.execute(bad);
        assert!(!results.last().unwrap().ok, "{bad}");
    }
}

#[test]
fn plot_sampling() {
    use frogcalc_core::plot::{sample_2d, sample_surface};
    let mut s = Session::new();
    let node = s.compile_expression("sin(x)").expect("compiles");
    let curve = sample_2d(&mut s, &node, "x", 0.0, std::f64::consts::PI, 65);
    assert!(curve
        .points
        .iter()
        .flatten()
        .any(|(_, y)| (*y - 1.0).abs() < 1e-3));
    let node = s.compile_expression("x^2 + y^2").expect("compiles");
    let surface = sample_surface(&mut s, &node, "x", "y", -1.0, 1.0, -1.0, 1.0, 9);
    assert_eq!(surface.values.len(), 81);
    assert!(surface.values.iter().flatten().all(|value| *value >= -1e-9));

    // 分段函数与用户函数都能采样
    s.execute("f(x) = piecewise(x < 0, -x, x)");
    let node = s.compile_expression("f(x)").expect("compiles");
    assert_eq!(s.eval_sample(&node, &[("x", -2.0)]), Some(2.0));
    assert_eq!(
        s.execute("f(x) = piecewise(x < 0, -x, x)")[0].plain,
        "f(x) = piecewise(x < 0, -x, x)"
    );
}

#[test]
fn piecewise_and_loop_control() {
    let mut s = Session::new();
    s.execute("f(x) = piecewise(x < 0, -x, x >= 0, x)");
    assert_eq!(s.execute("f(-2)")[0].plain, "2");
    assert_eq!(s.execute("f(3)")[0].plain, "3");
    assert_eq!(s.execute("piecewise(2 > 1, 10, 0)")[0].plain, "10");
    assert_eq!(s.execute("piecewise(2 < 1, 10, 20)")[0].plain, "20");

    let results = s.execute(
        "t = 0\nfor i in range(1, 10) {\n    if i > 4 {\n        break\n    }\n    t = t + i\n}\nt",
    );
    assert_eq!(results.last().unwrap().plain, "10");

    let results = s.execute(
        "t = 0\nfor i in range(1, 10) {\n    if i == 5 {\n        continue\n    }\n    t = t + i\n}\nt",
    );
    assert_eq!(results.last().unwrap().plain, "50");

    let results = s.execute("k = 0\nt = 0\nwhile k < 10 {\n    k = k + 1\n    if k == 3 {\n        continue\n    }\n    if k > 6 {\n        break\n    }\n    t = t + k\n}\nt");
    assert_eq!(results.last().unwrap().plain, "18");

    let results = s.execute("break");
    assert!(!results.last().unwrap().ok);

    // 近似路径支持分段函数与条件
    let approx = s.execute("N(piecewise(1 > 0, sqrt(2)), 20)")[0].clone();
    assert!(approx.ok, "{:?}", approx.error);
    assert!(approx.plain.starts_with("1.41421356"));
}
