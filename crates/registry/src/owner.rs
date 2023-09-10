use error::error::ApiError;
use rocket::data::{self, FromData, ToByteUnit};
use rocket::http::Status;
use rocket::outcome::try_outcome;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::io::AsyncReadExt;
use rocket::{outcome::Outcome::*, Data, Request};

const DATA_MAX: u64 = 1024 * 10;
const DATA_MIN: u64 = 10; // real min is probably much higher

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    pub id: i32,
    pub login: String,
    pub name: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerList {
    pub users: Vec<Owner>,
}

impl From<Vec<Owner>> for OwnerList {
    fn from(users: Vec<Owner>) -> Self {
        Self { users }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRequest {
    pub users: Vec<String>,
}

#[rocket::async_trait]
impl<'r> FromData<'r> for OwnerRequest {
    type Error = ApiError;

    async fn from_data(_: &'r Request<'_>, data: Data<'r>) -> data::Outcome<'r, Self> {
        let data_bytes = try_outcome!(read_data(data).await);
        let add_owner_request = match serde_json::from_slice::<OwnerRequest>(&data_bytes) {
            Ok(r) => r,
            Err(e) => {
                return Failure((
                    Status::Ok,
                    ApiError::new("Unable to add owners. Invalid data.", &e.to_string()),
                ))
            }
        };

        Success(add_owner_request)
    }
}

async fn read_data(data: Data<'_>) -> data::Outcome<'_, Vec<u8>, ApiError> {
    let mut data_bytes: Vec<u8> = vec![];
    match data
        .open(DATA_MAX.bytes())
        .read_to_end(&mut data_bytes)
        .await
    {
        Ok(size_read) => {
            if (size_read as u64) == DATA_MAX {
                Failure((
                    Status::Ok,
                    ApiError::from(&format!(
                        "Invalid max. length of sent bytes {}/{}.",
                        size_read, DATA_MAX
                    )),
                ))
            } else if (size_read as u64) <= DATA_MIN {
                Failure((
                    Status::Ok,
                    ApiError::from(&format!(
                        "Invalid min. length of sent bytes {}/{}.",
                        size_read, DATA_MIN
                    )),
                ))
            } else {
                Success(data_bytes)
            }
        }
        Err(e) => Failure((Status::Ok, ApiError::new("Invalid data.", &e.to_string()))),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerResponse {
    pub ok: bool,
    pub msg: String,
}

impl From<&str> for OwnerResponse {
    fn from(msg: &str) -> Self {
        Self {
            ok: true,
            msg: msg.to_string(),
        }
    }
}
