use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn minio_testcontainer(_attr: TokenStream, stream: TokenStream) -> TokenStream {
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
            let container = image::Minio::default()
                .start()
                .await.expect("Failed to start Minio container");
            let port = container.get_host_port_ipv4(image::Minio::PORT).await.expect("Failed to get port");
            #(#stmts)*
        }
    };

    output.into()
}
