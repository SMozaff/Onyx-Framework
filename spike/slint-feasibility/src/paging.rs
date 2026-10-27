//! Pure paging and sorting logic. Slint's StandardTableView does not page or
//! sort data itself; it only raises sort callbacks. This module is that logic.

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub cells: Vec<String>,
}

pub fn sort_rows(rows: &mut [Row], column: usize, ascending: bool) {
    rows.sort_by(|a, b| {
        let x = a.cells.get(column).map(String::as_str).unwrap_or("");
        let y = b.cells.get(column).map(String::as_str).unwrap_or("");
        let ord = x.cmp(y);
        if ascending { ord } else { ord.reverse() }
    });
}

/// Returns the zero-based page slice. Out-of-range pages are clamped.
pub fn page_slice(rows: &[Row], page: usize, page_size: usize) -> &[Row] {
    assert!(page_size > 0, "page_size must be positive");
    let pages = page_count(rows.len(), page_size);
    let page = page.min(pages.saturating_sub(1));
    let start = page * page_size;
    let end = (start + page_size).min(rows.len());
    &rows[start..end]
}

pub fn page_count(total: usize, page_size: usize) -> usize {
    if total == 0 { 1 } else { total.div_ceil(page_size) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(v: &str) -> Row { Row { cells: vec![v.to_string()] } }

    #[test]
    fn sorts_ascending_and_descending() {
        let mut rows = vec![row("b"), row("a"), row("c")];
        sort_rows(&mut rows, 0, true);
        assert_eq!(rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        sort_rows(&mut rows, 0, false);
        assert_eq!(rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["c", "b", "a"]);
    }

    #[test]
    fn missing_column_sorts_as_empty() {
        let mut rows = vec![row("x"), Row { cells: vec![] }];
        sort_rows(&mut rows, 3, true);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn pages_split_exactly_and_remainder() {
        let rows: Vec<Row> = (0..25).map(|i| row(&i.to_string())).collect();
        assert_eq!(page_count(25, 10), 3);
        assert_eq!(page_slice(&rows, 0, 10).len(), 10);
        assert_eq!(page_slice(&rows, 2, 10).len(), 5);
    }

    #[test]
    fn out_of_range_page_is_clamped_to_last() {
        let rows: Vec<Row> = (0..25).map(|i| row(&i.to_string())).collect();
        assert_eq!(page_slice(&rows, 99, 10)[0].cells[0], "20");
    }

    #[test]
    fn empty_table_has_one_empty_page() {
        let rows: Vec<Row> = vec![];
        assert_eq!(page_count(0, 10), 1);
        assert!(page_slice(&rows, 0, 10).is_empty());
    }

    #[test]
    fn persian_strings_sort_by_code_point() {
        let mut rows = vec![row("ب"), row("آ"), row("الف")];
        sort_rows(&mut rows, 0, true);
        assert_eq!(rows[0].cells[0], "آ");
    }
}
