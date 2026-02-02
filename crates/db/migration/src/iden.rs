use sea_orm_migration::prelude::*;

#[derive(Iden, Copy, Clone)]
pub enum UserIden {
    #[iden = "user"]
    Table,
    Id,
    Name,
    Pwd,
    Salt,
    IsAdmin,
    IsReadOnly,
    Created,
}

#[derive(Iden, Copy, Clone)]
pub enum SessionIden {
    #[iden = "session"]
    Table,
    Id,
    Token,
    Created,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden, Copy, Clone)]
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

#[derive(Iden, Copy, Clone)]
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

#[derive(Iden, Copy, Clone)]
pub enum OwnerIden {
    #[iden = "owner"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateUserIden {
    #[iden = "crate_user"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden, Copy, Clone)]
pub enum AuthTokenIden {
    #[iden = "auth_token"]
    Table,
    Id,
    Name,
    Token,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden, Copy, Clone)]
pub enum DocQueueIden {
    #[iden = "doc_queue"]
    Table,
    Id,
    Krate,
    Version,
    Path,
}
#[derive(Iden, Copy, Clone)]
pub enum CrateAuthorIden {
    #[iden = "crate_author"]
    Table,
    Id,
    Author,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateAuthorToCrateIden {
    #[iden = "crate_author_to_crate"]
    Table,
    Id,
    AuthorFk,
    CrateFk,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateKeywordIden {
    #[iden = "crate_keyword"]
    Table,
    Id,
    Keyword,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateKeywordToCrateIden {
    #[iden = "crate_keyword_to_crate"]
    Table,
    Id,
    KeywordFk,
    CrateFk,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateCategory {
    #[iden = "crate_category"]
    Table,
    Id,
    Category,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateCategoryToCrateIden {
    #[iden = "crate_category_to_crate"]
    Table,
    Id,
    CategoryFk,
    CrateFk,
}

#[derive(Iden, Copy, Clone)]
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

#[derive(Iden, Copy, Clone)]
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

#[derive(Iden, Copy, Clone)]
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

#[derive(Iden, Copy, Clone)]
pub enum CratesIoMetaIden {
    #[iden = "cratesio_meta"]
    Table,
    Id,
    Version,
    Downloads,
    CratesIoFk,
    Documentation,
}

#[derive(Iden, Copy, Clone)]
pub enum GroupIden {
    #[iden = "group"]
    Table,
    Id,
    Name,
}

#[derive(Iden, Copy, Clone)]
pub enum GroupUserIden {
    #[iden = "group_user"]
    Table,
    Id,
    #[iden = "group_fk"]
    GroupFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden, Copy, Clone)]
pub enum CrateGroupIden {
    #[iden = "crate_group"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "group_fk"]
    GroupFk,
}

#[derive(Iden, Copy, Clone)]
pub enum OAuth2IdentityIden {
    #[iden = "oauth2_identity"]
    Table,
    Id,
    #[iden = "user_fk"]
    UserFk,
    ProviderIssuer,
    Subject,
    Email,
    Created,
}

#[derive(Iden, Copy, Clone)]
pub enum OAuth2StateIden {
    #[iden = "oauth2_state"]
    Table,
    Id,
    State,
    PkceVerifier,
    Nonce,
    Created,
}

#[derive(Iden, Copy, Clone)]
pub enum ToolchainIden {
    #[iden = "toolchain"]
    Table,
    Id,
    Name,
    Version,
    Date,
    Channel,
    Created,
}

#[derive(Iden, Copy, Clone)]
pub enum ToolchainTargetIden {
    #[iden = "toolchain_target"]
    Table,
    Id,
    #[iden = "toolchain_fk"]
    ToolchainFk,
    Target,
    StoragePath,
    Hash,
    Size,
}
