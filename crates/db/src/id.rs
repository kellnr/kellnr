//! Typed primary keys, so the key of one table cannot be passed where the key
//! of another one is expected. They are stored and serialized as plain numbers.

use std::fmt;

use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Default,
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            ToSchema,
            DeriveValueType,
        )]
        #[serde(transparent)]
        pub struct $name(i64);

        impl $name {
            pub const fn new(id: i64) -> Self {
                Self(id)
            }

            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(
    /// Primary key of a user.
    UserId
);
id_type!(
    /// Primary key of a group.
    GroupId
);
id_type!(
    /// Primary key of a crate hosted by Kellnr.
    CrateId
);
id_type!(
    /// Primary key of an auth token.
    AuthTokenId
);

#[cfg(test)]
mod tests {
    use sea_orm::Value;

    use super::*;

    #[test]
    fn ids_are_numbers_in_json() {
        assert_eq!("42", serde_json::to_string(&UserId::new(42)).unwrap());
        assert_eq!("7", serde_json::to_string(&AuthTokenId::new(7)).unwrap());
        assert_eq!(
            CrateId::new(3_000_000_000),
            serde_json::from_str("3000000000").unwrap()
        );
        assert!(serde_json::from_str::<GroupId>(r#""1""#).is_err());
    }

    #[test]
    fn ids_are_big_ints_in_the_database() {
        assert_eq!(Value::BigInt(Some(5)), Value::from(GroupId::new(5)));
    }
}
