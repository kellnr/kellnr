use error::error::{ApiError, ApiResult};
use rocket::data::{FromData, Outcome, ToByteUnit};
use rocket::http::Status;
use rocket::outcome::Outcome::{Failure, Success};
use rocket::{Data, Request};
use std::io::Cursor;
use std::path::Path;
use zip::ZipArchive;

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub struct DocArchive(Zip);

impl DocArchive {
    pub fn extract(&mut self, path: &Path) -> ApiResult<()> {
        match self.0.extract(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::from(e)),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromData<'r> for DocArchive {
    type Error = ApiError;

    async fn from_data(_: &'r Request<'_>, data: Data<'r>) -> Outcome<'r, Self> {
        let settings = match settings::get_settings() {
            Ok(s) => s,
            Err(e) => {
                return Failure((Status::InternalServerError, ApiError::from(&e.to_string())))
            }
        };

        let stream = match data
            .open(settings.max_docs_size.megabytes())
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

        let reader = Cursor::new(stream);

        match ZipArchive::new(reader) {
            Ok(zip) => Success(DocArchive(zip)),
            Err(e) => Failure((Status::Ok, ApiError::from(e))),
        }
    }
}
