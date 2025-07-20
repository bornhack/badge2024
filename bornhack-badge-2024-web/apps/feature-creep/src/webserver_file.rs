use picoserve::response::IntoResponse;

/// [RequestHandler] that serves a single file.
#[derive(Debug, Clone)]
pub struct File {
    content_type: &'static str,
    body: &'static [u8],
    headers: &'static [(&'static str, &'static str)],
}

impl File {
    /// Create a file with the given content type but no additional headers.
    pub const fn with_content_type(content_type: &'static str, body: &'static [u8]) -> Self {
        Self {
            content_type,
            body,
            headers: &[],
        }
    }

    /// A HyperText Markup Language file with a MIME type of "text/html; charset=utf-8"
    pub const fn html(body: &'static str) -> Self {
        Self::with_content_type("text/html; charset=utf-8", body.as_bytes())
    }

    /// Cascading StyleSheets file with a MIME type of "text/css"
    pub const fn css(body: &'static str) -> Self {
        Self::with_content_type("text/css", body.as_bytes())
    }

    /// A Javascript file with a MIME type of "application/javascript; charset=utf-8"
    pub const fn javascript(body: &'static str) -> Self {
        Self::with_content_type("application/javascript; charset=utf-8", body.as_bytes())
    }

    /// Convert into a [super::Response] with a status code of "OK"
    pub fn into_response(
        self,
    ) -> picoserve::response::Response<
        impl picoserve::response::HeadersIter,
        impl picoserve::response::Body,
    > {
        let headers = self.headers;
        picoserve::response::Response::ok(self).with_headers(headers)
    }
}

impl<State, PathParameters> picoserve::routing::RequestHandlerService<State, PathParameters>
    for File
{
    async fn call_request_handler_service<
        R: picoserve::io::Read,
        W: picoserve::response::ResponseWriter<Error = R::Error>,
    >(
        &self,
        _state: &State,
        _path_parameters: PathParameters,
        request: picoserve::request::Request<'_, R>,
        response_writer: W,
    ) -> Result<picoserve::ResponseSent, W::Error> {
        self.clone()
            .write_to(request.body_connection.finalize().await?, response_writer)
            .await
    }
}

impl picoserve::response::Content for File {
    fn content_type(&self) -> &'static str {
        self.content_type
    }

    fn content_length(&self) -> usize {
        self.body.len()
    }

    async fn write_content<R: picoserve::io::Read, W: picoserve::io::Write>(
        self,
        _connection: picoserve::response::Connection<'_, R>,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(self.body).await
    }
}

impl picoserve::response::IntoResponse for File {
    async fn write_to<
        R: picoserve::io::Read,
        W: picoserve::response::ResponseWriter<Error = R::Error>,
    >(
        self,
        connection: picoserve::response::Connection<'_, R>,
        response_writer: W,
    ) -> Result<picoserve::ResponseSent, W::Error> {
        response_writer
            .write_response(connection, self.into_response())
            .await
    }
}

impl core::future::IntoFuture for File {
    type Output = Self;
    type IntoFuture = core::future::Ready<Self>;

    fn into_future(self) -> Self::IntoFuture {
        core::future::ready(self)
    }
}
