mod paging;

use paging::{page_count, page_slice, sort_rows, Row};
use slint::{ModelRc, SharedString, StandardListViewItem, TableColumn, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

const PAGE_SIZE: usize = 10;

fn to_model(rows: &[Row]) -> ModelRc<ModelRc<StandardListViewItem>> {
    let outer: Vec<ModelRc<StandardListViewItem>> = rows
        .iter()
        .map(|r| {
            let cells: Vec<StandardListViewItem> = r
                .cells
                .iter()
                .map(|c| StandardListViewItem::from(SharedString::from(c.as_str())))
                .collect();
            ModelRc::new(VecModel::from(cells))
        })
        .collect();
    ModelRc::new(VecModel::from(outer))
}

fn main() -> Result<(), slint::PlatformError> {
    let all: Vec<Row> = (0..137)
        .map(|i| Row { cells: vec![format!("user-{:03}", (i * 37) % 137), format!("{}", i)] })
        .collect();
    let state = Rc::new(RefCell::new((all.clone(), 0usize)));

    let ui = SpikeWindow::new()?;
    let columns: Vec<TableColumn> = ["Name", "Index"]
        .iter()
        .map(|t| {
            let mut col = TableColumn::default();
            col.title = SharedString::from(*t);
            col
        })
        .collect();
    ui.set_columns(ModelRc::new(VecModel::from(columns)));

    let refresh = {
        let weak = ui.as_weak();
        let state = state.clone();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let (rows, page) = &*state.borrow();
            let view = page_slice(rows, *page, PAGE_SIZE);
            ui.set_rows(to_model(view));
            ui.set_page_label(SharedString::from(format!(
                "page {} of {}",
                page + 1,
                page_count(rows.len(), PAGE_SIZE)
            )));
        }
    };
    refresh();

    {
        let state = state.clone();
        let refresh = refresh.clone();
        ui.on_sort_ascending(move |col| {
            let mut s = state.borrow_mut();
            sort_rows(&mut s.0, col as usize, true);
            s.1 = 0;
            drop(s);
            refresh();
        });
    }
    {
        let state = state.clone();
        let refresh = refresh.clone();
        ui.on_sort_descending(move |col| {
            let mut s = state.borrow_mut();
            sort_rows(&mut s.0, col as usize, false);
            s.1 = 0;
            drop(s);
            refresh();
        });
    }
    {
        let state = state.clone();
        let refresh = refresh.clone();
        ui.on_next_page(move || {
            let mut s = state.borrow_mut();
            let max = page_count(s.0.len(), PAGE_SIZE) - 1;
            s.1 = (s.1 + 1).min(max);
            drop(s);
            refresh();
        });
    }
    {
        let state = state.clone();
        ui.on_prev_page(move || {
            let mut s = state.borrow_mut();
            s.1 = s.1.saturating_sub(1);
            drop(s);
            refresh();
        });
    }

    ui.run()
}
