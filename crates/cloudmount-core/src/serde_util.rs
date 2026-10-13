//! 反序列化辅助：115 的字段类型不稳定（同一字段在不同接口/版本里可能是
//! 字符串或数字），这里提供统一的宽松解析器。

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

/// 把"数字或数字字符串"解析为 `u64`（缺失/空串视为 0）。
pub fn num_or_str_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl Visitor<'_> for V {
        type Value = u64;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("数字或数字字符串")
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u64, E> {
            Ok(v.max(0) as u64)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<u64, E> {
            Ok(v.max(0.0) as u64)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            Ok(v.trim().parse::<u64>().unwrap_or(0))
        }
        fn visit_none<E: de::Error>(self) -> Result<u64, E> {
            Ok(0)
        }
        fn visit_unit<E: de::Error>(self) -> Result<u64, E> {
            Ok(0)
        }
    }
    deserializer.deserialize_any(V)
}

/// 可选的"数字或数字字符串"（None / 空 / 非法 → None）。
pub fn opt_num_or_str_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => n.as_u64(),
        Some(serde_json::Value::String(s)) => s.trim().parse::<u64>().ok(),
        _ => None,
    })
}

/// `Option<i64>`：数字或数字字符串（115 部分接口用空串/字符串码）。
pub fn opt_num_or_str_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => n.as_i64(),
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                None
            } else {
                s.parse().ok()
            }
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Sizes {
        #[serde(deserialize_with = "num_or_str_u64")]
        a: u64,
        #[serde(deserialize_with = "num_or_str_u64")]
        b: u64,
        #[serde(default, deserialize_with = "opt_num_or_str_u64")]
        c: Option<u64>,
    }

    #[test]
    fn mixed_types() {
        let s: Sizes = serde_json::from_str(r#"{"a": 123, "b": "456", "c": null}"#).unwrap();
        assert_eq!(s.a, 123);
        assert_eq!(s.b, 456);
        assert_eq!(s.c, None);
    }

    #[test]
    fn string_number_option() {
        let s: Sizes = serde_json::from_str(r#"{"a":"0","b":0,"c":"789"}"#).unwrap();
        assert_eq!(s.c, Some(789));
    }
}
