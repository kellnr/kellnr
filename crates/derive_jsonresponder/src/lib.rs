use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(JsonResponder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_derive(&ast)
}

fn impl_derive(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl<'r> rocket::response::Responder<'r, 'static> for #name {
            fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'static> {
                let json = serde_json::to_string_pretty(&self).unwrap();
                rocket::response::Response::build()
                    .header(rocket::http::ContentType::new("application", "json"))
                    .sized_body(None, std::io::Cursor::new(json))
                    .ok()
            }
        }
    };
    gen.into()
}
