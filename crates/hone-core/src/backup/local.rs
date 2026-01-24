//! Local filesystem backup destination

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tracing::info;

use super::{parse_backup_time, BackupDestination, BackupInfo};
use crate::error::{Error, Result};

/// Local filesystem backup destination
pub struct LocalDestination {
    /// Directory where backups are stored
    backup_dir: PathBuf,
}

impl LocalDestination {
    /// Create a new local destination
    ///
    /// Creates the backup directory if it doesn't exist.
    pub fn new(backup_dir: impl Into<PathBuf>) -> Result<Self> {
        let backup_dir = backup_dir.into();

        // Create directory if it doesn't exist
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).map_err(|e| {
                Error::Backup(format!(
                    "Failed to create backup directory {}: {}",
                    backup_dir.display(),
                    e
                ))
            })?;
            info!("Created backup directory: {}", backup_dir.display());
        }

        Ok(Self { backup_dir })
    }

    /// Get the full path for a backup name
    fn backup_path(&self, name: &str) -> PathBuf {
        self.backup_dir.join(name)
    }

    /// Get the backup directory path
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }
}

impl BackupDestination for LocalDestination {
    fn name(&self) -> &str {
        "local"
    }

    fn store(&self, local_path: &Path, backup_name: &str) -> Result<String> {
        let dest_path = self.backup_path(backup_name);

        if dest_path.exists() {
            return Err(Error::Backup(format!(
                "Backup already exists: {}",
                dest_path.display()
            )));
        }

        // Check if source is already gzipped
        let is_gzipped = backup_name.ends_with(".gz");

        if is_gzipped && !local_path.to_string_lossy().ends_with(".gz") {
            // Compress while copying
            let source = File::open(local_path)?;
            let reader = BufReader::new(source);

            let dest = File::create(&dest_path)?;
            let writer = BufWriter::new(dest);
            let mut encoder = GzEncoder::new(writer, Compression::default());

            let mut reader = reader;
            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = reader.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                encoder.write_all(&buffer[..bytes_read])?;
            }
            encoder.finish()?;
        } else {
            // Direct copy
            fs::copy(local_path, &dest_path)?;
        }

        info!("Stored backup: {}", dest_path.display());
        Ok(backup_name.to_string())
    }

    fn retrieve(&self, backup_name: &str, local_path: &Path) -> Result<()> {
        let source_path = self.backup_path(backup_name);

        if !source_path.exists() {
            return Err(Error::Backup(format!(
                "Backup not found: {}",
                source_path.display()
            )));
        }

        // Check if we need to decompress
        let is_gzipped = backup_name.ends_with(".gz");
        let dest_wants_raw = !local_path.to_string_lossy().ends_with(".gz");

        if is_gzipped && dest_wants_raw {
            // Decompress while copying
            let source = File::open(&source_path)?;
            let reader = BufReader::new(source);
            let mut decoder = GzDecoder::new(reader);

            let dest = File::create(local_path)?;
            let mut writer = BufWriter::new(dest);

            let mut buffer = [0u8; 8192];
            loop {
                let bytes_read = decoder.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                writer.write_all(&buffer[..bytes_read])?;
            }
            writer.flush()?;
        } else {
            // Direct copy
            fs::copy(&source_path, local_path)?;
        }

        info!("Retrieved backup to: {}", local_path.display());
        Ok(())
    }

    fn list(&self) -> Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only include hone backup files
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) if name.starts_with("hone-") => name.to_string(),
                _ => continue,
            };

            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }

            let created_at = parse_backup_time(&file_name).unwrap_or_else(Utc::now);

            let compressed = file_name.ends_with(".gz");
            // Assume encrypted if created by our backup system
            let encrypted = true;

            backups.push(BackupInfo {
                name: file_name,
                path: path.to_string_lossy().to_string(),
                size: metadata.len(),
                created_at,
                compressed,
                encrypted,
            });
        }

        // Sort by creation time, newest first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    fn delete(&self, backup_name: &str) -> Result<()> {
        let path = self.backup_path(backup_name);

        if !path.exists() {
            return Err(Error::Backup(format!(
                "Backup not found: {}",
                path.display()
            )));
        }

        fs::remove_file(&path)?;
        info!("Deleted backup: {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_destination() -> (TempDir, LocalDestination) {
        let dir = TempDir::new().unwrap();
        let dest = LocalDestination::new(dir.path().join("backups")).unwrap();
        (dir, dest)
    }

    #[test]
    fn test_new_creates_directory() {
        let dir = TempDir::new().unwrap();
        let backup_dir = dir.path().join("new_backups");
        assert!(!backup_dir.exists());

        let _dest = LocalDestination::new(&backup_dir).unwrap();
        assert!(backup_dir.exists());
    }

    #[test]
    fn test_store_and_retrieve() {
        let (_dir, dest) = setup_test_destination();

        // Create a test file
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), b"test data for backup").unwrap();

        // Store it
        let backup_name = "hone-2024-01-15-120000.db.gz";
        dest.store(temp_file.path(), backup_name).unwrap();

        // Verify it exists
        let backups = dest.list().unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].name, backup_name);

        // Retrieve it
        let restore_path = dest.backup_dir().parent().unwrap().join("restored.db");
        dest.retrieve(backup_name, &restore_path).unwrap();
        assert!(restore_path.exists());
    }

    #[test]
    fn test_list_empty() {
        let (_dir, dest) = setup_test_destination();
        let backups = dest.list().unwrap();
        assert!(backups.is_empty());
    }

    #[test]
    fn test_delete() {
        let (_dir, dest) = setup_test_destination();

        // Create a test file
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), b"test data").unwrap();

        let backup_name = "hone-2024-01-15-120000.db.gz";
        dest.store(temp_file.path(), backup_name).unwrap();

        assert_eq!(dest.list().unwrap().len(), 1);

        dest.delete(backup_name).unwrap();

        assert_eq!(dest.list().unwrap().len(), 0);
    }

    #[test]
    fn test_delete_nonexistent() {
        let (_dir, dest) = setup_test_destination();
        let result = dest.delete("nonexistent.db.gz");
        assert!(result.is_err());
    }
}
