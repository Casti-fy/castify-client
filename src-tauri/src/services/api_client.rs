use std::sync::Arc;

use reqwest::{Client, RequestBuilder};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppError;
use crate::models::ErrorResponse;

pub struct ApiClient {
    client: Client,
    base_url: String,
    token: Option<String>,
    on_unauthorized: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ApiClient {
    pub fn new(base_url: &str, token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            token,
            on_unauthorized: None,
        }
    }

    pub fn set_on_unauthorized(&mut self, cb: Arc<dyn Fn() + Send + Sync>) {
        self.on_unauthorized = Some(cb);
    }

    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn build_request<B: Serialize>(
        &self,
        path: &str,
        method: &str,
        body: Option<&B>,
        auth: bool,
    ) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = match method {
            "POST" => self.client.post(&url),
            "PATCH" => self.client.patch(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => self.client.get(&url),
        };

        req = req.header("Content-Type", "application/json");

        if auth {
            if let Some(token) = &self.token {
                req = req.bearer_auth(token);
            }
        }

        if let Some(b) = body {
            req = req.json(b);
        }

        req
    }

    async fn send_and_check(&self, req: RequestBuilder) -> Result<reqwest::Response, AppError> {
        let resp = req.send().await?;
        let status = resp.status().as_u16();

        if status == 401 {
            if let Some(cb) = &self.on_unauthorized {
                cb();
            }
            return Err(AppError::Unauthorized);
        }

        if status >= 400 {
            if let Ok(err) = resp.json::<ErrorResponse>().await {
                return Err(AppError::Api(err.error));
            }
            return Err(AppError::Status(status));
        }

        Ok(resp)
    }

    pub async fn request<T: DeserializeOwned>(
        &self,
        path: &str,
        method: &str,
        auth: bool,
    ) -> Result<T, AppError> {
        self.request_with_body::<T, ()>(path, method, None, auth)
            .await
    }

    pub async fn request_with_body<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        method: &str,
        body: Option<&B>,
        auth: bool,
    ) -> Result<T, AppError> {
        let req = self.build_request(path, method, body, auth);
        let resp = self.send_and_check(req).await?;
        resp.json::<T>().await.map_err(|e| AppError::Network(e.to_string()))
    }

    pub async fn request_no_content<B: Serialize>(
        &self,
        path: &str,
        method: &str,
        body: Option<&B>,
        auth: bool,
    ) -> Result<(), AppError> {
        let req = self.build_request(path, method, body, auth);
        self.send_and_check(req).await?;
        Ok(())
    }
}
