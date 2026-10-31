use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// Runs the test once on SQLite and once on PostgreSQL, each time with a
/// fresh, fully migrated database from `kellnr_test_db`.
#[proc_macro_attribute]
pub fn db_test(_attr: TokenStream, stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as ItemFn);

    let ItemFn {
        attrs: _,
        vis,
        sig,
        block,
        modifiers: _,
    } = input;

    // Clone the function signature for our implementation function
    let impl_sig = sig.clone();
    let fn_name = &sig.ident;
    let sqlite_fn_name = quote::format_ident!("sqlite_{fn_name}");
    let postgres_fn_name = quote::format_ident!("postgres_{fn_name}");

    let output = quote! {
        // Keep the original function as the implementation
        #[allow(unused)]
        #vis #impl_sig #block

        // SQLite version of the test
        #[tokio::test]
        #vis async fn #sqlite_fn_name() {
            let test_db = kellnr_test_db::TestDb::sqlite().await;
            #fn_name(&test_db).await;
        }

        // PostgreSQL version of the test
        #[tokio::test]
        #vis async fn #postgres_fn_name() {
            let test_db = kellnr_test_db::TestDb::postgres().await;
            #fn_name(&test_db).await;
        }
    };

    output.into()
}
