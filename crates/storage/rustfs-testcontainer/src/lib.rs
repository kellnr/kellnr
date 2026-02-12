use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn rustfs_testcontainer(_attr: TokenStream, stream: TokenStream) -> TokenStream {
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
            let container = image::RustFs::default()
                .start()
                .await.expect("Failed to start RustFS container");
            let port = container.get_host_port_ipv4(image::RustFs::PORT).await.expect("Failed to get port");
            #(#stmts)*
        }
    };

    output.into()
}
