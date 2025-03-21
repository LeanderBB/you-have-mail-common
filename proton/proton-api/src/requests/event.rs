use http::{Method, RequestBuilder};
use serde::Deserialize;

#[doc(hidden)]
#[derive(Deserialize)]
#[cfg_attr(feature = "mocks", derive(serde::Serialize))]
pub struct GetLatestEventResponse {
    #[serde(rename = "EventID")]
    pub event_id: crate::domain::event::Id,
}

#[derive(Copy, Clone)]
pub struct GetLatestEventRequest;

impl http::Request for GetLatestEventRequest {
    type Response = http::JsonResponse<GetLatestEventResponse>;
    const METHOD: Method = Method::Get;

    fn url(&self) -> String {
        "core/v4/events/latest".to_owned()
    }

    fn build(&self, builder: RequestBuilder) -> http::Result<RequestBuilder> {
        Ok(builder)
    }
}

pub struct GetEventRequest<'a> {
    event_id: &'a crate::domain::event::Id,
}

impl<'a> GetEventRequest<'a> {
    #[must_use]
    pub fn new(id: &'a crate::domain::event::Id) -> Self {
        Self { event_id: id }
    }
}

impl http::Request for GetEventRequest<'_> {
    type Response = http::JsonResponse<crate::domain::event::Event>;
    const METHOD: Method = Method::Get;

    fn url(&self) -> String {
        format!("core/v4/events/{}", self.event_id)
    }
    fn build(&self, builder: RequestBuilder) -> http::Result<RequestBuilder> {
        Ok(builder)
    }
}
