use serde::Serialize;

use crate::persistence::{Item, ItemState};

#[derive(Clone, Serialize)]
pub struct IndexContext {
    checked_items: Vec<Item>,
    unchecked_items: Vec<Item>,
}

impl From<Vec<Item>> for IndexContext {
    fn from(items: Vec<Item>) -> Self {
        let mut checked_items = Vec::new();
        let mut unchecked_items = Vec::new();

        for item in items {
            match item.state {
                ItemState::Checked => checked_items.push(item),
                ItemState::Unchecked => unchecked_items.push(item),
                ItemState::Deleted => (), // intentionally ignored
            }
        }

        Self {
            checked_items,
            unchecked_items,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::persistence::{Item, ItemState};
    use crate::templates::IndexContext;
    use crate::uid::ItemUid;

    #[test]
    fn items_are_correctly_categorised() {
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

        let items = vec![checked_item.clone(), unchecked_item.clone()];
        let context = IndexContext::from(items);

        assert_eq!(context.checked_items, vec![checked_item]);
        assert_eq!(context.unchecked_items, vec![unchecked_item]);
    }

    #[test]
    fn deleted_items_are_ignored() {
        let deleted_item = Item {
            item_uid: ItemUid::new(),
            content: "deleted".to_owned().into(),
            state: ItemState::Deleted,
        };

        let items = vec![deleted_item.clone()];
        let context = IndexContext::from(items);

        assert!(
            !context.checked_items.contains(&deleted_item),
            "checked items contained a deleted item"
        );

        assert!(
            !context.unchecked_items.contains(&deleted_item),
            "unchecked items contained a delete item"
        );
    }
}
