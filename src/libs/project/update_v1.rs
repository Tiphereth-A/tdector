use std::collections::HashMap;

use serde_json::{Value, json};

use crate::enums::{AppError, AppResult};

pub fn migrate_v1_to_v2(mut value: Value) -> AppResult<Value> {
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            AppError::InvalidProjectFormat("Missing or invalid version field".to_string())
        })?;

    if version != 1 {
        return Err(AppError::InvalidProjectFormat(format!(
            "Expected version 1 for migration, got {version}"
        )));
    }

    let vocabulary = value
        .get("vocabulary")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            AppError::InvalidProjectFormat("Missing or invalid vocabulary array".to_string())
        })?;

    let mut formatted_entries = value
        .get("formatted_word")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut formatted_index_map: HashMap<Vec<usize>, usize> = HashMap::new();
    for (idx, entry) in formatted_entries.iter().enumerate() {
        let key = parse_word_indices(entry.get("word"))?;
        formatted_index_map.entry(key).or_insert(idx);
    }

    let sentences = value
        .get_mut("sentences")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            AppError::InvalidProjectFormat("Missing or invalid sentences array".to_string())
        })?;

    for sentence in sentences.iter_mut() {
        let words = sentence
            .get_mut("words")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                AppError::InvalidProjectFormat(
                    "Missing or invalid sentence words array".to_string(),
                )
            })?;

        let mut migrated_words: Vec<Value> = Vec::with_capacity(words.len());
        for word in words.iter() {
            match word {
                Value::Number(num) => {
                    let idx = num
                        .as_i64()
                        .or_else(|| num.as_u64().map(|v| v as i64))
                        .ok_or_else(|| {
                            AppError::InvalidProjectFormat(
                                "Invalid word index in sentence".to_string(),
                            )
                        })?;
                    migrated_words.push(Value::Number(idx.into()));
                }
                Value::Array(_) => {
                    let indices = parse_word_indices(Some(word))?;

                    if let Some(&existing_idx) = formatted_index_map.get(&indices) {
                        let signed_idx = -((existing_idx as i64) + 1);
                        migrated_words.push(Value::Number(signed_idx.into()));
                    } else {
                        let new_idx = formatted_entries.len();
                        formatted_entries.push(json!({
                            "word": indices,
                            "comment": ""
                        }));
                        formatted_index_map.insert(indices.clone(), new_idx);
                        let signed_idx = -((new_idx as i64) + 1);
                        migrated_words.push(Value::Number(signed_idx.into()));
                    }
                }
                _ => {
                    return Err(AppError::InvalidProjectFormat(
                        "Invalid word entry in sentence".to_string(),
                    ));
                }
            }
        }

        *words = migrated_words;
    }

    value
        .as_object_mut()
        .ok_or_else(|| AppError::InvalidProjectFormat("Invalid project root".to_string()))?
        .remove("formatted_word");

    value["vocabulary"] = json!({
        "orignal": vocabulary,
        "formatted": formatted_entries,
    });

    value["version"] = Value::Number(2.into());

    Ok(value)
}

fn parse_word_indices(word_value: Option<&Value>) -> AppResult<Vec<usize>> {
    let array = word_value.and_then(Value::as_array).ok_or_else(|| {
        AppError::InvalidProjectFormat("Invalid formatted word indices".to_string())
    })?;

    array
        .iter()
        .map(|v| {
            v.as_u64().map(|idx| idx as usize).ok_or_else(|| {
                AppError::InvalidProjectFormat("Invalid formatted word index".to_string())
            })
        })
        .collect()
}
