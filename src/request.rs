use crate::{async_trait, header, Body, Error, Request};
use bytes::Buf;
use form_data::{FormData, Limits};
use futures_util::stream::{Stream, StreamExt};

#[async_trait]
pub trait RequestExt {
    fn query_str(&self) -> &str;

    fn size(&self) -> Option<u64>;

    fn mime(&self) -> Option<mime::Mime>;

    fn header<T: std::str::FromStr>(&self, key: impl AsRef<str>) -> Option<T>;

    async fn bytes<B>(stream: B) -> anyhow::Result<bytes::Bytes>
    where
        B: Send + Stream<Item = Result<bytes::Bytes, Error>> + Unpin;

    async fn json<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T>;

    async fn form<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T>;

    fn query<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T>;

    fn multipart(self) -> anyhow::Result<FormData<Body>>;
}

#[async_trait]
impl RequestExt for Request<Body> {
    fn query_str(&self) -> &str {
        if let Some(query) = self.uri().query().as_ref() {
            query
        } else {
            ""
        }
    }

    fn size(&self) -> Option<u64> {
        self.header(header::CONTENT_LENGTH)
    }

    fn mime(&self) -> Option<mime::Mime> {
        self.header(header::CONTENT_TYPE)
    }

    fn header<T: std::str::FromStr>(&self, key: impl AsRef<str>) -> Option<T> {
        self.headers()
            .get(key.as_ref())
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<T>().ok())
    }

    async fn bytes<B>(mut stream: B) -> anyhow::Result<bytes::Bytes>
    where
        B: Send + Stream<Item = Result<bytes::Bytes, Error>> + Unpin,
    {
        let mut body = bytes::BytesMut::with_capacity(8192);

        while let Some(item) = stream.next().await {
            let chunk = item?;
            body.extend_from_slice(&chunk);
        }

        Ok(body.freeze())
    }

    async fn json<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T> {
        let is_json = self
            .mime()
            .filter(|m| {
                m.type_() == mime::APPLICATION
                    && (m.subtype() == mime::JSON || m.suffix() == Some(mime::JSON))
            })
            .is_some();

        anyhow::ensure!(is_json, "Content-Type is not JSON");

        serde_json::from_slice(&Self::bytes(self.into_body()).await?).map_err(anyhow::Error::new)
    }

    async fn form<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T> {
        let is_form = self
            .mime()
            .filter(|m| m.type_() == mime::APPLICATION && m.subtype() == mime::WWW_FORM_URLENCODED)
            .is_some();

        anyhow::ensure!(is_form, "Content-Type is not Form");

        serde_urlencoded::from_reader(Self::bytes(self.into_body()).await?.reader())
            .map_err(anyhow::Error::new)
    }

    fn query<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T> {
        serde_urlencoded::from_str(self.query_str()).map_err(anyhow::Error::new)
    }

    fn multipart(self) -> anyhow::Result<FormData<Body>> {
        let m = self
            .mime()
            .ok_or_else(|| anyhow::anyhow!("Content-Type is not Multipart"))?;

        let boundary = m
            .get_param(mime::BOUNDARY)
            .ok_or_else(|| anyhow::anyhow!("Missing Boundary"))?;

        Ok(FormData::with_limits(
            self.into_body(),
            boundary.as_str(),
            Limits::default(),
        ))
    }
}
