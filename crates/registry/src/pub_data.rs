use common::publish_metadata::PublishMetadata;
use error::error::ApiError;
use rocket::data::{self, FromData, ToByteUnit};
use rocket::http::Status;
use rocket::serde::json;
use rocket::{outcome::Outcome::*, Data, Request};
use settings::constants::MIN_BODY_CRATE_AND_DOC_BYTES;

#[derive(Debug, PartialEq, Eq)]
pub struct PubData {
    pub metadata_length: u32,
    pub metadata: PublishMetadata,
    pub crate_length: u32,
    pub cratedata: Vec<u8>,
}

fn convert_raw_metadata_to_string(raw_data: &[u8]) -> Result<String, ApiError> {
    match String::from_utf8((raw_data).to_vec()) {
        Ok(s) => Ok(s),
        Err(e) => Err(ApiError::new("Invalid raw metadata.", &e.to_string())),
    }
}

fn deserialize_metadata(raw_data: &[u8]) -> Result<PublishMetadata, ApiError> {
    let metadata_string = convert_raw_metadata_to_string(raw_data)?;

    match json::from_str(&metadata_string) {
        Ok(md) => Ok(md),
        Err(e) => Err(ApiError::new("Invalid metadata string.", &e.to_string())),
    }
}

fn convert_length(raw_data: &[u8]) -> Result<u32, ApiError> {
    match std::convert::TryInto::try_into(raw_data) {
        Ok(i) => Ok(u32::from_le_bytes(i)),
        Err(e) => Err(ApiError::new("Invalid metadata length.", &e.to_string())),
    }
}

#[rocket::async_trait]
impl<'r> FromData<'r> for PubData {
    type Error = ApiError;

    async fn from_data(_: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        // Dump whole request data for debugging
        // let mut file = std::fs::File::create("pub_data.bin").unwrap();
        // file.write_all(&data_bytes).unwrap();

        let settings = match settings::get_settings() {
            Ok(s) => s,
            Err(e) => {
                return Failure((Status::InternalServerError, ApiError::from(&e.to_string())))
            }
        };

        let data_bytes = match data
            .open(settings.max_crate_size.megabytes())
            .into_bytes()
            .await
        {
            Ok(v) => {
                if v.is_complete() {
                    v.into_inner()
                } else {
                    return Failure((Status::Ok, ApiError::from("Unable to read full data.")));
                }
            }
            Err(e) => return Failure((Status::Ok, ApiError::from(e))),
        };

        if data_bytes.len() < MIN_BODY_CRATE_AND_DOC_BYTES {
            return Failure((
                Status::Ok,
                ApiError::from(&format!(
                    "Invalid min. length. {}/{} bytes.",
                    data_bytes.len(),
                    MIN_BODY_CRATE_AND_DOC_BYTES
                )),
            ));
        }

        let metadata_length = match convert_length(&data_bytes[0..4]) {
            Ok(l) => l,
            Err(e) => return Failure((Status::Ok, e)),
        };
        let metadata_end = 4 + (metadata_length as usize);

        if metadata_end >= data_bytes.len() {
            return Failure((Status::Ok, ApiError::from("Invalid metadata size.")));
        }

        let metadata: PublishMetadata = match deserialize_metadata(&data_bytes[4..metadata_end]) {
            Ok(md) => md,
            Err(e) => return Failure((Status::Ok, e)),
        };
        let crate_length = match convert_length(&data_bytes[metadata_end..(metadata_end + 4)]) {
            Ok(l) => l,
            Err(e) => return Failure((Status::Ok, e)),
        };
        let crate_end = metadata_end + 4 + (crate_length as usize);
        let cratedata: Vec<u8> = data_bytes[metadata_end + 4..crate_end].to_vec();

        let pub_data = PubData {
            metadata_length,
            metadata,
            crate_length,
            cratedata,
        };

        Success(pub_data)
    }
}
