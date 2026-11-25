use super::moon::MoonTheme;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ThemeValidationReport {
    pub id: String,
    pub name: Option<String>,
    pub format: &'static str,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub(super) fn validate_theme_at(dir: &Path, id: &str) -> ThemeValidationReport {
    let mut report = ThemeValidationReport {
        id: id.into(),
        name: None,
        format: "moonlauncher/1",
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    match MoonTheme::read(dir, id) {
        Ok(theme) => {
            report.name = Some(theme.name.clone());
            if let Err(e) = theme.resolve(dir) {
                report.errors.push(e);
            }
        }
        Err(e) => report.errors.push(e),
    }
    report
}
