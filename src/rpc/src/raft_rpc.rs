#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Entry {
    #[prost(int32, tag = "1")]
    pub term: i32,
    #[prost(int32, tag = "2")]
    pub index: i32,
    #[prost(message, optional, tag = "3")]
    pub msg: ::core::option::Option<super::sepaxos_rpc::ClientMsg>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RaftMsg {
    #[prost(enumeration = "RaftMsgType", tag = "1")]
    pub entry_type: i32,
    #[prost(int32, tag = "3")]
    pub replica: i32,
    #[prost(int32, tag = "4")]
    pub prev_log_index: i32,
    #[prost(int32, tag = "5")]
    pub prev_log_term: i32,
    #[prost(int32, tag = "6")]
    pub term: i32,
    #[prost(message, repeated, tag = "7")]
    pub entries: ::prost::alloc::vec::Vec<Entry>,
    #[prost(int32, tag = "8")]
    pub leader_commit: i32,
    #[prost(bool, tag = "9")]
    pub success: bool,
    #[prost(int32, tag = "10")]
    pub entry_len: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Reply {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum State {
    Follower = 0,
    //// The node is campaigning to become the cluster leader.
    Candinate = 1,
    //// The node is the Raft cluster leader.
    Leader = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RaftMsgType {
    Append = 0,
    AppendResponse = 1,
    RequestVote = 2,
    RequestVoteResponse = 3,
}
#[doc = r" Generated client implementations."]
pub mod raft_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct RaftClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RaftClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> RaftClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn raft(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::RaftMsg>,
        ) -> Result<tonic::Response<super::Reply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/raft_rpc.Raft/Raft");
            self.inner
                .client_streaming(request.into_streaming_request(), path, codec)
                .await
        }
    }
    impl<T: Clone> Clone for RaftClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for RaftClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "RaftClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod raft_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with RaftServer."]
    #[async_trait]
    pub trait Raft: Send + Sync + 'static {
        async fn raft(
            &self,
            request: tonic::Request<tonic::Streaming<super::RaftMsg>>,
        ) -> Result<tonic::Response<super::Reply>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct RaftServer<T: Raft> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: Raft> RaftServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for RaftServer<T>
    where
        T: Raft,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/raft_rpc.Raft/Raft" => {
                    #[allow(non_camel_case_types)]
                    struct RaftSvc<T: Raft>(pub Arc<T>);
                    impl<T: Raft> tonic::server::ClientStreamingService<super::RaftMsg> for RaftSvc<T> {
                        type Response = super::Reply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::RaftMsg>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).raft(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = RaftSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.client_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: Raft> Clone for RaftServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: Raft> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Raft> tonic::transport::NamedService for RaftServer<T> {
        const NAME: &'static str = "raft_rpc.Raft";
    }
}
