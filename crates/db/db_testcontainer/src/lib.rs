use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn db_test(_attr: TokenStream, stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as ItemFn);

    let ItemFn {
        attrs: _,
        vis,
        sig,
        block,
    } = input;

    // Clone the function signature for our implementation function
    let impl_sig = sig.clone();
    let fn_name = &sig.ident;
    let sqlite_fn_name = quote::format_ident!("sqlite_{fn_name}");
    let postgres_fn_name = quote::format_ident!("postgres_{fn_name}");
    let _test_impl_name = quote::format_ident!("{fn_name}_impl");
    let _stmts = &block.stmts;

    let output = quote! {
        // Keep the original function as the implementation
        #[allow(unused)]
        #vis #impl_sig #block

        // SQLite version of the test
        #[tokio::test]
        #vis async fn #sqlite_fn_name() {
            use std::path;
            use std::ops::Add;
            use common::util::generate_rand_string;

            let path = path::PathBuf::from("/tmp").join(generate_rand_string(8).add(".db"));
            let con_string = db::SqliteConString {
                path: path.to_owned(),
                salt: "salt".to_string(),
                admin_pwd: "123".to_string(),
                admin_token: "token".to_string(),
                session_age: std::time::Duration::from_secs(1),
            };
            let con_string = db::ConString::Sqlite(con_string);
            let test_db = db::Database::new(&con_string, 10).await.unwrap();

            // Run the test with SQLite
            #fn_name(&test_db).await;

            // Clean up
            rm_rf::remove(&path).expect("Cannot remove test db");
        }

        // PostgreSQL version of the test
        #[tokio::test]
        #vis async fn #postgres_fn_name() {
            use testcontainers::runners::AsyncRunner;

            let pg_container = image::Postgres::default().start().await.expect("Failed to start postgres container");
            let port = pg_container.get_host_port_ipv4(image::Postgres::PG_PORT).await.expect("Failed to get port");
            let admin = db::AdminUser::new("123".to_string(), "token".to_string(), "salt".to_string());
            let pg_db = db::PgConString::new("localhost", port, "kellnr", "admin", "admin", admin);
            let pg_db = db::ConString::Postgres(pg_db);
            let test_db = db::Database::new(&pg_db, 10).await.unwrap();

            // Run the test with PostgreSQL
            #fn_name(&test_db).await;
        }
    };

    output.into()
}
