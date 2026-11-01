use utoipa::ToSchema;

// Describes binary request bodies, like uploaded archives, in the OpenAPI
// document. A `Vec<u8>` would be described as an array of integers.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub struct BinaryBody(pub Vec<u8>);
