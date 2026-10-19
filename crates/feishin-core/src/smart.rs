//! The same Navidrome all/any rule format used by the Electron query builder.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::sync::OnceLock;
use uuid::Uuid;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Field {
    pub label: String,
    #[serde(alias = "type")]
    pub kind: String,
    pub value: String,
}
pub fn validate_fields(custom: &[Field]) -> Result<()> {
    ensure!(custom.len() <= 256, "Too many custom query fields");
    let mut used = std::collections::HashSet::new();
    for field in custom {
        ensure!(
            !field.label.trim().is_empty() && field.label.len() <= 256,
            "Enter a label for each custom query field"
        );
        ensure!(
            !field.value.trim().is_empty()
                && field.value.len() <= 256
                && !field.value.contains(char::is_control),
            "Enter a tag for each custom query field"
        );
        ensure!(
            used.insert(&field.value),
            "Duplicate custom query tag: {}",
            field.value
        );
        ensure!(
            matches!(
                field.kind.as_str(),
                "boolean" | "date" | "dateRange" | "number" | "playlist" | "string"
            ),
            "Invalid custom query field type"
        );
    }
    Ok(())
}
pub fn combined_fields(custom: &[Field]) -> Vec<Field> {
    let mut combined = fields().to_vec();
    for field in custom {
        if let Some(old) = combined.iter_mut().find(|old| old.value == field.value) {
            *old = field.clone();
        } else {
            combined.push(field.clone());
        }
    }
    combined
}
pub fn field_kind<'a>(field: &str, custom: &'a [Field]) -> &'a str {
    custom
        .iter()
        .find(|definition| definition.value == field)
        .or_else(|| fields().iter().find(|definition| definition.value == field))
        .map_or("string", |field| field.kind.as_str())
}
pub fn fields() -> &'static [Field] {
    static FIELDS: OnceLock<Vec<Field>> = OnceLock::new();
    FIELDS
        .get_or_init(|| serde_json::from_str(include_str!("../assets/smart-fields.json")).unwrap())
}
pub fn kind(field: &str) -> &str {
    field_kind(field, &[])
}
pub fn operators(field: &str) -> &'static [&'static str] {
    operators_for_kind(kind(field))
}
pub fn operators_for_kind(kind: &str) -> &'static [&'static str] {
    match kind {
        "boolean" => &["is", "isNot", "isMissing", "isPresent"],
        "number" => &[
            "is",
            "isNot",
            "isMissing",
            "isPresent",
            "contains",
            "notContains",
            "gt",
            "lt",
            "inTheRange",
        ],
        "date" | "dateRange" => &[
            "is",
            "isNot",
            "isMissing",
            "isPresent",
            "before",
            "after",
            "inTheLast",
            "notInTheLast",
            "inTheRange",
            "beforeDate",
            "afterDate",
            "inTheRangeDate",
        ],
        "playlist" => &["inPlaylist", "notInPlaylist"],
        _ => &[
            "is",
            "isNot",
            "isMissing",
            "isPresent",
            "contains",
            "notContains",
            "startsWith",
            "endsWith",
        ],
    }
}

pub fn operator_label(language: &str, operator: &str) -> String {
    let key = match operator {
        "gt" => "isGreaterThan",
        "lt" => "isLessThan",
        other => other,
    };
    crate::i18n::translate(language, &format!("filterOperator.{key}"))
        .unwrap_or_else(|| operator.to_owned())
}

pub fn api_operator(operator: &str) -> &str {
    match operator {
        "beforeDate" => "before",
        "afterDate" => "after",
        "inTheRangeDate" => "inTheRange",
        other => other,
    }
}

pub fn display_operator<'a>(operator: &'a str, value: &Value) -> &'a str {
    let date = |value: &Value| {
        value.as_str().is_some_and(|value| {
            let value = value.trim().as_bytes();
            value.len() == 10
                && value.iter().enumerate().all(|(index, byte)| {
                    if matches!(index, 4 | 7) {
                        *byte == b'-'
                    } else {
                        byte.is_ascii_digit()
                    }
                })
        })
    };
    match operator {
        "before" if date(value) => "beforeDate",
        "after" if date(value) => "afterDate",
        "inTheRange"
            if value.as_array().is_some_and(|values| {
                values.len() == 2 && values.iter().all(|value| value.is_null() || date(value))
            }) =>
        {
            "inTheRangeDate"
        }
        other => other,
    }
}
#[derive(Clone, Debug)]
pub struct Node {
    pub id: Uuid,
    pub data: Data,
}
#[derive(Clone, Debug)]
pub enum Data {
    Group {
        any: bool,
        children: Vec<Node>,
        extra: Map<String, Value>,
    },
    Rule {
        field: String,
        operator: String,
        value: Value,
    },
}
impl Default for Node {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            data: Data::Group {
                any: false,
                children: vec![],
                extra: Map::new(),
            },
        }
    }
}
impl Node {
    pub fn rule() -> Self {
        Self {
            id: Uuid::new_v4(),
            data: Data::Rule {
                field: "title".into(),
                operator: "contains".into(),
                value: "".into(),
            },
        }
    }
    pub fn parse(value: &Value) -> Result<Self> {
        ensure!(
            value.to_string().len() <= 65_536,
            "Smart playlist rules are too large"
        );
        let mut count = 0;
        Self::parse_at(value, 0, &mut count)
    }
    fn parse_at(value: &Value, depth: usize, count: &mut usize) -> Result<Self> {
        *count += 1;
        ensure!(
            depth <= 8 && *count <= 128,
            "Too many nested smart playlist rules"
        );
        let object = value.as_object().context("A rule must be an object")?;
        let data = if object.contains_key("all") || object.contains_key("any") {
            ensure!(
                !(object.contains_key("all") && object.contains_key("any")),
                "Choose all or any in each group"
            );
            let any = object.contains_key("any");
            let children = object[if any { "any" } else { "all" }]
                .as_array()
                .context("A group must contain an array of rules")?
                .iter()
                .map(|value| Self::parse_at(value, depth + 1, count))
                .collect::<Result<_>>()?;
            let extra = object
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "all" | "any"))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            Data::Group {
                any,
                children,
                extra,
            }
        } else {
            ensure!(object.len() == 1, "Use one operator per rule");
            let (operator, value) = object.iter().next().unwrap();
            let values = value.as_object().context("A rule needs a field")?;
            ensure!(values.len() == 1, "Use one field per rule");
            let (field, value) = values.iter().next().unwrap();
            Data::Rule {
                field: field.clone(),
                operator: operator.clone(),
                value: value.clone(),
            }
        };
        Ok(Self {
            id: Uuid::new_v4(),
            data,
        })
    }
    pub fn json(&self) -> Value {
        match &self.data {
            Data::Group {
                any,
                children,
                extra,
            } => {
                let mut value = extra.clone();
                value.insert(
                    if *any { "any" } else { "all" }.into(),
                    children.iter().map(Self::json).collect(),
                );
                Value::Object(value)
            }
            Data::Rule {
                field,
                operator,
                value,
            } => json!({api_operator(operator):{field:value}}),
        }
    }
    pub fn find_mut(&mut self, id: Uuid) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        if let Data::Group { children, .. } = &mut self.data {
            return children.iter_mut().find_map(|node| node.find_mut(id));
        }
        None
    }
    pub fn remove(&mut self, id: Uuid) {
        if let Data::Group { children, .. } = &mut self.data {
            children.retain(|node| node.id != id);
            for node in children {
                node.remove(id);
            }
        }
    }
    pub fn apply_text(&mut self, text: &[String]) -> Result<()> {
        self.apply_text_with_fields(text, &[])
    }
    pub fn apply_text_with_fields(&mut self, text: &[String], custom: &[Field]) -> Result<()> {
        if let Data::Rule {
            field,
            operator,
            value,
        } = &mut self.data
        {
            let scalar = |text: &str, index: usize| -> Result<Value> {
                ensure!(text.len() <= 8192, "Rule value is too long");
                if matches!(operator.as_str(), "isMissing" | "isPresent")
                    || field_kind(field, custom) == "boolean"
                {
                    match text {
                        "true" => Ok(true.into()),
                        "false" => Ok(false.into()),
                        _ => anyhow::bail!("Choose true or false"),
                    }
                } else if matches!(
                    operator.as_str(),
                    "beforeDate" | "afterDate" | "inTheRangeDate"
                ) {
                    let previous = if operator == "inTheRangeDate" {
                        value.get(index)
                    } else {
                        Some(&*value)
                    };
                    if previous.and_then(Value::as_str) == Some(text) {
                        // Preserve an imported value until the user actually edits it.
                        Ok(text.into())
                    } else if text.trim().is_empty() {
                        Ok(if operator == "inTheRangeDate" {
                            Value::Null
                        } else {
                            "".into()
                        })
                    } else {
                        Ok(chrono::NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
                            .context("Enter a valid date (YYYY-MM-DD)")?
                            .format("%Y-%m-%d")
                            .to_string()
                            .into())
                    }
                } else if field_kind(field, custom) == "number"
                    || (operator == "inTheRange"
                        && matches!(field_kind(field, custom), "date" | "dateRange"))
                {
                    let number = text
                        .parse::<f64>()
                        .context("Enter a number for this rule")?;
                    ensure!(number.is_finite(), "Enter a finite number");
                    if number == 0.0 {
                        Ok(json!(0))
                    } else {
                        Ok(serde_json::from_str(&number.to_string())?)
                    }
                } else {
                    Ok(text.into())
                }
            };
            *value = if matches!(operator.as_str(), "inTheRange" | "inTheRangeDate") {
                ensure!(text.len() == 2, "Enter both range endpoints");
                Value::Array(
                    text.iter()
                        .enumerate()
                        .map(|(index, text)| scalar(text, index))
                        .collect::<Result<_>>()?,
                )
            } else if value.is_object() || (value.is_array() && operator != "inTheRange") {
                serde_json::from_str(text.first().context("Enter a value")?)
                    .context("Invalid rule value")?
            } else {
                scalar(text.first().context("Enter a value")?, 0)?
            };
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_operators_and_dates_match_original() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/smart-editor-reference.json")).unwrap();
        for case in reference["dates"].as_array().unwrap() {
            assert_eq!(
                display_operator(case["operator"].as_str().unwrap(), &case["value"]),
                case["display"].as_str().unwrap(),
                "{case}"
            );
        }
        for (operator, expected) in reference["api"].as_object().unwrap() {
            assert_eq!(api_operator(operator), expected.as_str().unwrap());
        }
        for (language, kinds) in reference["locales"].as_object().unwrap() {
            for (kind, entries) in kinds.as_object().unwrap() {
                let entries = entries.as_array().unwrap();
                assert_eq!(
                    operators_for_kind(kind),
                    entries
                        .iter()
                        .map(|entry| entry["value"].as_str().unwrap())
                        .collect::<Vec<_>>()
                );
                for entry in entries {
                    assert_eq!(
                        operator_label(language, entry["value"].as_str().unwrap()),
                        entry["label"]
                    );
                }
            }
        }
        let mut range =
            Node::parse(&json!({"inTheRangeDate":{"dateadded":[null,"2024-01-01"]}})).unwrap();
        range
            .apply_text(&[String::new(), "2024-02-29".into()])
            .unwrap();
        assert_eq!(
            range.json(),
            json!({"inTheRange":{"dateadded":[null,"2024-02-29"]}})
        );
        let saved = range.json();
        assert!(
            range
                .apply_text(&[String::new(), "2023-02-29".into()])
                .is_err()
        );
        assert_eq!(range.json(), saved);
        let mut imported = Node::parse(&json!({"beforeDate":{"dateadded":"2024-99-99"}})).unwrap();
        imported.apply_text(&["2024-99-99".into()]).unwrap();
        assert_eq!(
            imported.json(),
            json!({"before":{"dateadded":"2024-99-99"}})
        );
        imported.apply_text(&["2024-2-29".into()]).unwrap();
        assert_eq!(
            imported.json(),
            json!({"before":{"dateadded":"2024-02-29"}})
        );
        let mut relative = Node::parse(&json!({"inTheRange":{"dateadded":[30,90]}})).unwrap();
        relative.apply_text(&["10".into(), "45".into()]).unwrap();
        assert_eq!(relative.json(), json!({"inTheRange":{"dateadded":[10,45]}}));
    }

    #[test]
    fn nested_rule_round_trips_preserve_boolean_ranges_sort_and_unknown_metadata() {
        let value = json!({"all":[{"contains":{"title":"Live"}},{"any":[{"is":{"loved":true}},{"inTheRange":{"year":[1990,2020]}},{"before":{"dateadded":"2025-01-01"}}]}],"sort":"+dateAdded,-rating","limitPercent":30,"extension":{"future":true}});
        let mut rules = Node::parse(&value).unwrap();
        assert_eq!(rules.json(), value);
        if let Data::Group { children, .. } = &mut rules.data {
            children[0].apply_text(&["Studio".into()]).unwrap();
            if let Data::Group { children, .. } = &mut children[1].data {
                children[0].apply_text(&["false".into()]).unwrap();
                children[1]
                    .apply_text(&["2000".into(), "2024".into()])
                    .unwrap();
                assert!(
                    children[1]
                        .apply_text(&["NaN".into(), "2024".into()])
                        .is_err()
                );
            }
        }
        assert_eq!(rules.json()["all"][1]["any"][0]["is"]["loved"], false);
        assert_eq!(
            rules.json()["all"][1]["any"][1]["inTheRange"]["year"],
            json!([2000, 2024])
        );
        assert_eq!(rules.json()["extension"]["future"], true);
        assert!(fields().len() > 100);
        assert!(fields().iter().any(|field| field.value == "work"));
        let mut too_deep = json!({"all":[]});
        for _ in 0..10 {
            too_deep = json!({"all":[too_deep]});
        }
        assert!(Node::parse(&too_deep).is_err());
        assert!(Node::parse(&json!({"any":[],"all":[]})).is_err());
    }
}

#[cfg(test)]
mod custom_tests {
    use super::*;
    #[test]
    fn imported_custom_fields_keep_types_operators_and_unknown_rules() {
        let custom: Vec<Field> = serde_json::from_value(json!([
            {"label":"Mood score","type":"number","value":"moodscore"},
            {"label":"Reviewed","type":"boolean","value":"reviewed"},
            {"label":"Published range","type":"dateRange","value":"published"}
        ]))
        .unwrap();
        validate_fields(&custom).unwrap();
        assert!(operators_for_kind(field_kind("moodscore", &custom)).contains(&"gt"));
        assert!(operators_for_kind(field_kind("published", &custom)).contains(&"inTheRange"));
        let mut number = Node::parse(&json!({"inTheRange":{"moodscore":[1,5]}})).unwrap();
        number
            .apply_text_with_fields(&["2.5".into(), "9".into()], &custom)
            .unwrap();
        assert_eq!(number.json(), json!({"inTheRange":{"moodscore":[2.5,9]}}));
        assert!(
            number
                .apply_text_with_fields(&["NaN".into(), "9".into()], &custom)
                .is_err()
        );
        let mut boolean = Node::parse(&json!({"is":{"reviewed":true}})).unwrap();
        boolean
            .apply_text_with_fields(&["false".into()], &custom)
            .unwrap();
        assert_eq!(boolean.json()["is"]["reviewed"], false);
        assert!(
            boolean
                .apply_text_with_fields(&["no".into()], &custom)
                .is_err()
        );
        let mut unknown = Node::parse(&json!({"future":{"unknown":{"object":1}}})).unwrap();
        unknown
            .apply_text_with_fields(&[r#"{"object":2}"#.into()], &custom)
            .unwrap();
        assert_eq!(unknown.json()["future"]["unknown"]["object"], 2);
        assert_eq!(combined_fields(&custom).len(), fields().len() + 3);
        let mut repeated = custom.clone();
        repeated.push(custom[0].clone());
        assert!(validate_fields(&repeated).is_err());
        repeated[3].value = "\0".into();
        assert!(validate_fields(&repeated).is_err());
        let imported = crate::migration::preview(&crate::storage::Settings::default(), &serde_json::to_vec(&json!({"general":{},"queryBuilder":{"tag":custom},"playback":{"transcode":{"maxSampleRate":96000}}})).unwrap()).unwrap();
        assert_eq!(imported.settings.query_fields[0].kind, "number");
        assert_eq!(imported.settings.max_sample_rate, Some(96000));
        assert!(imported.omitted.is_empty(), "{:?}", imported.omitted);
    }
}
