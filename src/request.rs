use crate::{async_trait, header, Body, Error, Request};
use futures_util::stream::{Stream, StreamExt};

#[async_trait]
pub trait RequestExt {
    fn query_str(&self) -> &str;

    fn size(&self) -> Option<u64>;

    fn mime(&self) -> Option<mime::Mime>;

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
    fn query<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned;

    #[cfg(feature = "multipart")]
    fn multipart(self) -> anyhow::Result<form_data::FormData<Body>>;
}

#[async_trait]
impl RequestExt for Request<Body> {
    fn query_str(&self) -> &str {
        self.uri().query().unwrap_or_default().as_ref()
    }

    fn size(&self) -> Option<u64> {
        self.header(header::CONTENT_LENGTH)
    }

    fn mime(&self) -> Option<mime::Mime> {
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
            .mime()
            .filter(|m| {
                m.type_() == mime::APPLICATION
                    && (m.subtype() == mime::JSON || m.suffix() == Some(mime::JSON))
            })
            .is_some();

        anyhow::ensure!(valid, "Content-Type is not JSON");

        serde_json::from_slice(&Self::bytes(self.into_body()).await?).map_err(anyhow::Error::new)
    }

    #[cfg(feature = "form")]
    async fn form<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let valid = self
            .mime()
            .filter(|m| m.type_() == mime::APPLICATION && m.subtype() == mime::WWW_FORM_URLENCODED)
            .is_some();

        anyhow::ensure!(valid, "Content-Type is not Form");

        serde_urlencoded::from_reader(bytes::Buf::reader(Self::bytes(self.into_body()).await?))
            .map_err(anyhow::Error::new)
    }

    #[cfg(feature = "query")]
    fn query<T>(self) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_urlencoded::from_str(self.query_str()).map_err(anyhow::Error::new)
    }

    #[cfg(feature = "multipart")]
    fn multipart(self) -> anyhow::Result<form_data::FormData<Body>> {
        let m = self
            .mime()
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
}
