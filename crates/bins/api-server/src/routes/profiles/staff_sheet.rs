//! One-sheet staff import from `.xlsx` (Excel).
//!
//! Rules (owner-confirmed, see `docs/product/staff-and-team-rules.md`):
//! - One sheet, one row per person, first row is the header.
//! - Required columns: `onyx_id`, `first_name`, `last_name`, `national_id`.
//! - `onyx_id` may be blank: a blank cell means the system generates the ID.
//! - `national_id` is Iran only: exactly 10 ASCII digits. It must be stored
//!   as text in Excel. A numeric cell is rejected, because Excel drops
//!   leading zeros and we will not guess the lost digits.
//! - `team_ids` is `;`-separated. Blanks are dropped and repeats removed.
//! - Every row is checked, and all errors are returned with their row numbers.

use std::collections::HashMap;
use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};

pub const REQUIRED_COLUMNS: [&str; 4] = ["onyx_id", "first_name", "last_name", "national_id"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaffSheetRow {
    pub onyx_id: String,
    pub first_name: String,
    pub last_name: String,
    pub national_id: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub job_title: String,
    pub department: String,
    pub class_label: String,
    pub parent_onyx_id: String,
    pub team_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    /// 1-based spreadsheet row number (header is row 1).
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StaffSheetOutcome {
    pub rows: Vec<StaffSheetRow>,
    pub errors: Vec<RowError>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StaffSheetError {
    #[error("file is not a readable .xlsx workbook")]
    NotXlsx,
    #[error("workbook has no sheet")]
    NoSheet,
    #[error("missing required column: {0}")]
    MissingColumn(&'static str),
}

/// Parses the first sheet of an `.xlsx` workbook.
pub fn parse_staff_sheet(bytes: &[u8]) -> Result<StaffSheetOutcome, StaffSheetError> {
    let mut workbook: Xlsx<_> =
        Xlsx::new(Cursor::new(bytes.to_vec())).map_err(|_| StaffSheetError::NotXlsx)?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or(StaffSheetError::NoSheet)?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|_| StaffSheetError::NotXlsx)?;

    let mut rows_iter = range.rows();
    let header_cells = match rows_iter.next() {
        Some(h) => h,
        None => return Ok(StaffSheetOutcome::default()),
    };
    let mut col: HashMap<String, usize> = HashMap::new();
    for (i, cell) in header_cells.iter().enumerate() {
        let name = cell_text(cell).trim().to_ascii_lowercase();
        if !name.is_empty() {
            col.insert(name, i);
        }
    }
    for required in REQUIRED_COLUMNS {
        if !col.contains_key(required) {
            return Err(StaffSheetError::MissingColumn(required));
        }
    }

    let mut outcome = StaffSheetOutcome::default();
    for (idx, cells) in rows_iter.enumerate() {
        let row_no = idx + 2;
        if cells.iter().all(|c| cell_text(c).trim().is_empty()) {
            continue;
        }
        let get = |name: &str| -> Option<&Data> { col.get(name).and_then(|&i| cells.get(i)) };
        let text = |name: &str| -> String {
            get(name)
                .map(|c| cell_text(c).trim().to_string())
                .unwrap_or_default()
        };

        let mut errs: Vec<String> = Vec::new();
        for required in REQUIRED_COLUMNS {
            if required == "national_id" || required == "onyx_id" {
                continue;
            }
            if text(required).is_empty() {
                errs.push(format!("{required} is required"));
            }
        }

        let national_cell = get("national_id");
        let national_id = match national_cell {
            Some(Data::Float(_)) | Some(Data::Int(_)) => {
                errs.push("national_id must be stored as text, not a number".into());
                String::new()
            }
            _ => text("national_id"),
        };
        if national_id.is_empty() {
            errs.push("national_id is required".into());
        } else if national_id.len() != 10 || !national_id.bytes().all(|b| b.is_ascii_digit()) {
            errs.push("national_id must be exactly 10 digits".into());
        }

        if !errs.is_empty() {
            outcome.errors.push(RowError {
                row: row_no,
                message: errs.join("; "),
            });
            continue;
        }

        let mut team_ids: Vec<String> = Vec::new();
        for id in text("team_ids")
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if !team_ids.iter().any(|t| t == id) {
                team_ids.push(id.to_string());
            }
        }

        outcome.rows.push(StaffSheetRow {
            onyx_id: text("onyx_id"),
            first_name: text("first_name"),
            last_name: text("last_name"),
            national_id,
            contact_email: text("contact_email"),
            contact_phone: text("contact_phone"),
            job_title: text("job_title"),
            department: text("department"),
            class_label: text("class_label"),
            parent_onyx_id: text("parent_onyx_id"),
            team_ids,
        });
    }
    Ok(outcome)
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => d.as_f64().to_string(),
        Data::Error(e) => format!("#{e:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Workbook;

    /// Builds a one-sheet workbook. `national_as_number` writes the
    /// national_id of row 2 as a number to test rejection.
    fn workbook(rows: &[Vec<&str>], national_as_number: bool) -> Vec<u8> {
        let mut wb = Workbook::new();
        let ws = wb.add_worksheet();
        for (r, cells) in rows.iter().enumerate() {
            for (c, value) in cells.iter().enumerate() {
                let is_national = rows[0].get(c) == Some(&"national_id");
                if r == 1 && is_national && national_as_number {
                    ws.write_number(r as u32, c as u16, value.parse::<f64>().unwrap())
                        .unwrap();
                } else {
                    ws.write_string(r as u32, c as u16, *value).unwrap();
                }
            }
        }
        wb.save_to_buffer().unwrap()
    }

    const HEADER: [&str; 11] = [
        "onyx_id",
        "first_name",
        "last_name",
        "national_id",
        "contact_email",
        "contact_phone",
        "job_title",
        "department",
        "class_label",
        "parent_onyx_id",
        "team_ids",
    ];

    fn row(vals: [&str; 11]) -> Vec<&str> {
        vals.to_vec()
    }

    #[test]
    fn valid_row_is_parsed_with_split_names_and_deduped_teams() {
        let bytes = workbook(
            &[
                HEADER.to_vec(),
                row([
                    "st26sm0035",
                    "Soheil",
                    "Mozaffari",
                    "0012345678",
                    "a@b.ir",
                    "0912",
                    "Engineer",
                    "IT",
                    "",
                    "st26ab0001",
                    "tm261030sm0001; tm261030sm0001;;",
                ])
                .to_vec(),
            ],
            false,
        );
        let out = parse_staff_sheet(&bytes).unwrap();
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(out.rows.len(), 1);
        let r = &out.rows[0];
        assert_eq!(r.first_name, "Soheil");
        assert_eq!(r.last_name, "Mozaffari");
        assert_eq!(
            r.national_id, "0012345678",
            "leading zero preserved as text"
        );
        assert_eq!(r.team_ids, vec!["tm261030sm0001"]);
    }

    #[test]
    fn national_id_must_be_exactly_ten_digits() {
        let bytes = workbook(
            &[
                HEADER.to_vec(),
                row([
                    "st26sm0001",
                    "A",
                    "B",
                    "123456789",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
                row([
                    "st26sm0002",
                    "A",
                    "B",
                    "12345678901",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
                row([
                    "st26sm0003",
                    "A",
                    "B",
                    "12345abcde",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
            ],
            false,
        );
        let out = parse_staff_sheet(&bytes).unwrap();
        assert!(out.rows.is_empty());
        assert_eq!(out.errors.len(), 3);
        assert!(out
            .errors
            .iter()
            .all(|e| e.message.contains("exactly 10 digits")));
    }

    #[test]
    fn numeric_national_id_cell_is_rejected_not_guessed() {
        let bytes = workbook(
            &[
                HEADER.to_vec(),
                row([
                    "st26sm0001",
                    "A",
                    "B",
                    "0012345678",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
            ],
            true,
        );
        let out = parse_staff_sheet(&bytes).unwrap();
        assert!(out.rows.is_empty());
        assert_eq!(out.errors.len(), 1);
        assert!(out.errors[0].message.contains("stored as text"));
    }

    #[test]
    fn blank_onyx_id_is_accepted_and_left_for_generation() {
        let bytes = workbook(
            &[
                HEADER.to_vec(),
                row([
                    "",
                    "Sara",
                    "Karimi",
                    "0011223344",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
            ],
            false,
        );
        let out = parse_staff_sheet(&bytes).unwrap();
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(out.rows[0].onyx_id, "");
    }

    #[test]
    fn errors_report_row_numbers_and_all_missing_fields() {
        let bytes = workbook(
            &[
                HEADER.to_vec(),
                row(["", "", "", "0012345678", "", "", "", "", "", "", ""]).to_vec(),
                row([
                    "st26sm0002",
                    "Ali",
                    "Ahmadi",
                    "0098765432",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .to_vec(),
            ],
            false,
        );
        let out = parse_staff_sheet(&bytes).unwrap();
        assert_eq!(out.rows.len(), 1);
        assert_eq!(out.errors.len(), 1);
        assert_eq!(out.errors[0].row, 2);
        assert!(out.errors[0].message.contains("first_name is required"));
        assert!(out.errors[0].message.contains("last_name is required"));
    }

    #[test]
    fn missing_required_column_is_a_file_level_error() {
        let header: Vec<&str> = HEADER
            .iter()
            .copied()
            .filter(|h| *h != "national_id")
            .collect();
        let bytes = workbook(&[header], false);
        assert_eq!(
            parse_staff_sheet(&bytes),
            Err(StaffSheetError::MissingColumn("national_id"))
        );
    }

    #[test]
    fn non_xlsx_bytes_are_rejected() {
        assert_eq!(
            parse_staff_sheet(b"onyx_id,first_name\n1,2\n"),
            Err(StaffSheetError::NotXlsx)
        );
    }
}
