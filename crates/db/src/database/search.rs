use kellnr_common::account_name::UserName;
use kellnr_common::crate_overview::CrateOverview;
use kellnr_entity::{
    crate_category, crate_category_to_crate, crate_group, crate_keyword, crate_keyword_to_crate,
    crate_user, group_user, owner, user,
};
use kellnr_migration::iden::{CrateIden, CrateMetaIden, CratesIoIden, CratesIoMetaIden};
use sea_orm::sea_query::{
    Alias, Cond, Condition, Expr, Func, LikeExpr, Order, Query, SelectStatement, UnionType,
};
use sea_orm::{ExprTrait, FromQueryResult};

use super::Database;
use crate::error::DbError;
use crate::provider::{CrateSort, CrateTagFilter, CrateVisibility, DbResult};

/// Escape the LIKE wildcards `%` and `_` (and the escape character itself) in a
/// user-supplied search term so it is matched literally. Callers wrap the result
/// in `%...%` and pair it with `ESCAPE '\'`. Without this, a search for `%`
/// matches every row and turns the search endpoint into a cheap `DoS`.
fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl Database {
    /// Adds an optional `where` clause for case-insensitive matching of name and
    /// description.
    ///
    /// Case folding is only reliable for ASCII. The pattern is lowercased by
    /// Rust, which is Unicode-aware, while the columns are lowercased by the
    /// database: `PostgreSQL` folds according to the locale, but `SQLite`
    /// ships without ICU and folds ASCII only. A description containing
    /// "ÄPFEL" therefore stays uppercase on `SQLite` and never matches the
    /// lowercased pattern, for any spelling of the query. Crate names are
    /// unaffected, since `OriginalName` restricts them to ASCII.
    fn add_filter_by_name_and_description(
        mut query: SelectStatement,
        contains: Option<&str>,
        name: Expr,
        description: Expr,
    ) -> SelectStatement {
        if let Some(contains) = contains {
            let pattern = format!("%{}%", escape_like_pattern(contains)).to_lowercase();
            query.and_where(
                Func::lower(name)
                    .like(LikeExpr::new(&pattern).escape('\\'))
                    .or(Func::lower(description).like(LikeExpr::new(&pattern).escape('\\'))),
            );
        }
        query
    }

    /// Sort key for search results: exact name matches first, then names that
    /// start with the query, then names that contain it, then matches in the
    /// description only. Without a query every row gets the same key.
    fn search_rank(contains: Option<&str>, name: &Expr) -> Expr {
        let Some(contains) = contains else {
            return Expr::val(0);
        };
        let query = contains.to_lowercase();
        let escaped = escape_like_pattern(contains).to_lowercase();
        let lower_name = || Func::lower(name.clone());
        Expr::case(lower_name().eq(query), 0)
            .case(
                lower_name().like(LikeExpr::new(format!("{escaped}%")).escape('\\')),
                1,
            )
            .case(
                lower_name().like(LikeExpr::new(format!("%{escaped}%")).escape('\\')),
                2,
            )
            .finally(3)
            .into()
    }

    /// Joins the given SELECT statement against the crate table, restricts it to
    /// the latest version of every crate and applies the optional name filter.
    fn select_from_crates(mut query: SelectStatement, contains: Option<&str>) -> SelectStatement {
        query
            .inner_join(
                CrateIden::Table,
                Expr::col((CrateMetaIden::Table, CrateMetaIden::CrateFk))
                    .equals((CrateIden::Table, CrateIden::Id)),
            )
            .and_where(
                Expr::col((CrateMetaIden::Table, CrateMetaIden::Version))
                    .equals((CrateIden::Table, CrateIden::MaxVersion)),
            );

        Self::add_filter_by_name_and_description(
            query,
            contains,
            Expr::col((CrateIden::Table, CrateIden::OriginalName)),
            Expr::col((CrateIden::Table, CrateIden::Description)),
        )
    }

    /// Joins the given SELECT statement against the crates.io cache table,
    /// restricts it to the latest version of every crate and applies the
    /// optional name filter.
    fn select_from_cache(mut query: SelectStatement, contains: Option<&str>) -> SelectStatement {
        query
            .inner_join(
                CratesIoIden::Table,
                Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::CratesIoFk))
                    .equals((CratesIoIden::Table, CratesIoIden::Id)),
            )
            .and_where(
                Expr::col((CratesIoMetaIden::Table, CratesIoMetaIden::Version))
                    .equals((CratesIoIden::Table, CratesIoIden::MaxVersion)),
            );

        Self::add_filter_by_name_and_description(
            query,
            contains,
            Expr::col((CratesIoIden::Table, CratesIoIden::OriginalName)),
            Expr::col((CratesIoIden::Table, CratesIoIden::Description)),
        )
    }

    /// Condition on the crate table that hides the download-restricted crates
    /// `visibility` does not grant access to. `None` if nothing is hidden.
    pub(super) fn crate_visibility_condition(visibility: &CrateVisibility) -> Option<Condition> {
        let unrestricted = Expr::col((CrateIden::Table, CrateIden::RestrictedDownload)).eq(false);
        match visibility {
            CrateVisibility::All => None,
            CrateVisibility::Unrestricted => Some(Cond::all().add(unrestricted)),
            CrateVisibility::User(user) => {
                let crate_id = || Expr::col((CrateIden::Table, CrateIden::Id));
                Some(
                    Cond::any()
                        .add(unrestricted)
                        .add(crate_id().in_subquery(Self::owned_crate_ids(user)))
                        .add(crate_id().in_subquery(Self::crate_user_crate_ids(user)))
                        .add(crate_id().in_subquery(Self::crate_group_crate_ids(user))),
                )
            }
        }
    }

    /// Condition on the crate table that keeps the crates with the keyword
    /// and category of `tags`. `None` if `tags` filters nothing.
    fn crate_tag_condition(tags: &CrateTagFilter) -> Option<Condition> {
        if tags.is_empty() {
            return None;
        }
        let crate_id = || Expr::col((CrateIden::Table, CrateIden::Id));
        let mut condition = Cond::all();
        if let Some(keyword) = &tags.keyword {
            condition = condition.add(crate_id().in_subquery(Self::keyword_crate_ids(keyword)));
        }
        if let Some(category) = &tags.category {
            condition = condition.add(crate_id().in_subquery(Self::category_crate_ids(category)));
        }
        Some(condition)
    }

    fn keyword_crate_ids(keyword: &str) -> SelectStatement {
        Query::select()
            .column((
                crate_keyword_to_crate::Entity,
                crate_keyword_to_crate::Column::CrateFk,
            ))
            .from(crate_keyword_to_crate::Entity)
            .inner_join(
                crate_keyword::Entity,
                Expr::col((crate_keyword::Entity, crate_keyword::Column::Id)).equals((
                    crate_keyword_to_crate::Entity,
                    crate_keyword_to_crate::Column::KeywordFk,
                )),
            )
            .and_where(
                Func::lower(Expr::col((
                    crate_keyword::Entity,
                    crate_keyword::Column::Keyword,
                )))
                .eq(keyword.to_lowercase()),
            )
            .to_owned()
    }

    fn category_crate_ids(category: &str) -> SelectStatement {
        Query::select()
            .column((
                crate_category_to_crate::Entity,
                crate_category_to_crate::Column::CrateFk,
            ))
            .from(crate_category_to_crate::Entity)
            .inner_join(
                crate_category::Entity,
                Expr::col((crate_category::Entity, crate_category::Column::Id)).equals((
                    crate_category_to_crate::Entity,
                    crate_category_to_crate::Column::CategoryFk,
                )),
            )
            .and_where(
                Func::lower(Expr::col((
                    crate_category::Entity,
                    crate_category::Column::Category,
                )))
                .eq(category.to_lowercase()),
            )
            .to_owned()
    }

    pub(super) fn user_id_by_name(user: &UserName) -> SelectStatement {
        Query::select()
            .column((user::Entity, user::Column::Id))
            .from(user::Entity)
            .and_where(Expr::col((user::Entity, user::Column::Name)).eq(user))
            .to_owned()
    }

    fn owned_crate_ids(user: &UserName) -> SelectStatement {
        Query::select()
            .column((owner::Entity, owner::Column::CrateFk))
            .from(owner::Entity)
            .and_where(
                Expr::col((owner::Entity, owner::Column::UserFk))
                    .in_subquery(Self::user_id_by_name(user)),
            )
            .to_owned()
    }

    fn crate_user_crate_ids(user: &UserName) -> SelectStatement {
        Query::select()
            .column((crate_user::Entity, crate_user::Column::CrateFk))
            .from(crate_user::Entity)
            .and_where(
                Expr::col((crate_user::Entity, crate_user::Column::UserFk))
                    .in_subquery(Self::user_id_by_name(user)),
            )
            .to_owned()
    }

    fn crate_group_crate_ids(user: &UserName) -> SelectStatement {
        Query::select()
            .column((crate_group::Entity, crate_group::Column::CrateFk))
            .from(crate_group::Entity)
            .inner_join(
                group_user::Entity,
                Expr::col((group_user::Entity, group_user::Column::GroupFk))
                    .equals((crate_group::Entity, crate_group::Column::GroupFk)),
            )
            .and_where(
                Expr::col((group_user::Entity, group_user::Column::UserFk))
                    .in_subquery(Self::user_id_by_name(user)),
            )
            .to_owned()
    }

    /// Shared implementation for crate overview listing: optional name filter,
    /// optional keyword and category filter, optional limit/offset, cache union.
    async fn query_crates(
        &self,
        contains: Option<&str>,
        limit_offset: Option<(u64, u64)>,
        cache: bool,
        visibility: &CrateVisibility,
        sort: CrateSort,
        tags: &CrateTagFilter,
    ) -> DbResult<Vec<CrateOverview>> {
        /// Creates a SELECT statement for a given set of fields.
        fn select_fields(fields: &[(Expr, &str)]) -> SelectStatement {
            let mut query = Query::select();
            for (col, alias) in fields {
                query.expr_as(col.clone(), Alias::new(*alias));
            }
            query
        }

        let mut query = select_fields(&[
            (Expr::col(CrateIden::OriginalName), "name"),
            (Expr::col(CrateIden::MaxVersion), "version"),
            (Expr::col(CrateIden::LastUpdated), "date"),
            (Expr::col(CrateIden::TotalDownloads), "total_downloads"),
            (Expr::col(CrateIden::Description), "description"),
            (Expr::col(CrateMetaIden::Documentation), "documentation"),
            (Expr::col(CrateIden::Repository), "repository"),
            (Expr::cust("false"), "is_cache"),
            (
                Self::search_rank(
                    contains,
                    &Expr::col((CrateIden::Table, CrateIden::OriginalName)),
                ),
                "search_rank",
            ),
        ]);
        query.from(CrateMetaIden::Table);
        query = Self::select_from_crates(query, contains);
        if let Some(condition) = Self::crate_visibility_condition(visibility) {
            query.cond_where(condition);
        }
        if let Some(condition) = Self::crate_tag_condition(tags) {
            query.cond_where(condition);
        }

        // UNION with cached crates, which have no keywords or categories
        if cache && tags.is_empty() {
            let mut query2 = select_fields(&[
                (Expr::col(CratesIoIden::OriginalName), "name"),
                (Expr::col(CratesIoIden::MaxVersion), "version"),
                (Expr::col(CratesIoIden::LastUpdated), "date"),
                (Expr::col(CratesIoIden::TotalDownloads), "total_downloads"),
                (Expr::col(CratesIoIden::Description), "description"),
                (Expr::col(CratesIoMetaIden::Documentation), "documentation"),
                // The proxy does not store the repository of crates.io crates
                (Expr::cust("NULL"), "repository"),
                (Expr::cust("true"), "is_cache"),
                (
                    Self::search_rank(
                        contains,
                        &Expr::col((CratesIoIden::Table, CratesIoIden::OriginalName)),
                    ),
                    "search_rank",
                ),
            ]);
            query2.from(CratesIoMetaIden::Table);
            query2 = Self::select_from_cache(query2, contains);

            query.union(UnionType::All, query2);
        }

        // Ordering and optional limit/offset. The name breaks ties, so paging
        // stays stable within one rank, date or download count.
        match sort {
            CrateSort::Name => query.order_by(Alias::new("search_rank"), Order::Asc),
            // Dates are stored as "YYYY-MM-DD HH:MM:SS", so they sort as text.
            // Cached crates with an unknown date have an empty one and come last.
            CrateSort::Updated => query.order_by(Alias::new("date"), Order::Desc),
            CrateSort::Downloads => query.order_by(Alias::new("total_downloads"), Order::Desc),
        };
        query.order_by(Alias::new("name"), Order::Asc);
        if let Some((limit, offset)) = limit_offset {
            query.limit(limit).offset(offset);
        }

        let stmt = self.db_con.get_database_backend().build(&query);

        CrateOverview::find_by_statement(stmt)
            .all(&self.db_con)
            .await
            .map_err(DbError::from)
    }

    async fn count_crates(
        &self,
        contains: Option<&str>,
        cache: bool,
        visibility: &CrateVisibility,
        tags: &CrateTagFilter,
    ) -> DbResult<u64> {
        let mut query = Query::select();
        query.expr_as(
            Expr::col((CrateIden::Table, CrateIden::Id)).count(),
            Alias::new("count"),
        );
        query.from(CrateMetaIden::Table);
        query = Self::select_from_crates(query, contains);
        if let Some(condition) = Self::crate_visibility_condition(visibility) {
            query.cond_where(condition);
        }
        if let Some(condition) = Self::crate_tag_condition(tags) {
            query.cond_where(condition);
        }

        let count = self
            .count_by_statement(
                self.db_con.get_database_backend().build(&query),
                DbError::FailedToCountCrates,
            )
            .await?;

        let cache_count = if cache && tags.is_empty() {
            let mut query2 = Query::select();
            query2.expr_as(
                Expr::col((CratesIoIden::Table, CratesIoIden::Id)).count(),
                Alias::new("count"),
            );
            query2.from(CratesIoMetaIden::Table);
            query2 = Self::select_from_cache(query2, contains);

            self.count_by_statement(
                self.db_con.get_database_backend().build(&query2),
                DbError::FailedToCountCrates,
            )
            .await?
        } else {
            0
        };

        Ok(count + cache_count)
    }

    pub(super) async fn search_in_crate_name_and_description(
        &self,
        contains: &str,
        limit: u64,
        offset: u64,
        cache: bool,
        visibility: &CrateVisibility,
    ) -> DbResult<Vec<CrateOverview>> {
        self.query_crates(
            Some(contains),
            Some((limit, offset)),
            cache,
            visibility,
            CrateSort::Name,
            &CrateTagFilter::default(),
        )
        .await
    }

    pub(super) async fn count_by_crate_name_and_description(
        &self,
        contains: &str,
        cache: bool,
        visibility: &CrateVisibility,
    ) -> DbResult<u64> {
        self.count_crates(
            Some(contains),
            cache,
            visibility,
            &CrateTagFilter::default(),
        )
        .await
    }

    pub(super) async fn get_crate_overview_list(
        &self,
        limit: u64,
        offset: u64,
        cache: bool,
        visibility: &CrateVisibility,
        sort: CrateSort,
        tags: &CrateTagFilter,
    ) -> DbResult<Vec<CrateOverview>> {
        self.query_crates(None, Some((limit, offset)), cache, visibility, sort, tags)
            .await
    }

    pub(super) async fn count_crate_overview_list(
        &self,
        cache: bool,
        visibility: &CrateVisibility,
        tags: &CrateTagFilter,
    ) -> DbResult<u64> {
        self.count_crates(None, cache, visibility, tags).await
    }
}

#[cfg(test)]
mod like_escape_tests {
    use super::escape_like_pattern;

    #[test]
    fn escapes_wildcards_and_backslash() {
        assert_eq!(escape_like_pattern("a%b_c"), "a\\%b\\_c");
        assert_eq!(escape_like_pattern("100%"), "100\\%");
        // Backslash is escaped first so it doesn't double-escape the wildcards.
        assert_eq!(escape_like_pattern("a\\b"), "a\\\\b");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(escape_like_pattern("serde-json"), "serde-json");
    }
}
