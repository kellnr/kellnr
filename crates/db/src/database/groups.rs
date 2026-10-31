use kellnr_common::account_name::{GroupName, UserName};
use kellnr_entity::{group, group_user, user};
use kellnr_migration::iden::GroupIden;
use sea_orm::query::{QueryOrder, QuerySelect};
use sea_orm::sea_query::{Cond, Expr, JoinType};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, ModelTrait, QueryFilter, RelationTrait,
    Set,
};

use super::Database;
use crate::error::DbError;
use crate::provider::DbResult;
use crate::{Group, User};

impl Database {
    /// Looks up a `group_user` by group name and user name; returns `None` if not found.
    async fn get_group_user_by_group_and_user(
        &self,
        group_name: &GroupName,
        user: &UserName,
    ) -> DbResult<Option<group_user::Model>> {
        group_user::Entity::find()
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .join(JoinType::InnerJoin, group_user::Relation::User.def())
            .filter(
                Cond::all()
                    .add(group::Column::Name.eq(group_name))
                    .add(user::Column::Name.eq(user)),
            )
            .one(&self.db_con)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn add_group_user(
        &self,
        group_name: &GroupName,
        user: &UserName,
    ) -> DbResult<()> {
        let group_fk = self.get_group_model(group_name).await?.id;
        let user_fk = self.get_user_model(user).await?.id;
        self.add_group_user_impl(group_fk, user_fk).await
    }

    pub(super) async fn is_group_user(
        &self,
        group_name: &GroupName,
        user: &UserName,
    ) -> DbResult<bool> {
        Ok(self
            .get_group_user_by_group_and_user(group_name, user)
            .await?
            .is_some())
    }

    pub(super) async fn get_group_users(&self, group_name: &GroupName) -> DbResult<Vec<User>> {
        let u = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::GroupUser.def())
            .join(JoinType::InnerJoin, group_user::Relation::Group.def())
            .filter(Expr::col((GroupIden::Table, group::Column::Name)).eq(group_name))
            .all(&self.db_con)
            .await?;

        Ok(u.into_iter().map(User::from).collect())
    }

    pub(super) async fn delete_group(&self, group_name: &GroupName) -> DbResult<()> {
        self.get_group_model(group_name)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn delete_group_user(
        &self,
        group_name: &GroupName,
        user: &UserName,
    ) -> DbResult<()> {
        let model = self
            .get_group_user_by_group_and_user(group_name, user)
            .await?;
        self.delete_or_not_found(model, DbError::UserNotFound(user.to_string()))
            .await
    }

    pub(super) async fn add_group(&self, name: &GroupName) -> DbResult<()> {
        let g = group::ActiveModel {
            name: Set(name.as_str().to_owned()),
            ..Default::default()
        };

        g.insert(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_groups(&self) -> DbResult<Vec<Group>> {
        let groups = group::Entity::find()
            .order_by_asc(group::Column::Name)
            .all(&self.db_con)
            .await?;

        Ok(groups.into_iter().map(Group::from).collect())
    }
}
