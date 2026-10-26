//! Tests for the order of values.
//!
//! Rust's sort may panic if `Ord` is not a total order,
//! so every comparison that sorting uses must be transitive.
//! The edge cases assume a 64-bit `isize`.
#![cfg(target_pointer_width = "64")]

pub mod common;

use common::give;
use core::cmp::Ordering;
use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Filter, Native, RcIter, ValT};
use jaq_json::{Val, ValR};
use jaq_std::ValT as _;
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

const P53: isize = 1 << 53;

/// `2^53 + 1`, `2^53` and `2^53.0`, repeated.
///
/// `2^53 + 1` rounds to `2^53` as float.
/// When integers and floats were compared via `i as f64`,
/// `2^53 + 1` was greater than `2^53`, yet both equalled `2^53.0`,
/// so sorting them panicked.
fn around_2_pow_53() -> serde_json::Value {
    let xs = [json!(P53 + 1), json!(P53), json!(9007199254740992.0)];
    xs.iter().cycle().take(24).cloned().collect()
}

#[test]
fn sort_around_2_pow_53() {
    let low = ["9007199254740992", "9007199254740992.0"].repeat(8);
    let high = ["9007199254740993"; 8];
    let sorted = format!("[{}]", [&low[..], &high[..]].concat().join(","));
    give(around_2_pow_53(), "sort | tojson", json!(sorted));
    give(around_2_pow_53(), "sort_by(.) | tojson", json!(sorted));
    give(
        around_2_pow_53(),
        "group_by(.) | map(length)",
        json!([16, 8]),
    );
    let unique = "[9007199254740992,9007199254740993]";
    give(around_2_pow_53(), "unique | tojson", json!(unique));
    give(around_2_pow_53(), "unique_by(.) | tojson", json!(unique));
    let diff = "[9007199254740993, 9007199254740994] - .";
    give(around_2_pow_53(), diff, json!([P53 + 2]));
}

#[test]
fn unique_around_2_pow_63() {
    // 2^63 does not fit into `isize`, so it is a `Num`
    let id = |d| match d {
        6 => json!(isize::MAX - 1),
        7 => json!(isize::MAX),
        _ => json!(1_u64 << 63),
    };
    let digits = [
        7, 7, 6, 7, 8, 7, 7, 7, 7, 7, 8, 6, 8, 6, 7, 6, 6, 8, 7, 8, 8,
    ];
    let ids: serde_json::Value = digits.into_iter().map(id).collect();
    let unique = "[9223372036854775806,9223372036854775807,9223372036854775808]";
    give(ids, "unique | tojson", json!(unique));
}

#[test]
fn int_float_cmp_exact() {
    give(
        json!(null),
        "9007199254740993 == 9007199254740992.0",
        json!(false),
    );
    give(
        json!(null),
        "9007199254740993 > 9007199254740992.0",
        json!(true),
    );
    give(
        json!(null),
        "9007199254740992 == 9007199254740992.0",
        json!(true),
    );
    // 2^63 - 1 vs. 2^63
    give(json!(isize::MAX), ". < 9223372036854775807.0", json!(true));
    give(json!(isize::MAX), ". < 9223372036854775808", json!(true));
    give(
        json!(isize::MIN),
        ". == -9223372036854775808.0",
        json!(true),
    );
    give(json!(0), ". == -0.0", json!(true));
    // NaN stays smaller than any number
    give(json!(1), "[. > nan, nan < .]", json!([true, true]));
}

fn num(n: &str) -> Val {
    Val::Num(Rc::new(n.to_string()))
}

/// Numbers at the edges of exact `f64`, `isize` and `i128`, in all representations,
/// as ascending classes of equal values.
///
/// A `Num` is the integer it writes if that fits into `i128`, else its nearest float.
/// A `Num` that does not parse is `null`.
fn number_classes() -> Vec<Vec<Val>> {
    use Val::{Float, Int};
    let big = |sign: &str| format!("{sign}1{}", "0".repeat(400));
    vec![
        vec![
            Val::Null,
            num(""),
            num("abc"),
            num("1e"),
            num("+-9223372036854775809"),
            num("92233720368547758O9"),
        ],
        vec![Val::Bool(false)],
        vec![Val::Bool(true)],
        vec![
            Float(f64::NEG_INFINITY),
            num("-inf"),
            num("-1e400"),
            num(&big("-")),
        ],
        vec![Float(-f64::MAX), num("-1.7976931348623157e308")],
        // float below -2^127
        vec![
            Float(-170141183460469269510619166673045815296.0),
            num("-170141183460469269510619166673045815296"),
        ],
        // `i128::MIN`, and the integer below it, which is beyond `i128`
        vec![
            Float(-170141183460469231731687303715884105728.0),
            num("-170141183460469231731687303715884105728"),
            num("-170141183460469231731687303715884105729"),
        ],
        vec![num("-170141183460469231731687303715884105727")],
        vec![num("-18446744073709551617")],
        vec![Float(-18446744073709551616.0), num("-18446744073709551616")],
        // float below -2^63
        vec![Float(-9223372036854777856.0), num("-9223372036854777856")],
        vec![num("-9223372036854775809")],
        vec![
            Int(isize::MIN),
            Float(-9223372036854775808.0),
            num("-9223372036854775808"),
        ],
        vec![Int(isize::MIN + 1)],
        vec![Int(-P53 - 1)],
        vec![Int(-P53), Float(-(P53 as f64))],
        vec![Int(-1), Float(-1.0), num("-1e0")],
        vec![Float(-0.5), num("-0.5")],
        vec![
            Int(0),
            Float(0.0),
            Float(-0.0),
            num("-0"),
            num("0.0"),
            num("-0e5"),
            num("1e-99999999999999999999"),
            num("0000000000000000"),
            num("-0000000000000000"),
        ],
        vec![Float(5e-324), num("5e-324")],
        vec![Float(f64::MIN_POSITIVE)],
        vec![Float(0.1), num("0.1"), num("0.10000000000000001")],
        vec![Float(0.5), num("0.5")],
        vec![
            Int(1),
            Float(1.0),
            num("1.0"),
            num("1e0"),
            num("+0000000000000001"),
        ],
        vec![
            Int(P53 - 1),
            Float((P53 - 1) as f64),
            num("9007199254740991"),
        ],
        // a fraction is its nearest float
        vec![
            Int(P53),
            Float(P53 as f64),
            num("9007199254740992"),
            num("9007199254740993.0"),
        ],
        // an integer written with 16 bytes is beyond exact floats
        vec![Int(P53 + 1), num("9007199254740993")],
        vec![Int(P53 + 2), Float((P53 + 2) as f64)],
        // float below 2^63
        vec![
            Int(9223372036854774784),
            Float(9223372036854774784.0),
            num("9223372036854774784"),
        ],
        vec![Int(isize::MAX - 1)],
        vec![Int(isize::MAX), num("9223372036854775807")],
        vec![
            Float(9223372036854775808.0),
            num("9223372036854775808"),
            num("9223372036854775808.0"),
            num("9223372036854775808.5"),
            // beyond the next integer, which compares exactly
            num("9223372036854775809.5"),
            num("9.223372036854775809e18"),
        ],
        vec![
            num("9223372036854775809"),
            num("+9223372036854775809"),
            num("09223372036854775809"),
        ],
        vec![num("9223372036854775810")],
        // float above 2^63
        vec![Float(9223372036854777856.0), num("9223372036854777856")],
        // float below 2^64
        vec![Float(18446744073709549568.0), num("18446744073709549568")],
        vec![num("18446744073709551615")],
        vec![Float(18446744073709551616.0), num("18446744073709551616")],
        vec![num("18446744073709551617")],
        // float below 2^127
        vec![
            Float(170141183460469212842221372237303250944.0),
            num("170141183460469212842221372237303250944"),
        ],
        vec![num("170141183460469231731687303715884105726")],
        // `i128::MAX` rounds up to the float 2^127
        vec![num("170141183460469231731687303715884105727")],
        // integers beyond `i128` are their nearest float
        vec![
            Float(170141183460469231731687303715884105728.0),
            num("170141183460469231731687303715884105728"),
            num("170141183460469231731687303715884105729"),
        ],
        vec![Float(f64::MAX), num("1.7976931348623157e308")],
        vec![
            Float(f64::INFINITY),
            num("inf"),
            num("1e400"),
            num(&big("")),
        ],
        vec![Val::from(String::new())],
    ]
}

fn edge_numbers() -> Vec<Val> {
    number_classes().into_iter().flatten().collect()
}

#[test]
fn numbers_compare_by_exact_value() {
    let classes = number_classes();
    let vals = classes.iter().enumerate();
    let vals: Vec<_> = vals
        .flat_map(|(i, c)| c.iter().map(move |v| (i, v)))
        .collect();
    for (i, a) in &vals {
        for (j, b) in &vals {
            assert_eq!(a.cmp(b), i.cmp(j), "{a:?} vs. {b:?}");
            assert_eq!(a == b, i == j, "{a:?} vs. {b:?}");
        }
    }
}

#[test]
fn ids_above_2_pow_63_stay_distinct() {
    // 2^63 and above do not fit into `isize`, so they are `Num`s
    let ids = || {
        let id = |d| json!((1_u64 << 63) + d);
        json!([id(1), id(0), id(1), id(0), u64::MAX, id(2048)])
    };
    let unique =
        "[9223372036854775808,9223372036854775809,9223372036854777856,18446744073709551615]";
    give(ids(), "unique | tojson", json!(unique));
    give(ids(), "group_by(.) | map(length)", json!([2, 2, 1, 1]));
    give(ids(), "[.[0] == .[1], .[0] > .[1]]", json!([false, true]));
    give(ids(), "index(9223372036854775808)", json!(1));
    give(
        ids(),
        "[.[0]] | contains([9223372036854775808])",
        json!(false),
    );
    give(
        ids(),
        ". - [9223372036854775808] | unique | length",
        json!(3),
    );
    let float = "[.[1] == 9223372036854775808.0, .[0] > 9223372036854775808.0]";
    give(ids(), float, json!([true, true]));
    // NaN stays smaller than any number
    give(ids(), "[.[0] > nan, nan < .[0]]", json!([true, true]));
}

#[test]
fn order_is_total_on_edge_numbers() {
    let nums = edge_numbers();
    let arrs = nums.iter().map(|x| [x.clone()].into_iter().collect());
    let vals: Vec<Val> = nums.iter().cloned().chain(arrs).collect();
    for a in &vals {
        assert_eq!(a.cmp(a), Ordering::Equal, "{a:?}");
        for b in &vals {
            let ab = a.cmp(b);
            assert_eq!(ab, b.cmp(a).reverse(), "{a:?} vs. {b:?}");
            assert_eq!(a == b, ab == Ordering::Equal, "{a:?} vs. {b:?}");
            for c in &vals {
                if ab.is_le() && b.cmp(c).is_le() {
                    assert!(a.cmp(c).is_le(), "{a:?} <= {b:?} <= {c:?}");
                }
            }
        }
    }
}

fn compile(code: &str) -> Filter<Native<Val>> {
    let arena = Arena::default();
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let modules = loader.load(&arena, File { path: (), code }).unwrap();
    Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .unwrap()
}

fn run(f: &Filter<Native<Val>>, x: Val) -> Vec<ValR> {
    let inputs = RcIter::new(core::iter::empty());
    f.run((Ctx::new([], &inputs), x)).collect()
}

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Sorting mixed numbers, also inside arrays and objects, must not panic.
///
/// This includes NaN, which is smaller than any number and than itself, like in jq.
/// Rust's sort checks the order only while merging two runs,
/// comparing the right run's elements with the left's,
/// so NaN reads as a consistent order there.
/// This test fails if a future Rust sort detects the NaN quirk.
#[test]
fn sort_mixed_numbers_and_nan() {
    let nan = || Val::Float(f64::NAN);
    let key = || Val::from("k".to_string());
    let mut pool = edge_numbers();
    pool.extend([
        Val::from_map([(key(), Val::Int(P53 + 1))]).unwrap(),
        Val::from_map([(key(), Val::Float(P53 as f64))]).unwrap(),
    ]);
    // values from here on contain NaN
    let nan_from = pool.len();
    pool.extend([
        nan(),
        [nan(), Val::Int(1)].into_iter().collect(),
        [nan(), Val::Int(0)].into_iter().collect(),
        [Val::Int(1), nan()].into_iter().collect(),
        Val::from_map([(key(), nan())]).unwrap(),
    ]);
    let filters = ["sort", "sort_by(.)", "group_by(.)", "unique", ". - ."];
    let filters = filters.map(|code| (code, compile(code)));
    let sort = compile("sort");

    let mut state = 0x2545_f491_4f6c_dd1d;
    for len in 0..=100 {
        for _ in 0..10 {
            let idxs: Vec<usize> = (0..len)
                .map(|_| xorshift(&mut state) as usize % pool.len())
                .collect();
            let arr: Val = idxs.iter().map(|i| pool[*i].clone()).collect();
            for (code, f) in &filters {
                let out = catch_unwind(AssertUnwindSafe(|| run(f, arr.clone())));
                assert!(out.is_ok(), "{code} panicked on {arr:?}");
            }
            // without NaN, the output of `sort` is ordered
            if idxs.iter().all(|i| *i < nan_from) {
                let sorted = run(&sort, arr).pop().unwrap().unwrap();
                let sorted: Vec<Val> = sorted.into_seq().unwrap();
                assert!(sorted.windows(2).all(|w| w[0] <= w[1]), "{sorted:?}");
            }
        }
    }
}
