use std::collections::HashSet;

use kellnr_common::account_name::{GroupName, UserName};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_entity::prelude::*;
use kellnr_entity::{crate_group, crate_user, group, group_user, krate, owner, user};
use kellnr_migration::iden::CrateIden;
use sea_orm::query::QuerySelect;
use sea_orm::sea_query::{Cond, Expr, JoinType};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, ModelTrait, PaginatorTrait, QueryFilter,
    RelationTrait, Set,
};

use super::{Database, operations};
use crate::error::DbError;
use crate::provider::DbResult;
use crate::{Group, User};

/// Generates an `add_*` method that links two entities via a join table.
///
/// # Parameters
/// - `$method`: The method name to generate (e.g., `add_crate_user_impl`)
/// - `$relation_entity`: The join table entity module (e.g., `crate_user`)
/// - `$insert_entity`: The Sea ORM Entity type for insertion (e.g., `CrateUser`)
/// - `$fk1`: The foreign key field for the first entity
/// - `$fk2`: The foreign key field for the second entity
macro_rules! impl_add_relation {
    ($method:ident, $relation_entity:ident, $insert_entity:ident, $fk1:ident, $fk2:ident) => {
        pub(super) async fn $method(&self, fk1: i64, fk2: i64) -> DbResult<()> {
            let model = $relation_entity::ActiveModel {
                $fk1: Set(fk1),
                $fk2: Set(fk2),
                ..Default::default()
            };
            $insert_entity::insert(model).exec(&self.db_con).await?;
            Ok(())
        }
    };
}

impl Database {
    /// Looks up a `crate_group` by crate name and group name; returns `None` if not found.
    async fn get_crate_group_by_name_and_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<Option<crate_group::Model>> {
        crate_group::Entity::find()
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Group.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(group::Column::Name.eq(group)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    /// Looks up a `crate_user` by crate name and user name; returns `None` if not found.
    async fn get_crate_user_by_crate_and_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<Option<crate_user::Model>> {
        crate_user::Entity::find()
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .join(JoinType::InnerJoin, crate_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    impl_add_relation!(
        add_crate_user_impl,
        crate_user,
        CrateUser,
        crate_fk,
        user_fk
    );

    impl_add_relation!(
        add_crate_group_impl,
        crate_group,
        CrateGroup,
        crate_fk,
        group_fk
    );

    impl_add_relation!(
        add_group_user_impl,
        group_user,
        GroupUser,
        group_fk,
        user_fk
    );

    pub(super) async fn add_crate_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<()> {
        let crate_fk = self.get_krate_model(crate_name).await?.id;
        let user_fk = self.get_user_model(user).await?.id;
        self.add_crate_user_impl(crate_fk, user_fk).await
    }

    pub(super) async fn add_crate_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<()> {
        let crate_fk = self.get_krate_model(crate_name).await?.id;
        let group_fk = self.get_group_model(group).await?.id;
        self.add_crate_group_impl(crate_fk, group_fk).await
    }

    pub(super) async fn add_owner(
        &self,
        crate_name: &CanonicalName,
        owner: &UserName,
    ) -> DbResult<()> {
        let crate_fk = self.get_krate_model(crate_name).await?.id;
        operations::add_owner_if_not_exists(&self.db_con, owner, crate_fk).await
    }

    pub(super) async fn is_download_restricted(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<bool> {
        Ok(krate::Entity::find()
            .filter(krate::Column::Name.eq(crate_name))
            .one(&self.db_con)
            .await?
            .is_some_and(|model| model.restricted_download))
    }

    pub(super) async fn change_download_restricted(
        &self,
        crate_name: &CanonicalName,
        restricted: bool,
    ) -> DbResult<()> {
        let mut krate: krate::ActiveModel = self.get_krate_model(crate_name).await?.into();
        krate.restricted_download = Set(restricted);
        krate.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn is_crate_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<bool> {
        Ok(self
            .get_crate_user_by_crate_and_user(crate_name, user)
            .await?
            .is_some())
    }

    pub(super) async fn is_crate_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<bool> {
        Ok(self
            .get_crate_group_by_name_and_group(crate_name, group)
            .await?
            .is_some())
    }

    pub(super) async fn is_crate_group_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<bool> {
        let user = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::GroupUser.def())
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .join(JoinType::InnerJoin, group::Relation::CrateGroup.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?;
        Ok(user.is_some())
    }

    pub(super) async fn is_owner(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<bool> {
        let owner = owner::Entity::find()
            .join(JoinType::InnerJoin, owner::Relation::Krate.def())
            .join(JoinType::InnerJoin, owner::Relation::User.def())
            .filter(
                Cond::all()
                    .add(krate::Column::Name.eq(crate_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await?;

        Ok(owner.is_some())
    }

    pub(super) async fn get_crate_owners(&self, crate_name: &CanonicalName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::Owner.def())
            .join(JoinType::InnerJoin, owner::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    pub(super) async fn get_crate_users(&self, crate_name: &CanonicalName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::CrateUser.def())
            .join(JoinType::InnerJoin, crate_user::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    pub(super) async fn get_crate_groups(
        &self,
        crate_name: &CanonicalName,
    ) -> DbResult<Vec<Group>> {
        let u = group::Entity::find()
            .join(JoinType::InnerJoin, group::Relation::CrateGroup.def())
            .join(JoinType::InnerJoin, crate_group::Relation::Krate.def())
            .filter(Expr::col((CrateIden::Table, krate::Column::Name)).eq(crate_name))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(Group::from).collect())
    }

    pub(super) async fn delete_owners(
        &self,
        crate_name: &CanonicalName,
        owners: &[UserName],
        allow_ownerless: bool,
    ) -> DbResult<()> {
        self.in_transaction(async |txn| {
            // The row lock serializes concurrent removals on PostgreSQL, else each
            // could remove a different owner and leave the crate without one.
            let krate = krate::Entity::find()
                .filter(krate::Column::Name.eq(crate_name))
                .lock_exclusive()
                .one(txn)
                .await?
                .ok_or_else(|| DbError::CrateNotFound(crate_name.to_string()))?;

            let owners: HashSet<&UserName> = owners.iter().collect();
            for name in owners {
                let owner = owner::Entity::find()
                    .join(JoinType::InnerJoin, owner::Relation::User.def())
                    .filter(owner::Column::CrateFk.eq(krate.id))
                    .filter(user::Column::Name.eq(name.as_str()))
                    .one(txn)
                    .await?
                    .ok_or_else(|| DbError::OwnerNotFound(name.to_string()))?;
                owner.delete(txn).await?;
            }

            let remaining = owner::Entity::find()
                .filter(owner::Column::CrateFk.eq(krate.id))
                .count(txn)
                .await?;
            if remaining == 0 && !allow_ownerless {
                return Err(DbError::LastOwner);
            }

            Ok(())
        })
        .await
    }

    pub(super) async fn delete_crate_user(
        &self,
        crate_name: &CanonicalName,
        user: &UserName,
    ) -> DbResult<()> {
        let model = self
            .get_crate_user_by_crate_and_user(crate_name, user)
            .await?;
        self.delete_or_not_found(model, DbError::UserNotFound(user.to_string()))
            .await
    }

    pub(super) async fn delete_crate_group(
        &self,
        crate_name: &CanonicalName,
        group: &GroupName,
    ) -> DbResult<()> {
        let model = self
            .get_crate_group_by_name_and_group(crate_name, group)
            .await?;
        self.delete_or_not_found(model, DbError::GroupNotFound(group.to_string()))
            .await
    }
}
