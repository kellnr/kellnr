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
            use testcontainers::core::BuildImageOptions;
            use testcontainers::runners::{AsyncBuilder, AsyncRunner};
            // The image is built from `tests/fixtures/test-gcs-storage`, see `crates/storage/tests/gcs_image/mod.rs`.
            // Building is skipped once the image exists, so only the first run pays for it.
            let container = image::FakeGcsServerImage
                .build_image_with(BuildImageOptions::new().with_skip_if_exists(true))
                .await.expect("Failed to build fake-gcs-server image")
                .start()
                .await.expect("Failed to start fake-gcs-server container");
            let port = container
                .get_host_port_ipv4(image::FakeGcsServer::CONTAINER_PORT)
                .await.expect("Failed to read the port of fake-gcs-server");
            #(#stmts)*
        }
    };

    output.into()
}
