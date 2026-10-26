//! Tests for named core filters, sorted by name.

pub mod common;

use common::{fail, give};
use jaq_json::Error;
use serde_json::json;

yields!(bsearch_absent1, "[1, 3] | bsearch(0)", -1);
yields!(bsearch_absent2, "[1, 3] | bsearch(2)", -2);
yields!(bsearch_absent3, "[1, 3] | bsearch(4)", -3);
yields!(bsearch_present, "[1, 3] | [bsearch(1, 3)]", [0, 1]);

#[test]
fn fromjson_depth() {
    let nested = |n| format!("{}{}", "[".repeat(n), "]".repeat(n));
    give(json!(nested(128)), "fromjson | tojson", json!(nested(128)));
    let too_deep = |s: String| {
        let err = Error::str(format_args!(
            "cannot parse {s} as JSON: maximal depth exceeded"
        ));
        fail(json!(s), "fromjson", err)
    };
    too_deep(nested(129));
    // unterminated input, deep enough to overflow the stack without a bound
    too_deep("[".repeat(100_000));
    too_deep(r#"{"a":"#.repeat(100_000));
}

#[test]
fn has() {
    /* TODO: reenable these tests
    let err = Error::Index(Val::Null, Val::Int(0));
    fail(json!(null), "has(0)", err);
    let err = Error::Index(Val::Int(0), Val::Null);
    fail(json!(0), "has([][0])", err);
    let err = Error::Index(Val::Int(0), Val::Int(1));
    fail(json!(0), "has(1)", err);
    let err = Error::Index(Val::Str("a".to_string().into()), Val::Int(0));
    fail(json!("a"), "has(0)", err);
    */

    give(json!([0, null]), "has(0)", json!(true));
    give(json!([0, null]), "has(1)", json!(true));
    give(json!([0, null]), "has(2)", json!(false));

    give(json!({"a": 1, "b": null}), r#"has("a")"#, json!(true));
    give(json!({"a": 1, "b": null}), r#"has("b")"#, json!(true));
    give(json!({"a": 1, "b": null}), r#"has("c")"#, json!(false));
}

yields!(indices_str, r#""a,b, cd, efg" | indices(", ")"#, [3, 7]);
yields!(
    indices_arr_num,
    "[0, 1, 2, 1, 3, 1, 4] | indices(1)",
    [1, 3, 5]
);
yields!(
    indices_arr_arr,
    "[0, 1, 2, 3, 1, 4, 2, 5, 1, 2, 6, 7] | indices([1, 2])",
    [1, 8]
);
yields!(indices_arr_str, r#"["a", "b", "c"] | indices("b")"#, [1]);

yields!(indices_arr_empty, "[0, 1] | indices([])", json!([]));
yields!(indices_arr_larger, "[1, 2] | indices([1, 2, 3])", json!([]));

yields!(indices_arr_overlap, "[0, 0, 0] | indices([0, 0])", [0, 1]);
yields!(indices_str_overlap, r#""aaa" | indices("aa")"#, [0, 1]);
yields!(indices_str_gb1, r#""🇬🇧!" | indices("!")"#, [2]);
yields!(indices_str_gb2, r#""🇬🇧🇬🇧" | indices("🇬🇧")"#, [0, 2]);

#[test]
fn keys_unsorted() {
    give(json!([0, null, "a"]), "keys_unsorted", json!([0, 1, 2]));
    give(json!({"a": 1, "b": 2}), "keys_unsorted", json!(["a", "b"]));

    /* TODO: reenable these tests
    let err = |v| Error::Type(v, Type::Iter);
    fail(json!(0), "keys_unsorted", err(Val::Int(0)));
    fail(json!(null), "keys_unsorted", err(Val::Null));
    */
}

yields!(length_str_foo, r#""ƒoo" | length"#, 3);
yields!(length_str_namaste, r#""नमस्ते" | length"#, 6);
yields!(length_obj, r#"{"a": 5, "b": 3} | length"#, 2);
yields!(length_int_pos, " 2 | length", 2);
yields!(length_int_neg, "-2 | length", 2);
yields!(length_float_pos, " 2.5 | length", 2.5);
yields!(length_float_neg, "-2.5 | length", 2.5);

#[test]
fn tojson() {
    // TODO: correct this
    give(json!(1.0), "tojson", json!("1.0"));
    give(json!(0), "1.0 | tojson", json!("1.0"));
    give(json!(0), "1.1 | tojson", json!("1.1"));
    give(json!(0), "0.0 / 0.0 | tojson", json!("null"));
    give(json!(0), "1.0 / 0.0 | tojson", json!("null"));
}

#[test]
fn math_mul_str() {
    give(json!(["ab", 3]), ".[0] * .[1]", json!("ababab"));
    give(json!(["", isize::MAX]), ".[0] * .[1]", json!(""));
    // results of up to 16 MiB
    let len = 1 << 24;
    give(json!(["ab", len / 2]), ".[0] * .[1] | length", json!(len));
    let err = || Error::str("Repeat string result too long");
    fail(json!(["ab", len / 2 + 1]), ".[0] * .[1]", err());
    // this used to overflow the capacity
    fail(json!(["ab", isize::MAX]), ".[0] * .[1]", err());
    fail(json!(["ab", isize::MAX]), ".[1] * .[0]", err());
    // this used to fail allocating memory
    fail(json!(["a", isize::MAX]), ".[0] * .[1]", err());
}

#[test]
fn math_neg_big_int() {
    // integers beyond `isize` are `Num`s, and negating them is exact, like in jq
    let x = "9223372036854775809";
    give(
        json!(null),
        &format!("-{x} | tojson"),
        json!(format!("-{x}")),
    );
    give(json!(null), &format!("-(-{x}) | tojson"), json!(x));
    let parsed = format!("\"-{x}\" | fromjson | . == -{x}");
    give(json!(null), &parsed, json!(true));
    give(
        json!(null),
        &format!("[-{x}, -9223372036854775808] | unique | length"),
        json!(2),
    );
    give(json!(null), &format!("-{x} | abs | tojson"), json!(x));
    // `-2^63` fits into `isize`
    give(
        json!(null),
        "-9223372036854775808 | . == -9223372036854775807 - 1",
        json!(true),
    );
    give(
        json!(null),
        "-9223372036854775808 | tojson",
        json!("-9223372036854775808"),
    );
    // `-i128::MIN` is beyond `i128`, so it becomes a float
    let max = "170141183460469231731687303715884105728";
    let min = format!("\"-{max}\" | fromjson | -. | [type, . == {max}.0]");
    give(json!(null), &min, json!(["number", true]));
}

#[test]
fn math_rem() {
    // generated with this command with modification for errors and float rounding
    // cargo run -- -rn 'def f: -2, -1, 0, 2.1, 3, 2000000001; f as $a | f as $b | "give!(json!(null), \"\($a) / \($b)\", \(try ($a % $b) catch tojson));"'
    // TODO: use fail!()?
    give(json!(null), "-2 % -2", json!(0));
    give(json!(null), "-2 % -1", json!(0));
    give(
        json!(null),
        "try (-2 % 0) catch .",
        json!("cannot calculate -2 % 0"),
    );
    give(json!(null), "-2 % 2.1", json!(-2.0));
    give(json!(null), "-2 % 3", json!(-2));
    give(json!(null), "-2 % 2000000001", json!(-2));
    give(json!(null), "-1 % -2", json!(-1));
    give(json!(null), "-1 % -1", json!(0));
    give(
        json!(null),
        "try (-1 % 0) catch .",
        json!("cannot calculate -1 % 0"),
    );
    give(json!(null), "-1 % 2.1", json!(-1.0));
    give(json!(null), "-1 % 3", json!(-1));
    give(json!(null), "-1 % 2000000001", json!(-1));
    give(json!(null), "0 % -2", json!(0));
    give(json!(null), "0 % -1", json!(0));
    give(
        json!(null),
        "try (0 % 0) catch .",
        json!("cannot calculate 0 % 0"),
    );
    give(json!(null), "0 % 2.1", json!(0.0));
    give(json!(null), "0 % 3", json!(0));
    give(json!(null), "0 % 2000000001", json!(0));
    give(json!(null), "2.1 % -2 | . * 1000 | round", json!(100));
    give(json!(null), "2.1 % -1 | . * 1000 | round", json!(100));
    give(json!(null), "2.1 % 0 | isnan", json!(true));
    give(json!(null), "2.1 % 2.1", json!(0.0));
    give(json!(null), "2.1 % 3", json!(2.1));
    give(json!(null), "2.1 % 2000000001", json!(2.1));
    give(json!(null), "3 % -2", json!(1));
    give(json!(null), "3 % -1", json!(0));
    give(
        json!(null),
        "try (3 % 0) catch .",
        json!("cannot calculate 3 % 0"),
    );
    give(json!(null), "3 % 2.1 | . * 1000 | round", json!(900));
    give(json!(null), "3 % 3", json!(0));
    give(json!(null), "3 % 2000000001", json!(3));
    give(json!(null), "2000000001 % -2", json!(1));
    give(json!(null), "2000000001 % -1", json!(0));
    give(
        json!(null),
        "try (2000000001 % 0) catch .",
        json!("cannot calculate 2000000001 % 0"),
    );
    give(
        json!(null),
        "2000000001 % 2.1 | . * 1000 | round",
        json!(1800), // 1000 in jq
    );
    give(json!(null), "2000000001 % 3", json!(0));
    give(json!(null), "2000000001 % 2000000001", json!(0));
}

yields!(
    path_values,
    "[{a: 1, b: [2, 3]} | path_values]",
    json!([[["a"], 1], [["b"], [2, 3]], [["b", 0], 2], [["b", 1], 3]])
);
