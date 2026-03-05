#[cfg(target_arch = "wasm32")]
use std::{collections::HashMap, pin::Pin};

use crate::language::{Context, LspError, LspErrorCode};

#[cfg_attr(target_arch = "wasm32", derive(Default))]
pub struct RequestHandler(
    #[cfg(target_arch = "wasm32")]
    pub  HashMap<
        &'static str,
        Box<
            dyn Fn(
                &mut Context,
                serde_json::Value,
            )
                -> Pin<Box<dyn Future<Output = Result<serde_json::Value, LspError>>>>,
        >,
    >,
    #[cfg(not(target_arch = "wasm32"))] pub async_lsp::router::Router<std::sync::OnceLock<Context>>,
);

impl RequestHandler {
    #[cfg(target_arch = "wasm32")]
    pub fn register<
        R: lsp_types::request::Request,
        Fut: Future<Output = std::result::Result<R::Result, LspError>> + 'static,
    >(
        &mut self,
        handler: impl Fn(&mut Context, R::Params) -> Fut,
    ) where
        R::Params: 'static,
    {
        self.0.insert(
            R::METHOD,
            Box::new(move |ctx, value| {
                Box::pin(async move {
                    let params = serde_json::from_value(value).map_err(|e| LspError {
                        code: LspErrorCode::InvalidParameter,
                        message: format!("error when deserializing request: {e:?}"),
                    })?;
                    handler(params, ctx).await.map(|x| serde_json::to_value(x).unwrap())
                })
            }),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register<
        R: lsp_types::request::Request,
        Fut: Future<Output = Result<R::Result, LspError>> + Send + 'static,
    >(
        &mut self,
        handler: impl Fn(&mut Context, R::Params) -> Fut + Send + 'static,
    ) where
        R::Params: 'static,
    {
        self.0.request::<R, _>(move |ctx: &mut std::sync::OnceLock<Context>, params| {
            let fut = handler(ctx.get_mut().unwrap(), params);
            async move {
                fut.await.map_err(|err| {
                    async_lsp::ResponseError::new(
                        match err.code {
                            LspErrorCode::InvalidParameter => async_lsp::ErrorCode::INVALID_PARAMS,
                            LspErrorCode::InternalError => async_lsp::ErrorCode::INTERNAL_ERROR,
                            LspErrorCode::RequestFailed => async_lsp::ErrorCode::REQUEST_FAILED,
                            LspErrorCode::ContentModified => async_lsp::ErrorCode::CONTENT_MODIFIED,
                        },
                        err.message,
                    )
                })
            }
        });
    }
}
