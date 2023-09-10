use json_payload::json_payload;

#[json_payload]
#[derive(Default)]
pub struct PubDataSuccess {
    pub warnings: Option<Warnings>,
}

#[json_payload]
pub struct Warnings {
    pub invalid_categories: Option<Vec<String>>,
    pub invalid_badges: Option<Vec<String>>,
    pub other: Option<Vec<String>>,
}

impl PubDataSuccess {
    pub fn new() -> Self {
        Self::default()
    }
}
