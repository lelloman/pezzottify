//! Static application route declarations over the native HTTP engine.
pub use engine::*;
use simple_server::engine_web as engine;
pub type Request<B = Body> = http::Request<B>;
pub type Response<B = Body> = http::Response<B>;
pub mod extract {
    pub use simple_server::engine_web::extract::*;
    pub type Request<B = super::Body> = super::http::Request<B>;
}
pub mod body {
    pub use simple_server::engine_web::body::*;
    pub use simple_server::engine_web::Bytes;
    pub async fn to_bytes(body: Body, limit: usize) -> Result<super::Bytes, BodyError> {
        body.collect(limit).await
    }
}
#[derive(Clone)]
pub struct Router<S = ()>(engine::Router<S>);
impl<S: Clone + Send + Sync + 'static> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<S: Clone + Send + Sync + 'static> Router<S> {
    pub fn new() -> Self {
        Self(engine::Router::new().expect("native router"))
    }
    pub fn route(self, path: &str, methods: MethodRouter<S>) -> Self {
        Self(self.0.route(path, methods.0).expect("valid route"))
    }
    pub fn nest(self, path: &str, router: Self) -> Self {
        Self(self.0.nest(path, router.0).expect("valid route nesting"))
    }
    pub fn merge(self, router: Self) -> Self {
        Self(self.0.merge(router.0).expect("distinct routes"))
    }
    pub fn with_state<S2: Clone + Send + Sync + 'static>(self, state: S) -> Router<S2> {
        Router(self.0.with_state(state).expect("router state"))
    }
    pub fn fallback<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(self.0.fallback_handler(handler).expect("fallback"))
    }
    pub fn fallback_static_dir_with_index(
        self,
        path: impl AsRef<std::path::Path>,
        index: impl AsRef<std::path::Path>,
    ) -> Self {
        Self(
            self.0
                .fallback_static_dir_with_index(path, Some(index))
                .expect("static frontend"),
        )
    }
    pub fn layer<L, B>(self, layer: L) -> Self
    where
        L: engine::middleware::Layer<engine::Route> + Clone + Send + Sync + 'static,
        L::Service: engine::Service<
                engine::Request,
                Response = http::Response<B>,
                Error = std::convert::Infallible,
            > + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as engine::Service<engine::Request>>::Future: Send + 'static,
        B: engine::HttpBody<Data = engine::Bytes> + Send + 'static,
        B::Error: Into<engine::body::BoxError>,
    {
        Self(self.0.layer(layer).expect("valid middleware"))
    }
    pub fn route_layer<L, B>(self, layer: L) -> Self
    where
        L: engine::middleware::Layer<engine::Route> + Clone + Send + Sync + 'static,
        L::Service: engine::Service<
                engine::Request,
                Response = http::Response<B>,
                Error = std::convert::Infallible,
            > + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as engine::Service<engine::Request>>::Future: Send + 'static,
        B: engine::HttpBody<Data = engine::Bytes> + Send + 'static,
        B::Error: Into<engine::body::BoxError>,
    {
        Self(self.0.route_layer(layer).expect("valid middleware"))
    }
}
#[derive(Clone)]
pub struct MethodRouter<S = ()>(engine::MethodRouter<S>);
impl<S: Clone + Send + Sync + 'static> MethodRouter<S> {
    pub fn get<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::GET, handler)
                .expect("method handler"),
        )
    }
    pub fn post<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::POST, handler)
                .expect("method handler"),
        )
    }
    pub fn put<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::PUT, handler)
                .expect("method handler"),
        )
    }
    pub fn delete<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::DELETE, handler)
                .expect("method handler"),
        )
    }
    pub fn patch<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::PATCH, handler)
                .expect("method handler"),
        )
    }
    pub fn head<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::HEAD, handler)
                .expect("method handler"),
        )
    }
    pub fn options<H, T>(self, handler: H) -> Self
    where
        H: engine::Handler<T, S>,
        T: 'static,
    {
        Self(
            self.0
                .on_handler(engine::Method::OPTIONS, handler)
                .expect("method handler"),
        )
    }
    pub fn layer<L, B>(self, layer: L) -> Self
    where
        L: engine::middleware::Layer<engine::Route> + Clone + Send + Sync + 'static,
        L::Service: engine::Service<
                engine::Request,
                Response = http::Response<B>,
                Error = std::convert::Infallible,
            > + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as engine::Service<engine::Request>>::Future: Send + 'static,
        B: engine::HttpBody<Data = engine::Bytes> + Send + 'static,
        B::Error: Into<engine::body::BoxError>,
    {
        Self(self.0.layer(layer).expect("valid middleware"))
    }
    pub fn route_layer<L, B>(self, layer: L) -> Self
    where
        L: engine::middleware::Layer<engine::Route> + Clone + Send + Sync + 'static,
        L::Service: engine::Service<
                engine::Request,
                Response = http::Response<B>,
                Error = std::convert::Infallible,
            > + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as engine::Service<engine::Request>>::Future: Send + 'static,
        B: engine::HttpBody<Data = engine::Bytes> + Send + 'static,
        B::Error: Into<engine::body::BoxError>,
    {
        Self(self.0.route_layer(layer).expect("valid middleware"))
    }
    pub fn with_state<S2: Clone + Send + Sync + 'static>(self, state: S) -> MethodRouter<S2> {
        MethodRouter(self.0.with_state(state).expect("method state"))
    }
}
pub mod routing {
    use super::*;
    pub fn get<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).get(handler)
    }
    pub fn post<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).post(handler)
    }
    pub fn put<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).put(handler)
    }
    pub fn delete<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).delete(handler)
    }
    pub fn patch<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).patch(handler)
    }
    pub fn head<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).head(handler)
    }
    pub fn options<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::new().expect("methods")).options(handler)
    }
    pub fn any<S, H, T>(handler: H) -> MethodRouter<S>
    where
        S: Clone + Send + Sync + 'static,
        H: engine::Handler<T, S>,
        T: 'static,
    {
        MethodRouter(engine::MethodRouter::any_handler(handler).expect("any handler"))
    }
}
pub struct TcpListener(engine::TcpListener);
impl TcpListener {
    pub async fn bind(address: impl ToString) -> std::io::Result<Self> {
        bind(address).await
    }
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        Ok(self.0.local_addr())
    }
}
pub async fn bind(address: impl ToString) -> std::io::Result<TcpListener> {
    engine::bind(address.to_string()).await.map(TcpListener)
}
pub async fn serve(
    listener: TcpListener,
    router: Router,
    shutdown: simple_server::engine_lifecycle::Shutdown,
) -> std::io::Result<()> {
    engine::serve(listener.0, router.0, shutdown).await
}
pub use serve as serve_with_connect_info;
#[cfg(test)]
impl Router {
    pub async fn oneshot(
        &self,
        request: engine::Request,
    ) -> Result<engine::Response, std::io::Error> {
        {
            use tower::ServiceExt;
            Ok(self
                .0
                .clone()
                .into_service()?
                .oneshot(request)
                .await
                .unwrap())
        }
    }
}
#[cfg(test)]
pub struct TestServer {
    address: std::net::SocketAddr,
    stop: simple_server::engine_lifecycle::Shutdown,
    task: Option<simple_server::runtime::JoinHandle<std::io::Result<()>>>,
}
#[cfg(test)]
impl TestServer {
    pub async fn tcp(router: Router) -> std::io::Result<Self> {
        let listener = bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let stop = simple_server::engine_lifecycle::Shutdown::new();
        let task = simple_server::runtime::spawn(serve(listener, router, stop.clone()));
        Ok(Self {
            address,
            stop,
            task: Some(task),
        })
    }
    pub fn address(&self) -> Option<std::net::SocketAddr> {
        Some(self.address)
    }
    pub fn base_url(&self) -> Option<String> {
        Some(format!("http://{}", self.address))
    }
    pub async fn shutdown(mut self) -> std::io::Result<()> {
        self.stop.request();
        self.task
            .take()
            .unwrap()
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?
    }
}
#[cfg(test)]
impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.request();
    }
}
