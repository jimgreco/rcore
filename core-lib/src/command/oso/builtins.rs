//! Builtin types supported in Polar

use std::collections::HashMap;

use super::{Class, ClassBuilder, PolarValue};

fn boolean() -> ClassBuilder<bool> {
    ClassBuilder::<bool>::with_default()
        .with_equality_check()
        .name("bool")
}

fn integer() -> ClassBuilder<i64> {
    ClassBuilder::<i64>::with_default()
        .with_equality_check()
        .name("int")
}

fn float() -> ClassBuilder<f64> {
    ClassBuilder::<f64>::with_default()
        .with_equality_check()
        .name("float")
}

fn list() -> ClassBuilder<Vec<PolarValue>> {
    ClassBuilder::<Vec<PolarValue>>::with_default()
        .with_equality_check()
        .name("list")
}

fn dictionary() -> ClassBuilder<HashMap<String, PolarValue>> {
    ClassBuilder::<HashMap<String, PolarValue>>::with_default()
        .with_equality_check()
        .name("dict")
}

fn option() -> ClassBuilder<Option<PolarValue>> {
    ClassBuilder::<Option<PolarValue>>::with_default()
        .with_equality_check()
        .name("option")
        .legacy_add_method("unwrap", |v: &Option<PolarValue>| v.clone().unwrap())
        .legacy_add_method("is_none", Option::is_none)
        .legacy_add_method("is_some", Option::is_some)
        .with_iter()
}

fn string() -> ClassBuilder<String> {
    ClassBuilder::<String>::with_default()
        .with_equality_check()
        .name("string")
        .legacy_add_method("len", |s: &String| s.len() as i64)
        .legacy_add_method("is_empty", String::is_empty)
        .legacy_add_method("is_char_boundary", |s: &String, index: i64| {
            s.is_char_boundary(index as usize)
        })
        .legacy_add_method("bytes", |s: &String| {
            s.bytes().map(|c| c as i64).collect::<Vec<i64>>()
        })
        .legacy_add_method("chars", |s: &String| {
            s.chars().map(|c| c.to_string()).collect::<Vec<String>>()
        })
        .legacy_add_method("char_indices", |s: &String| {
            s.char_indices()
                .map(|(i, c)| {
                    vec![
                        PolarValue::Integer(i as i64),
                        PolarValue::String(c.to_string()),
                    ]
                })
                .collect::<Vec<Vec<PolarValue>>>()
        })
        .legacy_add_method("split_whitespace", |s: &String| {
            s.split_whitespace()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("lines", |s: &String| {
            s.lines().map(|c| c.to_string()).collect::<Vec<String>>()
        })
        .legacy_add_method("lines", |s: &String| {
            s.lines().map(|c| c.to_string()).collect::<Vec<String>>()
        })
        .legacy_add_method("contains", |s: &String, pat: String| s.contains(&pat))
        .legacy_add_method("starts_with", |s: &String, pat: String| s.starts_with(&pat))
        .legacy_add_method("ends_with", |s: &String, pat: String| s.ends_with(&pat))
        .legacy_add_method("find", |s: &String, pat: String| {
            s.find(&pat).map(|i| i as i64)
        })
        .legacy_add_method("rfind", |s: &String, pat: String| {
            s.rfind(&pat).map(|i| i as i64)
        })
        .legacy_add_method("split", |s: &String, pat: String| {
            s.split(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("rsplit", |s: &String, pat: String| {
            s.rsplit(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("split_terminator", |s: &String, pat: String| {
            s.split_terminator(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("rsplit_terminator", |s: &String, pat: String| {
            s.rsplit_terminator(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("splitn", |s: &String, n: i32, pat: String| {
            s.splitn(n as usize, &pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("rsplitn", |s: &String, n: i32, pat: String| {
            s.rsplitn(n as usize, &pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("matches", |s: &String, pat: String| {
            s.matches(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("rmatches", |s: &String, pat: String| {
            s.rmatches(&pat)
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
        })
        .legacy_add_method("match_indices", |s: &String, pat: String| {
            s.match_indices(&pat)
                .map(|(i, c)| {
                    vec![
                        PolarValue::Integer(i as i64),
                        PolarValue::String(c.to_string()),
                    ]
                })
                .collect::<Vec<Vec<PolarValue>>>()
        })
        .legacy_add_method("rmatch_indices", |s: &String, pat: String| {
            s.rmatch_indices(&pat)
                .map(|(i, c)| {
                    vec![
                        PolarValue::Integer(i as i64),
                        PolarValue::String(c.to_string()),
                    ]
                })
                .collect::<Vec<Vec<PolarValue>>>()
        })
        .legacy_add_method("trim", |s: &String| s.trim().to_string())
        .legacy_add_method("trim_start", |s: &String| s.trim_start().to_string())
        .legacy_add_method("trim_end", |s: &String| s.trim_end().to_string())
        .legacy_add_method("is_ascii", |s: &String| s.is_ascii())
        .legacy_add_method("to_lowercase", |s: &String| s.to_lowercase())
        .legacy_add_method("to_uppercase", |s: &String| s.to_uppercase())
        .legacy_add_method("repeat", |s: &String, n: i64| s.repeat(n as usize))
}

/// Returns the builtin types, the name, class, and instance
pub fn classes() -> Vec<Class> {
    vec![
        boolean().build(),
        integer().build(),
        float().build(),
        list().build(),
        dictionary().build(),
        string().build(),
        option().build(),
    ]
}
