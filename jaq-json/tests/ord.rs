//! Tests for the order of values.
//!
//! Rust's sort may panic if `Ord` is not a total order,
//! so every comparison that sorting uses must be transitive.

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

/// Numbers at the edges of exact `f64` and `isize`, in all representations.
fn edge_numbers() -> Vec<Val> {
    let ints = [
        0,
        1,
        -1,
        P53 - 1,
        P53,
        P53 + 1,
        P53 + 2,
        -P53 - 1,
        -P53,
        isize::MAX - 1,
        isize::MAX,
        isize::MIN + 1,
        isize::MIN,
    ];
    let floats = [
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.0,
        P53 as f64,
        (P53 + 2) as f64,
        -(P53 as f64),
        // largest float below 2^63
        9223372036854774784.0,
        9223372036854775808.0,
        -9223372036854775808.0,
        -9223372036854777856.0,
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let nums = [
        "9223372036854775808",
        "9223372036854775809",
        "18446744073709551615",
        "-9223372036854775809",
        "9007199254740993.0",
        "0.1",
        "1e400",
        "-1e400",
    ];
    let ints = ints.into_iter().map(Val::Int);
    let floats = floats.into_iter().map(Val::Float);
    let nums = nums.into_iter().map(|n| Val::Num(Rc::new(n.to_string())));
    ints.chain(floats).chain(nums).collect()
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
