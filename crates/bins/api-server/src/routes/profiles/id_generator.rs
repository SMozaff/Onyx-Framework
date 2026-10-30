//! Generates human-readable IDs from their parts. Pure functions only:
//! the per-year counter is supplied by the caller, which owns storage.
//!
//! - Staff: `st` + `YY` + initials (first letter of first English name +
//!   first letter of last English name, lowercase) + 4-digit order per year.
//!   Example: `st26sm0035`.
//! - Team: `tm` + `YYMMDD` creation date + leader initials (same rule) +
//!   4-digit order per year. Example: `tm261030sm0007`.
//!
//! Orders start at 1 and are never reused. Names must be Latin letters.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// Name has no ASCII letter to take an initial from.
    NonLatinName,
    /// Order is 0 or above 9999.
    OrderOutOfRange(u32),
    /// Year is outside 2000..=2099.
    YearOutOfRange(i32),
    /// Month or day is not a real calendar date.
    InvalidDate,
}

/// Lowercase initial pair from two English names.
pub fn initials(first: &str, last: &str) -> Result<String, IdError> {
    let f = first_ascii_letter(first)?;
    let l = first_ascii_letter(last)?;
    Ok(format!("{f}{l}"))
}

fn first_ascii_letter(name: &str) -> Result<char, IdError> {
    name.trim()
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .ok_or(IdError::NonLatinName)
}

fn check_order(order: u32) -> Result<(), IdError> {
    if (1..=9999).contains(&order) {
        Ok(())
    } else {
        Err(IdError::OrderOutOfRange(order))
    }
}

fn two_digit_year(year: i32) -> Result<u32, IdError> {
    if (2000..=2099).contains(&year) {
        Ok((year - 2000) as u32)
    } else {
        Err(IdError::YearOutOfRange(year))
    }
}

/// `st` + `YY` + initials + 4-digit order.
pub fn staff_id(year: i32, first: &str, last: &str, order: u32) -> Result<String, IdError> {
    let yy = two_digit_year(year)?;
    check_order(order)?;
    Ok(format!("st{yy:02}{}{order:04}", initials(first, last)?))
}

/// `tm` + `YYMMDD` + leader initials + 4-digit order.
pub fn team_id(
    year: i32,
    month: u32,
    day: u32,
    leader_first: &str,
    leader_last: &str,
    order: u32,
) -> Result<String, IdError> {
    let yy = two_digit_year(year)?;
    if !is_valid_date(year, month, day) {
        return Err(IdError::InvalidDate);
    }
    check_order(order)?;
    Ok(format!(
        "tm{yy:02}{month:02}{day:02}{}{order:04}",
        initials(leader_first, leader_last)?
    ))
}

fn is_valid_date(year: i32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap {
                29
            } else {
                28
            }
        }
    };
    day <= days
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_id_matches_the_example() {
        assert_eq!(
            staff_id(2026, "Soheil", "Mozaffari", 35).unwrap(),
            "st26sm0035"
        );
    }

    #[test]
    fn staff_order_is_zero_padded_and_initials_lowercase() {
        assert_eq!(staff_id(2026, "ALI", "ahmadi", 1).unwrap(), "st26aa0001");
        assert_eq!(staff_id(2026, "Ali", "Ahmadi", 9999).unwrap(), "st26aa9999");
    }

    #[test]
    fn staff_order_bounds_are_enforced() {
        assert_eq!(
            staff_id(2026, "A", "B", 0),
            Err(IdError::OrderOutOfRange(0))
        );
        assert_eq!(
            staff_id(2026, "A", "B", 10000),
            Err(IdError::OrderOutOfRange(10000))
        );
    }

    #[test]
    fn year_must_be_in_range() {
        assert_eq!(
            staff_id(1999, "A", "B", 1),
            Err(IdError::YearOutOfRange(1999))
        );
        assert_eq!(
            staff_id(2100, "A", "B", 1),
            Err(IdError::YearOutOfRange(2100))
        );
    }

    #[test]
    fn non_latin_name_has_no_initial() {
        assert_eq!(
            staff_id(2026, "سهیل", "Mozaffari", 1),
            Err(IdError::NonLatinName)
        );
        assert_eq!(
            staff_id(2026, "", "Mozaffari", 1),
            Err(IdError::NonLatinName)
        );
    }

    #[test]
    fn team_id_matches_the_layout() {
        assert_eq!(
            team_id(2026, 10, 30, "Soheil", "Mozaffari", 7).unwrap(),
            "tm261030sm0007"
        );
    }

    #[test]
    fn team_id_rejects_impossible_dates() {
        assert_eq!(team_id(2026, 2, 29, "A", "B", 1), Err(IdError::InvalidDate));
        assert_eq!(team_id(2024, 2, 29, "A", "B", 1).unwrap(), "tm240229ab0001");
        assert_eq!(team_id(2026, 13, 1, "A", "B", 1), Err(IdError::InvalidDate));
        assert_eq!(team_id(2026, 4, 31, "A", "B", 1), Err(IdError::InvalidDate));
    }
}
