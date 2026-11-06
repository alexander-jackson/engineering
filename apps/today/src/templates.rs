use chrono::NaiveDate;
use serde::Serialize;

use crate::persistence::{Item, ItemState};

#[derive(Clone, Serialize)]
pub struct IndexContext {
    date: String,
    open_items: Vec<Item>,
    completed_items: Vec<Item>,
}

impl IndexContext {
    pub fn new(date: NaiveDate, items: Vec<Item>) -> Self {
        // Completed items are shown apart from open ones; each group keeps its stored order
        let (completed_items, open_items): (Vec<Item>, Vec<Item>) = items
            .into_iter()
            .filter(|item| item.state != ItemState::Deleted)
            .partition(|item| item.state == ItemState::Checked);

        Self {
            date: date.format("%A %-d %B").to_string(),
            open_items,
            completed_items,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::persistence::{Item, ItemState};
    use crate::templates::IndexContext;
    use crate::uid::ItemUid;

    #[test]
    fn date_is_formatted_for_display() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let context = IndexContext::new(date, vec![]);

        assert_eq!(context.date, "Friday 6 November");
    }

    #[test]
    fn completed_items_are_separated_from_open_items() {
        let checked_item = Item {
            item_uid: ItemUid::new(),
            content: "checked".to_owned().into(),
            source: "checked".to_owned(),
            state: ItemState::Checked,
        };

        let unchecked_item = Item {
            item_uid: ItemUid::new(),
            content: "unchecked".to_owned().into(),
            source: "unchecked".to_owned(),
            state: ItemState::Unchecked,
        };

        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let items = vec![checked_item.clone(), unchecked_item.clone()];
        let context = IndexContext::new(date, items);

        assert_eq!(context.open_items, vec![unchecked_item]);
        assert_eq!(context.completed_items, vec![checked_item]);
    }

    #[test]
    fn items_keep_their_order_within_each_group() {
        let item = |content: &str, state| Item {
            item_uid: ItemUid::new(),
            content: content.to_owned().into(),
            source: content.to_owned(),
            state,
        };

        let first_checked = item("first checked", ItemState::Checked);
        let first_open = item("first open", ItemState::Unchecked);
        let second_checked = item("second checked", ItemState::Checked);
        let second_open = item("second open", ItemState::Unchecked);

        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let items = vec![
            first_checked.clone(),
            first_open.clone(),
            second_checked.clone(),
            second_open.clone(),
        ];
        let context = IndexContext::new(date, items);

        assert_eq!(context.open_items, vec![first_open, second_open]);
        assert_eq!(context.completed_items, vec![first_checked, second_checked]);
    }

    #[test]
    fn deleted_items_are_ignored() {
        let deleted_item = Item {
            item_uid: ItemUid::new(),
            content: "deleted".to_owned().into(),
            source: "deleted".to_owned(),
            state: ItemState::Deleted,
        };

        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let items = vec![deleted_item.clone()];
        let context = IndexContext::new(date, items);

        assert!(context.open_items.is_empty() && context.completed_items.is_empty());
    }
}
