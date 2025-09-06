use foundation_uid::{typed_uid, Uuid};
use serde::{Deserialize, Serialize};

typed_uid! {
    Eq, PartialEq, Hash, Serialize, Deserialize;

    ItemUid,
}
