pub mod crate_access;
pub mod error;
pub mod session;
pub mod ui;
pub mod user;

#[cfg(test)]
mod test_helper {
    use cookie::{Cookie, CookieJar};
    use std::borrow::Cow;

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
        cookies
            .into_iter()
            .for_each(|(k, v)| jar.add(Cookie::new(k, v)));
        clear
            .iter()
            .map(|c| c.encoded().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}
