//! Test helper to generate signed cookies matching `PrivateCookieJar` behavior.

#[cfg(test)]
pub(crate) mod cookies {
    use std::borrow::Cow;

    use cookie::{Cookie, CookieJar};

    // Match the key used in tests that set `AppStateData.signing_key`.
    pub(crate) const TEST_KEY: &[u8] = &[1; 64];


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
