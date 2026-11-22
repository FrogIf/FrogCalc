use frogcalc_core::eval::session::Session;

fn plain(input: &str) -> String {
    let mut session = Session::new();
    let results = session.execute(input);
    let last = results.last().expect("no result");
    assert!(last.ok, "{input}: {:?}", last.error);
    last.plain.clone()
}

fn error(input: &str) -> String {
    let mut session = Session::new();
    let results = session.execute(input);
    let last = results.last().expect("no result");
    assert!(!last.ok);
    last.error.clone().unwrap_or_default()
}

#[test]
fn exact_arithmetic() {
    let mut s = Session::new();
    assert_eq!(s.execute("a = 1 / 3")[0].plain, "1/3");
    assert_eq!(s.execute("b = 2 / 3")[0].plain, "2/3");
    assert_eq!(s.execute("a + b")[0].plain, "1");
    assert_eq!(plain("1/3 + 1/6"), "1/2");
    assert_eq!(plain("0.1 + 0.2"), "3/10");
    assert_eq!(plain("sqrt(2) * sqrt(2)"), "2");
    assert_eq!(plain("sqrt(2)^2"), "2");
    assert_eq!(plain("sqrt(8)"), "2*sqrt(2)");
    assert_eq!(plain("sqrt(8) / sqrt(2)"), "2");
    assert_eq!(plain("sqrt(-4)"), "2i");
}

#[test]
fn big_numbers() {
    assert_eq!(plain("2^10000").len(), 3011);
    assert_eq!(plain("1000!").len(), 2568);
}

#[test]
fn complex() {
    assert_eq!(plain("(2 + 3i) * (4 - 5i)"), "23 + 2i");
    assert_eq!(plain("(1 + i) / (1 - i)"), "i");
    assert_eq!(plain("i^2"), "-1");
}

#[test]
fn parser_precedence() {
    assert_eq!(plain("2 + 3 * 4"), "14");
    assert_eq!(plain("-2^2"), "-4");
    assert_eq!(plain("2^3^2"), "512");
    assert_eq!(plain("2(3 + 4)"), "14");
    assert_eq!(plain("2^3!"), "64");
    assert!(!plain("(x + 1)(x - 1)").is_empty());
}

#[test]
fn calculus_and_solve() {
    assert!(plain("diff(x^3 * sin(x), x)").contains("cos(x)"));
    assert!(plain("integrate(x^2, x)").contains("x^3"));
    assert!(plain("integrate(x * exp(x), x)").contains("exp(x)"));
    assert!(plain("integrate(1/(x^2 - 1), x)").contains("ln"));
    assert_eq!(plain("integrate(x^2, x, 0, 1)"), "1/3");
    assert_eq!(plain("solve(x^2 - 2x - 3, x)"), "{-1, 3}");
    assert_eq!(plain("solve(sin(x) == 0, x)"), "{0, pi}");
}

#[test]
fn symbolic_transform() {
    assert_eq!(plain("simplify((x^2 - 1)/(x - 1))"), "x + 1");
    assert_eq!(
        plain("expand((x + 1)^5)"),
        "x^5 + 5*x^4 + 10*x^3 + 10*x^2 + 5*x + 1"
    );
    assert!(plain("factor(x^4 - 1)").contains("x^2 + 1"));
}

#[test]
fn approximate() {
    assert!(plain("N(pi, 50)").starts_with("3.14159265358979323846"));
    assert!(plain("N(pi, 100)").starts_with(
        "3.14159265358979323846264338327950288419716939937510582097494459230781640628620899862803482534211706"
    ));
    assert!(plain("N(e, 50)").starts_with("2.71828182845904523536"));
    assert!(plain("N(sin(1), 50)").starts_with("0.84147098480789650665"));
}

#[test]
fn language_features() {
    assert_eq!(plain("a = {1, 2, 3}"), "{1, 2, 3}");
    let mut s = Session::new();
    assert_eq!(s.execute("f(x) = x^2 + 1")[0].plain, "f(x) = x^2 + 1");
    assert_eq!(s.execute("f(3)")[0].plain, "10");
    assert_eq!(plain("subs(x^2 + 1, x, 3)"), "10");
    assert!(error("1/0").contains("除数"));
    let mut s = Session::new();
    s.execute("2 + 3");
    assert_eq!(s.execute("ans * 2")[0].plain, "10");
}

#[test]
fn control_flow() {
    let mut s = Session::new();
    let results = s.execute("total = 0\nfor i in range(1, 5) {\n    total = total + i\n}\ntotal");
    assert_eq!(results.last().unwrap().plain, "15");
    assert!(s.workspace().iter().all(|item| item.name != "i"));

    let results = s.execute("k = 0\nwhile k < 3 {\n    k = k + 1\n}\nk");
    assert_eq!(results.last().unwrap().plain, "3");

    let results = s.execute(
        "x = 0\nif x > 0 {\n    y = 1\n} else if x == 0 {\n    y = 2\n} else {\n    y = 3\n}\ny",
    );
    assert_eq!(results.last().unwrap().plain, "2");

    let results = s.execute("t = 0\nfor v in {1, 2, 3} {\n    t = t + v\n}\nt");
    assert_eq!(results.last().unwrap().plain, "6");

    let results = s.execute("t = 0\nfor i in range(10, 1, -2) {\n    t = t + i\n}\nt");
    assert_eq!(results.last().unwrap().plain, "30");
}

#[test]
fn comparisons_and_logic() {
    assert_eq!(plain("2 < 3"), "true");
    assert_eq!(plain("2 >= 3"), "false");
    assert_eq!(plain("2 != 3"), "true");
    assert_eq!(plain("true and false"), "false");
    assert_eq!(plain("not true"), "false");
    assert_eq!(plain("1 < 2 and 3 < 4"), "true");
    assert_eq!(plain("1 > 2 or 3 < 4"), "true");
    assert_eq!(plain("1 + 1 == 2"), "2 = 2");
    assert!(error("x < 1").contains("无法比较"));

    let mut s = Session::new();
    let results = s.execute("a = 0\nif 1 == 1 { a = 1 } else { a = 2 }\na");
    assert_eq!(results.last().unwrap().plain, "1");

    let results = s.execute("b = 0\nif not (1 > 2) { b = 1 }\nb");
    assert_eq!(results.last().unwrap().plain, "1");
}

#[test]
fn control_flow_guards_and_functions() {
    assert!(error("while true { }").contains("循环"));

    let mut s = Session::new();
    s.execute("f(x) = x > 0");
    assert_eq!(s.execute("f(1)")[0].plain, "true");
    assert_eq!(s.execute("f(-1)")[0].plain, "false");

    let source = s.workspace_source();
    let mut restored = Session::new();
    restored.execute(&source);
    assert_eq!(restored.execute("f(2)")[0].plain, "true");
}

#[test]
fn block_functions() {
    let mut s = Session::new();
    s.execute("abs2(x) = {\n    if x < 0 {\n        -x\n    } else {\n        x\n    }\n}");
    assert_eq!(s.execute("abs2(-3)")[0].plain, "3");
    assert_eq!(s.execute("abs2(2)")[0].plain, "2");

    s.execute("x = 10");
    s.execute("g(y) = {\n    x = 5\n    x + y\n}");
    assert_eq!(s.execute("g(1)")[0].plain, "6");
    assert_eq!(s.execute("x")[0].plain, "10");

    s.execute(
        "sumto(k) = {\n    total = 0\n    for j in range(1, k) {\n        total = total + j\n    }\n    total\n}",
    );
    assert_eq!(s.execute("sumto(10)")[0].plain, "55");

    s.execute(
        "fact(k) = {\n    if k <= 1 {\n        1\n    } else {\n        k * fact(k - 1)\n    }\n}",
    );
    assert_eq!(s.execute("fact(5)")[0].plain, "120");

    s.execute("triple() = {1, 2, 3}");
    assert_eq!(s.execute("triple()")[0].plain, "{1, 2, 3}");

    let source = s.workspace_source();
    let mut restored = Session::new();
    restored.execute(&source);
    assert_eq!(restored.execute("fact(4)")[0].plain, "24");
    assert_eq!(restored.execute("abs2(-2)")[0].plain, "2");
    assert_eq!(restored.execute("triple()")[0].plain, "{1, 2, 3}");

    let nested = s.execute("h(x) = {\n    q(y) = y\n};\nh(1)");
    assert!(!nested.last().unwrap().ok);
    assert!(nested
        .last()
        .unwrap()
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("函数内部"));
}

#[test]
fn workspace_source_roundtrip() {
    let mut s = Session::new();
    s.execute("a = 1/3\nf(x) = x^2 + 1\nb = {1, 2}");
    let source = s.workspace_source();
    let mut restored = Session::new();
    restored.execute(&source);
    let workspace = restored.workspace();
    assert!(workspace.iter().any(|i| i.name == "a" && i.plain == "1/3"));
    assert!(workspace.iter().any(|i| i.name == "f"));
    assert!(workspace
        .iter()
        .any(|i| i.name == "b" && i.plain == "{1, 2}"));
}

#[test]
fn exact_decimal_relation() {
    let mut s = Session::new();
    let half = s.execute("1/2")[0].clone();
    assert_eq!(half.plain, "1/2");
    assert_eq!(half.approx.as_deref(), Some("0.5"));
    assert!(half.approx_exact);

    let third = s.execute("1/3")[0].clone();
    assert_eq!(third.plain, "1/3");
    assert!(third.approx.is_some());
    assert!(!third.approx_exact);

    let complex = s.execute("1/2 + 1/4i")[0].clone();
    assert_eq!(complex.approx.as_deref(), Some("0.5 + 0.25i"));
    assert!(complex.approx_exact);
}

#[test]
fn angles_and_commands() {
    let mut s = Session::new();
    s.execute("setangle(\"deg\")");
    assert_eq!(s.execute("sin(30)")[0].plain, "1/2");
    assert_eq!(s.execute("asin(1/2)")[0].plain, "30");
    assert!(s.execute("help(\"syntax\")")[0]
        .plain
        .contains("FrogCalc 语言"));
    s.execute("k = 1");
    assert_eq!(s.workspace().len(), 1);
    s.execute("clear()");
    assert_eq!(s.workspace().len(), 0);
}

#[test]
fn semicolon_and_multiline() {
    let mut s = Session::new();
    let results = s.execute("a = 2; b = 3; a + b");
    assert_eq!(results.len(), 3);
    assert!(results[0].suppressed);
    assert!(results[1].suppressed);
    assert_eq!(results[2].plain, "5");
    let multiline = s.execute("diff(\n  x^3 * sin(x),\n  x\n)")[0].clone();
    assert!(multiline.ok);
    assert!(multiline.plain.contains("cos(x)"));
}
