//! Value type.
#![allow(clippy::use_self)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Represents a key-value pair.
pub type Map = BTreeMap<String, Value>;

/// Represents any valid value.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// A nil value.
    Nil,
    /// A boolean value.
    Bool(bool),
    /// A number value.
    Number(Number),
    /// A string value.
    String(String),
    /// A list of values.
    Array(Vec<Value>),
    /// A map of values.
    Map(Map),
}

/// Represents a number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Number {
    /// An integer number.
    Int(i64),
    /// A floating point number.
    Float(f64),
}

// For sake of convenience, we implement `Eq` for `Number`.
impl Eq for Number {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::value::{Number, Value};

    #[test]
    fn must_into_value() {
        json_value_eq(json!(null), Value::Nil);
        json_value_eq(json!(true), Value::Bool(true));
        json_value_eq(json!(0), Value::Number(Number::Int(0)));
        json_value_eq(json!(0.1), Value::Number(Number::Float(0.1)));
        json_value_eq(json!("str"), Value::String(String::from("str")));
        json_value_eq(
            json!([1, false]),
            Value::Array(vec![Value::Number(Number::Int(1)), Value::Bool(false)]),
        );
        json_value_eq(
            json!({"a": 1, "b": false}),
            Value::Map(
                vec![
                    ("a".to_string(), Value::Number(Number::Int(1))),
                    ("b".to_string(), Value::Bool(false)),
                ]
                .into_iter()
                .collect(),
            ),
        );
    }

    #[test]
    fn must_refl_by_value() {
        json_refl_by_value(json!(null));
        json_refl_by_value(json!(true));
        json_refl_by_value(json!(0));
        json_refl_by_value(json!(0.1));
        json_refl_by_value(json!("str"));
        json_refl_by_value(json!([1, false]));
        json_refl_by_value(json!({"a": 1, "b": false}));
    }

    #[allow(clippy::needless_pass_by_value)]
    fn json_value_eq(json: serde_json::Value, expected: super::Value) {
        let value: Value = serde_json::from_value(json).unwrap();
        assert_eq!(value, expected);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn json_refl_by_value(json: serde_json::Value) {
        let value: Value = serde_json::from_value(json.clone()).unwrap();
        let output = serde_json::to_value(value).unwrap();
        assert_eq!(output, json);
    }
}
