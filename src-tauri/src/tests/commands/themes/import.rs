use super::*;
use std::io::Write;

const EXAMPLE: &str = include_str!("../../../../../examples/themes/quiet-orbit/moon-theme.json");

fn package(root: &Path, entries: &[(&str, &str)]) -> PathBuf {
    let path = root.join("sample.moontheme");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, content) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    path
}

#[test]
fn moon_package_roundtrip() {
    let root = tempfile::tempdir().unwrap();
    let themes = root.path().join("themes");
    let input = package(root.path(), &[("wrapper/moon-theme.json", EXAMPLE)]);
    let (entry, stage) = stage_zip(input.to_str().unwrap(), &themes).unwrap();
    let id = entry.id.as_str();
    assert_eq!(id, "quiet-orbit");
    assert_eq!(entry.name.as_str(), "Quiet Orbit");
    let installed = themes.join(id);
    stage.install(&installed).unwrap();
    assert_eq!(
        load_user_theme(&installed, id).unwrap().variables["--border-radius"],
        "12px"
    );
    let output = root.path().join("export.MOONTHEME");
    assert!(has_theme_extension(&output));
    assert!(!has_theme_extension(Path::new("sample.cbth")));
    export_theme_directory(&installed, &output).unwrap();
    let (entry, _) = stage_zip(output.to_str().unwrap(), &themes).unwrap();
    assert_eq!(entry.id.as_str(), id);
}

#[test]
fn unsupported_or_invalid_packages_preserve_installed_files() {
    let root = tempfile::tempdir().unwrap();
    let themes = root.path().join("themes");
    let installed = themes.join("quiet-orbit");
    std::fs::create_dir_all(&installed).unwrap();
    std::fs::write(installed.join(MANIFEST), EXAMPLE).unwrap();
    let unsupported = EXAMPLE.replace("\"format_version\": 1", "\"format_version\": 2");
    for entries in [
        vec![("Meta.toml", "name = 'legacy'")],
        vec![("theme.json", "{}")],
        vec![(MANIFEST, &unsupported)],
        vec![(MANIFEST, EXAMPLE), ("nested/moon-theme.json", EXAMPLE)],
        vec![
            (MANIFEST, EXAMPLE),
            ("assets/file", "a"),
            ("assets/file", "b"),
        ],
    ] {
        let input = package(root.path(), &entries);
        assert!(stage_zip(input.to_str().unwrap(), &themes).is_err());
        assert_eq!(
            std::fs::read_to_string(installed.join(MANIFEST)).unwrap(),
            EXAMPLE
        );
        assert_eq!(std::fs::read_dir(&themes).unwrap().count(), 1);
    }
}

#[test]
fn failed_replacement_rolls_back() {
    let root = tempfile::tempdir().unwrap();
    let installed = root.path().join("quiet-orbit");
    std::fs::create_dir(&installed).unwrap();
    std::fs::write(installed.join(MANIFEST), EXAMPLE).unwrap();
    let stage = StagedTheme::new(root.path()).unwrap();
    std::fs::remove_dir(stage.root.join("theme")).unwrap();
    assert!(stage.install(&installed).is_err());
    assert_eq!(
        std::fs::read_to_string(installed.join(MANIFEST)).unwrap(),
        EXAMPLE
    );
}
