use super::*;
use crate::commands::themes::moon::MANIFEST;

const EXAMPLE: &str = include_str!("../../../../../examples/themes/quiet-orbit/moon-theme.json");

#[test]
fn lists_only_moon_manifests_with_matching_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    std::fs::write(path.join("theme.json"), "{}").unwrap();
    std::fs::write(path.join("Meta.toml"), "name = 'legacy'").unwrap();
    assert!(list_theme_entry(path, "quiet-orbit").is_none());
    std::fs::write(path.join(MANIFEST), EXAMPLE).unwrap();
    assert!(list_theme_entry(path, "wrong-id").is_none());
    assert_eq!(
        list_theme_entry(path, "quiet-orbit").unwrap().name.as_str(),
        "Quiet Orbit"
    );
}

#[test]
fn validates_manifest_and_referenced_assets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    std::fs::write(path.join(MANIFEST), EXAMPLE).unwrap();
    assert!(validate_theme_at(path, "quiet-orbit").errors.is_empty());
    let mut json: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
    json["background"] = serde_json::json!({"path": "assets/missing.webp"});
    std::fs::write(path.join(MANIFEST), json.to_string()).unwrap();
    assert!(!validate_theme_at(path, "quiet-orbit").errors.is_empty());
}
