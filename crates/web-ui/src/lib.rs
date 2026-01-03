pub mod crate_access;
pub mod error;
pub mod group;
pub mod session;
pub mod ui;
pub mod user;

#[cfg(test)]
mod test_helper {
    use std::borrow::Cow;

    use cookie::{Cookie, CookieJar};

    pub(crate) const TEST_KEY: &[u8] = &[1; 64];

    // there has to be a better way to set cookies, i really don't like importing cookie crate just to do this
    pub(crate) fn encode_cookies<
        const N: usize,
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    >(
        cookies: [(K, V); N],
    ) -> String {
        let mut clear = CookieJar::new();
        let mut jar = clear.private_mut(&TEST_KEY.try_into().unwrap());
        for (k, v) in cookies {
            jar.add(Cookie::new(k, v));
        }
        clear
            .iter()
            .map(|c| c.encoded().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}
