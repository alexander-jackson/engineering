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
        // Items are kept in their stored order so completed items stay in place
        let items = items
            .into_iter()
            .filter(|item| item.state != ItemState::Deleted)
            .collect();

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
    fn items_keep_their_order_and_state() {
        let checked_item = Item {
            item_uid: ItemUid::new(),
            content: "checked".to_owned().into(),
            state: ItemState::Checked,
        };

        let unchecked_item = Item {
            item_uid: ItemUid::new(),
            content: "unchecked".to_owned().into(),
            state: ItemState::Unchecked,
        };

        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let items = vec![checked_item.clone(), unchecked_item.clone()];
        let context = IndexContext::new(date, items);

        assert_eq!(context.items, vec![checked_item, unchecked_item]);
    }

    #[test]
    fn deleted_items_are_ignored() {
        let deleted_item = Item {
            item_uid: ItemUid::new(),
            content: "deleted".to_owned().into(),
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
