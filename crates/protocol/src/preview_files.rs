use crate::{Counter, JobId, ProtocolError};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PREVIEW_FILES_DEFAULT_LIMIT: u16 = 50;
pub const PREVIEW_FILES_MAX_LIMIT: u16 = 100;
pub const PREVIEW_FILE_MAX_PATH_BYTES: usize = 4096;
pub const PREVIEW_FILES_MAX_SEARCH_BYTES: usize = 512;
const MAX_FILE_ID_BYTES: usize = 128;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFileSort {
    #[default]
    Path,
    Size,
    CreatedAt,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PreviewFilesQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_to: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orphan: Option<bool>,
    pub search: String,
    pub sort: PreviewFileSort,
    pub direction: SortDirection,
    pub offset: u64,
    pub limit: u16,
}
impl Default for PreviewFilesQuery {
    fn default() -> Self {
        Self {
            created_from: None,
            created_to: None,
            orphan: None,
            search: String::new(),
            sort: PreviewFileSort::Path,
            direction: SortDirection::Asc,
            offset: 0,
            limit: PREVIEW_FILES_DEFAULT_LIMIT,
        }
    }
}
impl PreviewFilesQuery {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.search.len() > PREVIEW_FILES_MAX_SEARCH_BYTES
            || self.search.contains('\0')
            || self.limit == 0
            || self.limit > PREVIEW_FILES_MAX_LIMIT
            || self.offset > i64::MAX as u64
            || matches!((self.created_from, self.created_to), (Some(from), Some(to)) if from > to)
        {
            return Err(ProtocolError::InvalidJob);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewFile {
    pub id: String,
    pub path: String,
    pub size_bytes: Counter,
    pub created_at: DateTime<Utc>,
    pub orphan: bool,
}
impl PreviewFile {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.id.is_empty()
            || self.id.len() > MAX_FILE_ID_BYTES
            || self.id.contains('\0')
            || self.path.is_empty()
            || self.path.len() > PREVIEW_FILE_MAX_PATH_BYTES
            || self.path.contains('\0')
        {
            return Err(ProtocolError::InvalidJob);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewFilesPage {
    pub job_id: JobId,
    pub rows: Vec<PreviewFile>,
    pub total: Counter,
    pub revision: Counter,
    pub complete: bool,
}
impl PreviewFilesPage {
    pub fn validate(&self, query: &PreviewFilesQuery, job_id: JobId) -> Result<(), ProtocolError> {
        query.validate()?;
        if self.job_id != job_id
            || self.rows.len() > usize::from(query.limit)
            || (!self.rows.is_empty()
                && query.offset.saturating_add(self.rows.len() as u64) > self.total.get())
        {
            return Err(ProtocolError::InvalidJob);
        }
        let mut seen = HashSet::new();
        for row in &self.rows {
            row.validate()?;
            if query.created_from.is_some_and(|from| row.created_at < from)
                || query.created_to.is_some_and(|to| row.created_at > to)
                || query.orphan.is_some_and(|orphan| row.orphan != orphan)
            {
                return Err(ProtocolError::InvalidJob);
            }
            if !seen.insert(&row.id) {
                return Err(ProtocolError::InvalidJob);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_duplicate_or_wrong_job_rows_are_rejected() {
        let query = PreviewFilesQuery::default();
        assert!(query.validate().is_ok());
        assert!(PreviewFilesQuery {
            limit: 0,
            ..query.clone()
        }
        .validate()
        .is_err());
        assert!(PreviewFilesQuery {
            offset: u64::MAX,
            ..query.clone()
        }
        .validate()
        .is_err());
        let id = JobId::new();
        let row = PreviewFile {
            id: "file-1".into(),
            path: "C:\\mail\\file.eml".into(),
            size_bytes: 4u64.into(),
            created_at: Utc::now(),
            orphan: false,
        };
        let mut page = PreviewFilesPage {
            job_id: id,
            rows: vec![row.clone()],
            total: 2u64.into(),
            revision: 1u64.into(),
            complete: false,
        };
        assert!(page.validate(&query, id).is_ok());
        assert!(page.validate(&query, JobId::new()).is_err());
        page.rows.push(row);
        assert!(page.validate(&query, id).is_err());
    }
}
