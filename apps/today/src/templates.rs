use chrono::NaiveDate;
use serde::Serialize;

use crate::persistence::{Item, ItemState};

#[derive(Clone, Serialize)]
pub struct IndexContext {
    date: String,
    items: Vec<Item>,
}

impl IndexContext {
    pub fn new(date: NaiveDate, items: Vec<Item>) -> Self {
        let mut items: Vec<Item> = items
            .into_iter()
            .filter(|item| item.state != ItemState::Deleted)
            .collect();

        // Completed items move below open ones on load; the stable sort keeps their stored order within each group
        items.sort_by_key(|item| item.state == ItemState::Checked);

        Self {
            date: date.format("%A %-d %B").to_string(),
            items,
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
    fn completed_items_move_below_open_items() {
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

        assert_eq!(context.items, vec![unchecked_item, checked_item]);
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

        assert_eq!(
            context.items,
            vec![first_open, second_open, first_checked, second_checked]
        );
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

        assert!(
            !context.items.contains(&deleted_item),
            "items contained a deleted item"
        );
    }
}
