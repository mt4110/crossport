use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

pub fn update_env_file(path: &Path, key: &str, value: &str) -> Result<()> {
    let content = fs::read_to_string(path).context("Failed to read .env file")?;

    // Backup
    let backup_path = path.with_file_name(format!(
        "{}.bak",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&backup_path, &content).context("Failed to write backup file")?;
    println!("Backed up .env to {:?}", backup_path);

    let re = Regex::new(&format!(r"(?m)^{}=.*$", key)).unwrap();

    let new_content = if re.is_match(&content) {
        re.replace(&content, format!("{}={}", key, value).as_str())
            .to_string()
    } else {
        format!("{}\n{}={}", content, key, value)
    };

    fs::write(path, new_content).context("Failed to write .env file")?;
    println!("Updated {} in {:?}", key, path);

    Ok(())
}
