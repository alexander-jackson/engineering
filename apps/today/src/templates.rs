use chrono::{Days, NaiveDate};
use serde::Serialize;

use crate::clock;
use crate::persistence::{Item, ItemState};

#[derive(Clone, Serialize)]
pub struct IndexContext {
    date: String,
    is_today: bool,
    previous: Option<String>,
    next: Option<String>,
    open_items: Vec<Item>,
    completed_items: Vec<Item>,
}

impl IndexContext {
    pub fn new(date: NaiveDate, today: NaiveDate, items: Vec<Item>) -> Self {
        // Completed items are shown apart from open ones; each group keeps its stored order
        let (completed_items, open_items): (Vec<Item>, Vec<Item>) = items
            .into_iter()
            .filter(|item| item.state != ItemState::Deleted)
            .partition(|item| item.state == ItemState::Checked);

        // Links only point at days that can actually be viewed
        let previous = (date > clock::earliest_viewable(today))
            .then(|| format!("/?date={}", date - Days::new(1)));
        let next = (date < today).then(|| {
            let next = date + Days::new(1);

            if next == today {
                "/".to_owned()
            } else {
                format!("/?date={next}")
            }
        });

        Self {
            date: date.format("%A %-d %B").to_string(),
            is_today: date == today,
            previous,
            next,
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
        let context = IndexContext::new(date, date, vec![]);

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
        let context = IndexContext::new(date, date, items);

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
        let context = IndexContext::new(date, date, items);

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
        let context = IndexContext::new(date, date, items);

        assert!(context.open_items.is_empty() && context.completed_items.is_empty());
    }

    #[test]
    fn navigation_links_stop_at_the_ends_of_the_window() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();

        let context = IndexContext::new(today, today, vec![]);
        assert_eq!(context.previous.as_deref(), Some("/?date=2026-11-05"));
        assert_eq!(context.next, None);
        assert!(context.is_today);

        let yesterday = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let context = IndexContext::new(yesterday, today, vec![]);
        assert_eq!(context.next.as_deref(), Some("/"));
        assert!(!context.is_today);

        let two_days_ago = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        let context = IndexContext::new(two_days_ago, today, vec![]);
        assert_eq!(context.next.as_deref(), Some("/?date=2026-11-05"));

        let earliest = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let context = IndexContext::new(earliest, today, vec![]);
        assert_eq!(context.previous, None);
        assert!(context.next.is_some());
    }
}
