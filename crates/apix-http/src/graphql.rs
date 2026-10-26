//! Fetches a GraphQL schema by sending the introspection query to the
//! request's own URL, with its headers, auth and settings.

use apix_core::graphql::{INTROSPECTION_QUERY, Schema};
use apix_core::{Body, GraphQl, Method, Request};

use crate::{HttpClient, HttpError};

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server answered, but not with a schema.
    #[error("{0}")]
    Schema(String),
}

/// A schema and the introspection JSON it was read from (to keep on disk).
#[derive(Debug, Clone)]
pub struct Fetched {
    pub schema: Schema,
    pub json: String,
}

/// The request that asks for the schema: `req` (already resolved) with its
/// body replaced by the introspection query, sent as a POST.
pub fn introspection_request(req: &Request) -> Request {
    let mut req = req.clone();
    req.method = Method::Post;
    req.body = Body::GraphQl(GraphQl {
        query: INTROSPECTION_QUERY.to_owned(),
        variables: String::new(),
    });
    req
}

/// Reads a schema from a server's answer to the introspection query.
pub fn read_schema(status: u16, body: &str) -> Result<Fetched, SchemaError> {
    match Schema::from_introspection(body) {
        Ok(schema) => Ok(Fetched {
            schema,
            json: body.to_owned(),
        }),
        Err(_) if !(200..300).contains(&status) => {
            let mut text = body.trim().to_owned();
            if text.len() > 200 {
                let cut = (0..=200)
                    .rev()
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(0);
                text.truncate(cut);
                text.push('…');
            }
            Err(SchemaError::Schema(if text.is_empty() {
                format!("the server answered {status}")
            } else {
                format!("the server answered {status}: {text}")
            }))
        }
        Err(reason) => Err(SchemaError::Schema(reason)),
    }
}

/// Sends the introspection query with `req`'s URL, headers and auth.
pub async fn fetch_schema(client: &HttpClient, req: &Request) -> Result<Fetched, SchemaError> {
    let response = client.send(&introspection_request(req)).await?;
    read_schema(response.status, &response.raw_body())
}
