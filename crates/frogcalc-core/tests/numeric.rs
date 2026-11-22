use frogcalc_core::numeric::bigfloat::{bf_exp, bf_ln, bf_pi, bf_sin, bf_sqrt, BigFloat};
use frogcalc_core::numeric::{rat, rat_display, rat_exact_decimal};

#[test]
fn rational_display() {
    assert_eq!(rat_display(&rat(1, 2)), "1/2");
    assert_eq!(rat_display(&rat(1, 3)), "1/3");
    assert_eq!(rat_display(&rat(3, 2000)), "3/2000");
    assert_eq!(rat_display(&rat(2, 1)), "2");
    assert_eq!(rat_exact_decimal(&rat(1, 2), 20).as_deref(), Some("0.5"));
    assert_eq!(
        rat_exact_decimal(&rat(3, 2000), 20).as_deref(),
        Some("0.0015")
    );
    assert_eq!(rat_exact_decimal(&rat(1, 3), 20), None);
    assert_eq!(rat_exact_decimal(&rat(1, 2), 0), None);
}

#[test]
fn pi_50_digits() {
    let pi = bf_pi(50);
    assert!(pi
        .to_string()
        .starts_with("3.1415926535897932384626433832795028841971693993751"));
}

#[test]
fn pi_100_digits() {
    let pi = bf_pi(100);
    assert!(pi
        .to_string()
        .starts_with("3.14159265358979323846264338327950288419716939937510582097494459230781640628620899862803482534211706"));
}

#[test]
fn constants_and_functions() {
    let one = BigFloat::one(50);
    let e = bf_exp(&one, 50).unwrap();
    let ln2 = bf_ln(&BigFloat::from_i64(2, 50), 50).unwrap();
    let sqrt2 = bf_sqrt(&BigFloat::from_i64(2, 50), 50).unwrap();
    let sin1 = bf_sin(&one, 50).unwrap();
    let e100 = bf_exp(&BigFloat::one(100), 100).unwrap();
    println!("e={e}\nln2={ln2}\nsqrt2={sqrt2}\nsin1={sin1}\ne100={e100}");
    assert!(e
        .to_string()
        .starts_with("2.7182818284590452353602874713526624977572"));
    assert!(ln2
        .to_string()
        .starts_with("0.69314718055994530941723212145817656807550013436"));
    assert!(sqrt2
        .to_string()
        .starts_with("1.41421356237309504880168872420969807856967187537"));
    assert!(sin1
        .to_string()
        .starts_with("0.8414709848078965066525023216302989996225630607"));
}
