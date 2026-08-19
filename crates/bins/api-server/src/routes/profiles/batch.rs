//! Batch import/export of staff profiles. Requested 2026-08-13:
//! "capability to import or export staff list profiles by admin access
//! as a batch for an organization, whole staff members as once."
//!
//! # Confirmed spec (see `DECISIONS.md` "Staff Profiles — batch
//! import/export")
//! - Formats: CSV and JSON, both directions.
//! - Mandatory row fields: `user_id`, `full_name`, `contact_email`,
//!   `contact_phone`, `job_title`, `department`. A row missing any of
//!   these is rejected.
//! - `user_id` must reference an account that **already exists**
//!   (created via the normal admin user-creation flow) — import does
//!   not create accounts, only profiles.
//! - Upsert: a row whose `user_id` matches an existing profile updates
//!   it; an unmatched `user_id` creates a new profile.
//! - Row-level failures are skipped, not batch-fatal: the rest of the
//!   batch still imports, and every failure is reported with a reason.
//! - Admin-only, no exceptions.

use axum::{
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use super::{
    domain_error, fetch_staff_profile_rows, find_by_owner, infrastructure_error, to_public_dto,
    upsert_profile, PublicProfileDto, UpsertProfileRequest,
};
use crate::routes::admin::require_admin;
use crate::routes::{ApiError, ApiState};

/// One row of the import/export file format, before/after JSON or CSV
/// (de)serialization. Field names match the file's column headers
/// (CSV) or object keys (JSON) exactly — deliberately the wire format
/// itself, not reused from `UpsertProfileRequest`, since import rows
/// carry the mandatory-field validation semantics
/// (`ImportRow::validate`) that a plain upsert request does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportExportRow {
    pub user_id: String,
    pub full_name: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub job_title: String,
    pub department: String,
    #[serde(default)]
    pub photo_blob_key: Option<String>,
    #[serde(default)]
    pub class_label: Option<String>,
    #[serde(default)]
    pub parent_display_name: Option<String>,
    /// CSV has no native list type — encoded as a single field with
    /// `;`-separated values on export, and accepted in the same form on
    /// import. JSON export/import instead uses this crate's native
    /// `Vec<String>` (`serde`'s default `Vec` (de)serialization), which
    /// is why this field is not itself the CSV-joined string — the CSV
    /// reader/writer joins/splits it at the row (de)serialization
    /// boundary, not here.
    #[serde(default)]
    pub team_memberships: Vec<String>,
}

impl ImportExportRow {
    /// Validates the confirmed mandatory fields, returning a
    /// human-readable reason string on the first failure found (not
    /// every failure — one clear reason per rejected row was judged
    /// sufficient; a row missing three fields doesn't need three
    /// separate complaints to be useful to the person fixing the file).
    fn validate(&self) -> Result<(), String> {
        if self.user_id.trim().is_empty() {
            return Err("user_id is required".to_string());
        }
        if uuid::Uuid::parse_str(self.user_id.trim()).is_err() {
            return Err(format!("user_id is not a valid UUID: {}", self.user_id));
        }
        if self.full_name.trim().is_empty() {
            return Err("full_name is required".to_string());
        }
        if self.contact_email.trim().is_empty() {
            return Err("contact_email is required".to_string());
        }
        if self.contact_phone.trim().is_empty() {
            return Err("contact_phone is required".to_string());
        }
        if self.job_title.trim().is_empty() {
            return Err("job_title is required".to_string());
        }
        if self.department.trim().is_empty() {
            return Err("department is required".to_string());
        }
        Ok(())
    }

    fn into_upsert_request(self) -> UpsertProfileRequest {
        UpsertProfileRequest {
            owner_id: self.user_id,
            full_name: self.full_name,
            photo_blob_key: self.photo_blob_key,
            job_title: Some(self.job_title),
            contact_email: Some(self.contact_email),
            contact_phone: Some(self.contact_phone),
            department: Some(self.department),
            class_label: self.class_label,
            parent_display_name: self.parent_display_name,
            team_memberships: self.team_memberships,
        }
    }
}

/// One row's outcome — success (created or updated) or failure (with
/// why). Every row in the batch gets exactly one of these, in the same
/// order as the input, so the caller can match failures back to their
/// original position in the file.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RowResult {
    Created {
        user_id: String,
    },
    Updated {
        user_id: String,
    },
    Failed {
        user_id: Option<String>,
        reason: String,
    },
}

#[derive(Debug, Serialize)]
pub struct ImportSummary {
    pub total_rows: usize,
    pub created: usize,
    pub updated: usize,
    pub failed: usize,
    pub results: Vec<RowResult>,
}

/// Parses `body` as either CSV or JSON based on `content_type`. JSON is
/// expected to be an array of `ImportExportRow` objects; CSV uses
/// headers matching `ImportExportRow`'s field names (`;`-joined for
/// `team_memberships`, per that field's own doc comment).
fn parse_rows(body: &[u8], content_type: &str) -> Result<Vec<ImportExportRow>, ApiError> {
    if content_type.contains("json") {
        serde_json::from_slice(body)
            .map_err(|e| domain_error(format!("invalid JSON import file: {e}")))
    } else {
        // Default to CSV for anything not explicitly JSON — matches
        // this route's own accepted-formats contract (CSV and JSON
        // only) rather than guessing at other content types.
        let mut reader = csv::ReaderBuilder::new().from_reader(body);
        let mut rows = Vec::new();
        for record in reader.deserialize::<CsvRow>() {
            let record = record.map_err(|e| domain_error(format!("invalid CSV row: {e}")))?;
            rows.push(record.into());
        }
        Ok(rows)
    }
}

/// CSV-specific row shape: `team_memberships` is a single
/// `;`-delimited string field here (CSV has no native list type), split
/// into `ImportExportRow::team_memberships` on conversion.
#[derive(Debug, Deserialize, Serialize)]
struct CsvRow {
    user_id: String,
    full_name: String,
    contact_email: String,
    contact_phone: String,
    job_title: String,
    department: String,
    #[serde(default)]
    photo_blob_key: String,
    #[serde(default)]
    class_label: String,
    #[serde(default)]
    parent_display_name: String,
    #[serde(default)]
    team_memberships: String,
}

fn empty_to_none(s: String) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

impl From<CsvRow> for ImportExportRow {
    fn from(row: CsvRow) -> Self {
        Self {
            user_id: row.user_id,
            full_name: row.full_name,
            contact_email: row.contact_email,
            contact_phone: row.contact_phone,
            job_title: row.job_title,
            department: row.department,
            photo_blob_key: empty_to_none(row.photo_blob_key),
            class_label: empty_to_none(row.class_label),
            parent_display_name: empty_to_none(row.parent_display_name),
            team_memberships: row
                .team_memberships
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }
}

impl From<&PublicProfileDto> for CsvRow {
    fn from(dto: &PublicProfileDto) -> Self {
        Self {
            user_id: dto.owner_id.clone(),
            full_name: dto.full_name.clone(),
            contact_email: dto.contact_email.clone().unwrap_or_default(),
            contact_phone: dto.contact_phone.clone().unwrap_or_default(),
            job_title: dto.job_title.clone().unwrap_or_default(),
            department: dto.department.clone().unwrap_or_default(),
            photo_blob_key: dto.photo_blob_key.clone().unwrap_or_default(),
            class_label: dto.class_label.clone().unwrap_or_default(),
            parent_display_name: dto.parent_display_name.clone().unwrap_or_default(),
            team_memberships: dto.team_memberships.join(";"),
        }
    }
}

/// `POST /api/admin/profiles/import` — admin-only batch import.
/// Multipart form upload with a single `file` field; `Content-Type` of
/// that field's own part determines CSV vs. JSON parsing (falls back to
/// CSV if unspecified/ambiguous — see `parse_rows`).
pub async fn import_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ImportSummary>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&admin.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let admin_uuid =
        uuid::Uuid::parse_str(&admin.user_id).map_err(|_| domain_error("invalid admin user id"))?;
    let actor = platform_kernel::ObjectId(*admin_uuid.as_bytes());

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| domain_error(format!("invalid multipart upload: {e}")))?
    {
        if field.name() == Some("file") {
            content_type = field.content_type().unwrap_or_default().to_string();
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| domain_error(format!("failed to read upload: {e}")))?
                    .to_vec(),
            );
        }
    }
    let file_bytes = file_bytes.ok_or_else(|| domain_error("no 'file' field in upload"))?;

    let rows = parse_rows(&file_bytes, &content_type)?;

    let mut results = Vec::with_capacity(rows.len());
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut failed = 0usize;

    for row in rows {
        let user_id_for_report = if row.user_id.trim().is_empty() {
            None
        } else {
            Some(row.user_id.clone())
        };

        if let Err(reason) = row.validate() {
            failed += 1;
            results.push(RowResult::Failed {
                user_id: user_id_for_report,
                reason,
            });
            continue;
        }

        // user_id must reference an already-existing account — import
        // does not create accounts (confirmed 2026-08-13). Checked here
        // per row rather than relying on the FK-style check that would
        // happen inside upsert_profile's own commit path, so the
        // failure reason is specific ("no such user") rather than a
        // generic infrastructure error.
        match state.user_store.find_by_id(&row.user_id).await {
            Ok(None) => {
                failed += 1;
                results.push(RowResult::Failed {
                    user_id: user_id_for_report,
                    reason: format!("no account exists for user_id {}", row.user_id),
                });
                continue;
            }
            Err(e) => {
                failed += 1;
                results.push(RowResult::Failed {
                    user_id: user_id_for_report,
                    reason: format!("user lookup failed: {e}"),
                });
                continue;
            }
            Ok(Some(_)) => {}
        }

        let already_existed = find_by_owner(&state, organization_id, &row.user_id)
            .await
            .ok()
            .flatten()
            .is_some();

        let owner_id = row.user_id.clone();
        match upsert_profile(&state, &actor, organization_id, row.into_upsert_request()).await {
            Ok(_) => {
                if already_existed {
                    updated += 1;
                    results.push(RowResult::Updated { user_id: owner_id });
                } else {
                    created += 1;
                    results.push(RowResult::Created { user_id: owner_id });
                }
            }
            Err(e) => {
                failed += 1;
                results.push(RowResult::Failed {
                    user_id: Some(owner_id),
                    reason: e.body.safe_details.to_string(),
                });
            }
        }
    }

    Ok(Json(ImportSummary {
        total_rows: results.len(),
        created,
        updated,
        failed,
        results,
    }))
}

/// `GET /api/admin/profiles/export?format=csv|json` — admin-only
/// whole-organization export. Defaults to CSV if `format` is omitted or
/// unrecognized.
pub async fn export_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&admin.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;

    let rows = fetch_staff_profile_rows(&state, organization_id)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    let profiles: Vec<PublicProfileDto> = rows
        .into_iter()
        .filter_map(|row| serde_json::from_value::<profile_domain::StaffProfile>(row).ok())
        .map(|p| to_public_dto(&p))
        .collect();

    let format = params.get("format").map(String::as_str).unwrap_or("csv");
    if format == "json" {
        let body = serde_json::to_vec_pretty(&profiles)
            .map_err(|e| infrastructure_error(e.to_string()))?;
        Ok((StatusCode::OK, [("content-type", "application/json")], body).into_response())
    } else {
        let mut writer = csv::Writer::from_writer(Vec::new());
        for profile in &profiles {
            let csv_row: CsvRow = profile.into();
            writer
                .serialize(&csv_row)
                .map_err(|e| infrastructure_error(e.to_string()))?;
        }
        let body = writer
            .into_inner()
            .map_err(|e| infrastructure_error(e.to_string()))?;
        Ok((StatusCode::OK, [("content-type", "text/csv")], body).into_response())
    }
}
