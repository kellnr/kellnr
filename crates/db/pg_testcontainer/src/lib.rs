use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn pg_testcontainer(_attr: TokenStream, stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as ItemFn);

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = input;

    let stmts = &block.stmts;

    let output = quote! {
        #(#attrs)* #vis #sig {
            use testcontainers::runners::AsyncRunner;
            let pg_container = image::Postgres::default().start().await.expect("Failed to start postgres container");
            let port = pg_container.get_host_port_ipv4(image::Postgres::PG_PORT).await.expect("Failed to get port");
            let admin = db::AdminUser::new("123".to_string(), "token".to_string(), "salt".to_string());
            let pg_db = db::PgConString::new("localhost", port, "kellnr", "admin", "admin", admin);
            let pg_db = db::ConString::Postgres(pg_db);
            //let pg_db = db::ConString::new("localhost", 5432, "postgres", "admin", "admin", "admin", "token", "salt");
            let test_db = db::Database::new(&pg_db, 10).await.unwrap();
            #(#stmts)*
        }
    };

    output.into()
}
