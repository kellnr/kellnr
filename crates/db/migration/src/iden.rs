use sea_orm_migration::prelude::*;

#[derive(Iden)]
pub enum UserIden {
    #[iden = "user"]
    Table,
    Id,
    Name,
    Pwd,
    Salt,
    IsAdmin,
}

#[derive(Iden)]
pub enum SessionIden {
    #[iden = "session"]
    Table,
    Id,
    Token,
    Created,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    Id,
    Name,
    OriginalName,
    MaxVersion,
    LastUpdated,
    TotalDownloads,
    Description,
    Homepage,
    Repository,
    ETag,
    RestrictedDownload,
}

#[derive(Iden)]
pub enum CrateMetaIden {
    #[iden = "crate_meta"]
    Table,
    Id,
    Version,
    Created,
    Downloads,
    Readme,
    License,
    LicenseFile,
    Documentation,
    #[iden = "crate_fk"]
    CrateFk,
}

#[derive(Iden)]
pub enum OwnerIden {
    #[iden = "owner"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum CrateUserIden {
    #[iden = "crate_user"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum AuthTokenIden {
    #[iden = "auth_token"]
    Table,
    Id,
    Name,
    Token,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum DocQueueIden {
    #[iden = "doc_queue"]
    Table,
    Id,
    Krate,
    Version,
    Path,
}
#[derive(Iden)]
pub enum CrateAuthorIden {
    #[iden = "crate_author"]
    Table,
    Id,
    Author,
}

#[derive(Iden)]
pub enum CrateAuthorToCrateIden {
    #[iden = "crate_author_to_crate"]
    Table,
    Id,
    AuthorFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateKeywordIden {
    #[iden = "crate_keyword"]
    Table,
    Id,
    Keyword,
}

#[derive(Iden)]
pub enum CrateKeywordToCrateIden {
    #[iden = "crate_keyword_to_crate"]
    Table,
    Id,
    KeywordFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateCategory {
    #[iden = "crate_category"]
    Table,
    Id,
    Category,
}

#[derive(Iden)]
pub enum CrateCategoryToCrateIden {
    #[iden = "crate_category_to_crate"]
    Table,
    Id,
    CategoryFk,
    CrateFk,
}

#[derive(Iden)]
pub enum CrateIndexIden {
    #[iden = "crate_index"]
    Table,
    Id,
    Vers,
    Deps,
    Cksum,
    Features,
    Yanked,
    Links,
    V,
    CrateFk,
}

#[derive(Iden)]
pub enum CratesIoIden {
    #[iden = "cratesio_crate"]
    Table,
    Id,
    Name,
    OriginalName,
    Description,
    ETag,
    LastModified,
    TotalDownloads,
    MaxVersion,
}

#[derive(Iden)]
pub enum CratesIoIndexIden {
    #[iden = "cratesio_index"]
    Table,
    Id,
    Vers,
    Deps,
    Cksum,
    Features,
    Yanked,
    Links,
    V,
    CratesIoFk,
}

#[derive(Iden)]
pub enum CratesIoMetaIden {
    #[iden = "cratesio_meta"]
    Table,
    Id,
    Version,
    Downloads,
    CratesIoFk,
    Documentation,
}
