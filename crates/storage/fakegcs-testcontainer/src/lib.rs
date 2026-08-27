use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn fakegcs_testcontainer(_attr: TokenStream, stream: TokenStream) -> TokenStream {
    let input = parse_macro_input!(stream as ItemFn);

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
        modifiers: _,
    } = input;

    let stmts = &block.stmts;

    let output = quote! {
        #(#attrs)* #vis #sig {
            use testcontainers::ImageExt;
            use testcontainers::runners::AsyncRunner;
            // `fake-gcs-server`'s `-public-host` must match the host:port the test will
            // actually connect through, so the port has to be known before the container
            // starts (rather than reading back a Docker-assigned random port afterwards).
            // Reserve one by briefly binding to it, then hand it to the image so it can be
            // baked into `-public-host`, and fix the container's published port to match.
            let port = {
                let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
                    .expect("Failed to reserve a free port for fake-gcs-server");
                listener.local_addr().expect("Failed to read reserved port").port()
            };
            let container = image::FakeGcsServer::new(port)
                .with_mapped_port(port, image::FakeGcsServer::CONTAINER_PORT)
                .start()
                .await.expect("Failed to start fake-gcs-server container");
            #(#stmts)*
        }
    };

    output.into()
}
