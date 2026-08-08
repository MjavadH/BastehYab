use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use thiserror::Error;

use crate::{
    domain::{operator::Operator, package::InternetPackage},
    normalizers::{validate_package, NormalizationError},
};

pub const CACHE_SCHEMA_VERSION: u32 = 1;
const MAX_CACHE_BYTES: u64 = 5 * 1024 * 1024;
const FRESH_FOR_SECONDS: i64 = 6 * 60 * 60;
const FUTURE_TOLERANCE_SECONDS: i64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorSnapshot {
    pub operator: Operator,
    pub fetched_at_unix_seconds: i64,
    pub stored_at_unix_seconds: i64,
    pub packages: Vec<InternetPackage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSnapshot {
    pub snapshot: OperatorSnapshot,
    pub freshness: CacheFreshness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheEnvelope {
    schema_version: u32,
    operator: Operator,
    fetched_at_unix_seconds: i64,
    stored_at_unix_seconds: i64,
    packages: Vec<InternetPackage>,
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("application cache directory unavailable: {0}")]
    Path(#[from] tauri::Error),
    #[error("cache serialization failure: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported cache schema version {found}, expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error("cache operator mismatch: expected {expected:?}, found {found:?}")]
    OperatorMismatch { expected: Operator, found: Operator },
    #[error("package {package_id} failed validation: {source}")]
    PackageInvalid {
        package_id: String,
        source: NormalizationError,
    },
    #[error("duplicate package id {0}")]
    DuplicatePackageId(String),
    #[error("cache file is too large: {size} bytes")]
    TooLarge { size: u64 },
    #[error("cache timestamp is invalid")]
    InvalidTimestamp,
}

#[derive(Debug, Clone)]
pub struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn for_app(app: &tauri::AppHandle) -> Result<Self, CacheError> {
        Ok(Self::new(app.path().app_cache_dir()?.join("package-cache")))
    }

    pub fn load(
        &self,
        operator: Operator,
        now_unix_seconds: i64,
    ) -> Result<Option<CachedSnapshot>, CacheError> {
        let path = self.path_for(operator);
        if !path.exists() {
            return Ok(None);
        }
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_CACHE_BYTES {
            return Err(CacheError::TooLarge {
                size: metadata.len(),
            });
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(path)?.read_to_end(&mut bytes)?;
        let envelope: CacheEnvelope = serde_json::from_slice(&bytes)?;
        if envelope.schema_version != CACHE_SCHEMA_VERSION {
            return Err(CacheError::UnsupportedSchema {
                found: envelope.schema_version,
                expected: CACHE_SCHEMA_VERSION,
            });
        }
        if envelope.operator != operator {
            return Err(CacheError::OperatorMismatch {
                expected: operator,
                found: envelope.operator,
            });
        }
        let snapshot = OperatorSnapshot {
            operator: envelope.operator,
            fetched_at_unix_seconds: envelope.fetched_at_unix_seconds,
            stored_at_unix_seconds: envelope.stored_at_unix_seconds,
            packages: envelope.packages,
        };
        validate_snapshot(&snapshot)?;
        let freshness = freshness(snapshot.fetched_at_unix_seconds, now_unix_seconds)?;
        Ok(Some(CachedSnapshot {
            snapshot,
            freshness,
        }))
    }

    pub fn load_all(
        &self,
        now_unix_seconds: i64,
    ) -> Vec<(Operator, Result<Option<CachedSnapshot>, CacheError>)> {
        all_operators()
            .into_iter()
            .map(|op| (op, self.load(op, now_unix_seconds)))
            .collect()
    }

    pub fn commit(&self, snapshot: &OperatorSnapshot) -> Result<(), CacheError> {
        validate_snapshot(snapshot)?;
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(snapshot.operator);
        let tmp = self.temp_path_for(snapshot.operator);
        let envelope = CacheEnvelope {
            schema_version: CACHE_SCHEMA_VERSION,
            operator: snapshot.operator,
            fetched_at_unix_seconds: snapshot.fetched_at_unix_seconds,
            stored_at_unix_seconds: snapshot.stored_at_unix_seconds,
            packages: snapshot.packages.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&envelope)?;
        bytes.push(b'\n');
        {
            let mut file = File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &path)?;
        if let Ok(dir) = File::open(&self.root) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn path_for(&self, operator: Operator) -> PathBuf {
        self.root.join(file_name(operator))
    }
    fn temp_path_for(&self, operator: Operator) -> PathBuf {
        self.root.join(format!("{}.tmp", file_name(operator)))
    }
}

pub fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn freshness(fetched_at: i64, now: i64) -> Result<CacheFreshness, CacheError> {
    if fetched_at < 0 || now < 0 || fetched_at > now + FUTURE_TOLERANCE_SECONDS {
        return Err(CacheError::InvalidTimestamp);
    }
    if now.saturating_sub(fetched_at) <= FRESH_FOR_SECONDS {
        Ok(CacheFreshness::Fresh)
    } else {
        Ok(CacheFreshness::Stale)
    }
}

pub fn validate_snapshot(snapshot: &OperatorSnapshot) -> Result<(), CacheError> {
    if snapshot.fetched_at_unix_seconds < 0
        || snapshot.stored_at_unix_seconds < 0
        || snapshot.stored_at_unix_seconds + FUTURE_TOLERANCE_SECONDS
            < snapshot.fetched_at_unix_seconds
    {
        return Err(CacheError::InvalidTimestamp);
    }
    let mut ids = BTreeSet::new();
    for package in &snapshot.packages {
        if package.operator != snapshot.operator {
            return Err(CacheError::OperatorMismatch {
                expected: snapshot.operator,
                found: package.operator,
            });
        }
        validate_package(package).map_err(|source| CacheError::PackageInvalid {
            package_id: package.id.0.clone(),
            source,
        })?;
        if !ids.insert(package.id.clone()) {
            return Err(CacheError::DuplicatePackageId(package.id.0.clone()));
        }
    }
    Ok(())
}

pub fn all_operators() -> [Operator; 4] {
    [
        Operator::Mci,
        Operator::Irancell,
        Operator::Rightel,
        Operator::Samantel,
    ]
}
fn file_name(operator: Operator) -> &'static str {
    match operator {
        Operator::Mci => "mci.json",
        Operator::Irancell => "irancell.json",
        Operator::Rightel => "rightel.json",
        Operator::Samantel => "samantel.json",
    }
}

#[cfg(test)]
pub(crate) fn operator_path_for_tests(root: &Path, operator: Operator) -> PathBuf {
    root.join(file_name(operator))
}
