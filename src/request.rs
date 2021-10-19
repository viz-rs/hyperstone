use crate::{async_trait, header, Body, Error, Request};
use futures_util::stream::{Stream, StreamExt};

#[async_trait]
pub trait RequestExt {
    fn query_string(&self) -> &str;

    fn content_length(&self) -> Option<u64>;

    fn content_type(&self) -> Option<mime::Mime>;

    fn header<T>(&self, key: impl AsRef<str>) -> Option<T>
    where
        T: std::str::FromStr;

    async fn bytes<T>(stream: T) -> anyhow::Result<bytes::Bytes>
    where
        T: Send + Stream<Item = Result<bytes::Bytes, Error>> + Unpin;

    #[cfg(feature = "json")]
    async fn json<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned;

    #[cfg(feature = "form")]
    async fn form<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned;

    #[cfg(feature = "query")]
    fn query<T>(&self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned;

    #[cfg(feature = "multipart")]
    fn multipart(self) -> anyhow::Result<form_data::FormData<Body>>;

    #[cfg(feature = "cookie")]
    fn cookie_jar(&mut self) -> anyhow::Result<cookie::CookieJar>;

    #[cfg(feature = "cookie")]
    fn cookie(&mut self, name: impl AsRef<str>) -> Option<cookie::Cookie<'static>>;
}

#[async_trait]
impl RequestExt for Request<Body> {
    fn query_string(&self) -> &str {
        self.uri().query().unwrap_or_default().as_ref()
    }

    fn content_length(&self) -> Option<u64> {
        self.header(header::CONTENT_LENGTH)
    }

    fn content_type(&self) -> Option<mime::Mime> {
        self.header(header::CONTENT_TYPE)
    }

    fn header<T>(&self, key: impl AsRef<str>) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.headers()
            .get(key.as_ref())
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<T>().ok())
    }

    async fn bytes<T>(mut stream: T) -> anyhow::Result<bytes::Bytes>
    where
        T: Send + Stream<Item = Result<bytes::Bytes, Error>> + Unpin,
    {
        let mut body = bytes::BytesMut::with_capacity(8192);

        while let Some(item) = stream.next().await {
            body.extend_from_slice(&item?);
        }

        Ok(body.freeze())
    }

    #[cfg(feature = "json")]
    async fn json<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let valid = self
            .content_type()
            .filter(|m| {
                m.type_() == mime::APPLICATION
                    && (m.subtype() == mime::JSON || m.suffix() == Some(mime::JSON))
            })
            .is_some();

        anyhow::ensure!(valid, "Content-Type is not JSON");

        serde_json::from_slice(&Self::bytes(self.into_body()).await?).map_err(Into::into)
    }

    #[cfg(feature = "form")]
    async fn form<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let valid = self
            .content_type()
            .filter(|m| m.type_() == mime::APPLICATION && m.subtype() == mime::WWW_FORM_URLENCODED)
            .is_some();

        anyhow::ensure!(valid, "Content-Type is not Form");

        serde_urlencoded::from_reader(bytes::Buf::reader(Self::bytes(self.into_body()).await?))
            .map_err(Into::into)
    }

    #[cfg(feature = "query")]
    fn query<T>(&self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_urlencoded::from_str(self.query_string()).map_err(Into::into)
    }

    #[cfg(feature = "multipart")]
    fn multipart(self) -> anyhow::Result<form_data::FormData<Body>> {
        let m = self
            .content_type()
            .filter(|m| m.type_() == mime::APPLICATION && m.subtype() == mime::MULTIPART)
            .ok_or_else(|| anyhow::anyhow!("Content-Type is not Multipart"))?;

        let b = m
            .get_param(mime::BOUNDARY)
            .ok_or_else(|| anyhow::anyhow!("Missing Boundary"))?;

        Ok(form_data::FormData::with_limits(
            self.into_body(),
            b.as_str(),
            form_data::Limits::default(),
        ))
    }

    #[cfg(feature = "cookie")]
    fn cookie_jar(&mut self) -> anyhow::Result<cookie::CookieJar> {
        if let Some(jar) = self.extensions().get::<cookie::CookieJar>().cloned() {
            return Ok(jar);
        }

        let mut jar = cookie::CookieJar::new();

        if let Some::<header::HeaderValue>(cookie) = self.header(header::COOKIE) {
            for pair in cookie.to_str().map_err(anyhow::Error::new)?.split(';') {
                jar.add_original(
                    cookie::Cookie::parse_encoded(pair.trim().to_string())
                        .map_err(anyhow::Error::new)?,
                )
            }
        }

        self.extensions_mut()
            .insert::<cookie::CookieJar>(jar.clone());

        Ok(jar)
    }

    #[cfg(feature = "cookie")]
    fn cookie(&mut self, name: impl AsRef<str>) -> Option<cookie::Cookie<'static>> {
        self.cookie_jar()
            .ok()
            .and_then(|jar| jar.get(name.as_ref()).cloned())
    }
}

#[cfg(test)]
mod tests {
    use crate::{header, Body, Method, Request, RequestExt};
    use anyhow::Result;
    use serde::Deserialize;

    #[test]
    fn request() -> Result<()> {
        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/?offset=10&limit=10")
            .body(Into::<Body>::into(""))
            .unwrap();

        #[derive(Debug, Deserialize)]
        struct Query {
            offset: usize,
            limit: usize,
        }

        req.headers_mut().insert(header::COOKIE, {
            let cookie = cookie::Cookie::new("viz.id", "123 321");
            header::HeaderValue::from_str(&cookie.encoded().to_string()).unwrap()
        });

        let size = req.content_length();
        let mime = req.content_type();
        let cookie = req.cookie("viz.id");
        let query = req.query::<Query>()?;

        dbg!(size, mime, cookie, query);

        Ok(())
    }
}
