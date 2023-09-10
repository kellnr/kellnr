use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{Responder, Response};
use rocket::serde::Deserialize;
use std::io::Cursor;

#[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Prefetch {
    pub data: Vec<u8>,
    pub etag: String,
    pub last_modified: String,
}

impl<'r> Responder<'r, 'static> for Prefetch {
    fn respond_to(self, _request: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .raw_header("Content-Length", self.data.len().to_string())
            .raw_header("ETag", self.etag)
            .raw_header("Last-Modified", self.last_modified)
            .sized_body(None, Cursor::new(self.data))
            .ok()
    }
}

/*
    The part below is for the unfinished http registry.
    It is not final and may change!
*/
#[derive(Debug)]
pub struct Headers {
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Headers {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let if_modified_since = request
            .headers()
            .get_one("if-modified-since")
            .map(|h| h.to_string());
        let if_none_match = request
            .headers()
            .get_one("if-none-match")
            .map(|h| h.to_string());

        Outcome::Success(Headers {
            if_none_match,
            if_modified_since,
        })
    }
}

#[cfg(test)]
mod tests {
    // Indirectly tested through API access to higher level functions
}
