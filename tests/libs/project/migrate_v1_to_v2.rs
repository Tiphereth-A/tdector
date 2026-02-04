use serde_json::Value;
use std::fs;

const INPUT: &str = "tests/libs/project/sample/migrate_v1_to_v2.json";
const EXPECTED: &str = "tests/libs/project/sample/migrate_v1_to_v2.expected.json";

#[test]
fn test_roundtrip_v1_to_v2() {
    let v1_content = fs::read_to_string(INPUT).expect("Failed to read INPUT");
    let v1_value: Value = serde_json::from_str(&v1_content).expect("Failed to parse INPUT");

    let project = tdector::libs::project::load_project_from_json(v1_value)
        .expect("Failed to load and convert v1 JSON to Project");

    let saved_project = tdector::libs::project::convert_to_saved_project(&project)
        .expect("Failed to convert Project back to SavedProjectV2");

    let saved_json =
        serde_json::to_value(&saved_project).expect("Failed to serialize SavedProjectV2 to JSON");

    let expected_content = fs::read_to_string(EXPECTED).expect("Failed to read EXPECTED");
    let expected_value: Value =
        serde_json::from_str(&expected_content).expect("Failed to parse EXPECTED");

    assert_eq!(
        saved_json, expected_value,
        "Serialized result does not match expected output"
    );
}

#[test]
fn test_sample_files_exist() {
    let v1_content = fs::read_to_string(INPUT).expect("INPUT should exist");
    serde_json::from_str::<Value>(&v1_content).expect("INPUT should be valid JSON");

    let v2_content = fs::read_to_string(EXPECTED).expect("EXPECTED should exist");
    serde_json::from_str::<Value>(&v2_content).expect("EXPECTED should be valid JSON");
}

#[test]
fn test_v1_file_version() {
    let content = fs::read_to_string(INPUT).expect("Failed to read INPUT");
    let value: Value = serde_json::from_str(&content).expect("Failed to parse INPUT");
    assert_eq!(value["version"], 1, "INPUT should have version=1");
}

#[test]
fn test_v2_file_version() {
    let content = fs::read_to_string(EXPECTED).expect("Failed to read EXPECTED");
    let value: Value = serde_json::from_str(&content).expect("Failed to parse EXPECTED");
    assert_eq!(value["version"], 2, "EXPECTED should have version=2");
}
