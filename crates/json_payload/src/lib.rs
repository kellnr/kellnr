use proc_macro2::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn json_payload(
    _metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();
    let output = quote! {
        #[derive(Debug, Clone, PartialEq, Eq, rocket::serde::Serialize, rocket::serde::Deserialize, derive_jsonresponder::JsonResponder)]
        #input
    };
    output.into()
}
