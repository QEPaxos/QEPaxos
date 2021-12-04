#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TryPreAcceptReplyOpt {
    #[prost(int32, tag = "1")]
    pub conflict_instance: i32,
    #[prost(int32, tag = "2")]
    pub conflict_replica: i32,
    #[prost(enumeration = "LogStatus", tag = "3")]
    pub conflict_status: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Msg {
    #[prost(enumeration = "MsgType", tag = "1")]
    pub entry_type: i32,
    #[prost(int32, tag = "2")]
    pub from: i32,
    #[prost(int32, tag = "3")]
    pub replica: i32,
    #[prost(int32, tag = "4")]
    pub instance: i32,
    #[prost(int32, tag = "5")]
    pub ballot: i32,
    #[prost(int32, repeated, tag = "6")]
    pub deps: ::prost::alloc::vec::Vec<i32>,
    #[prost(message, repeated, tag = "7")]
    pub commands: ::prost::alloc::vec::Vec<super::sepaxos_rpc::ClientMsg>,
    #[prost(enumeration = "LogStatus", optional, tag = "8")]
    pub status: ::core::option::Option<i32>,
    #[prost(bool, optional, tag = "9")]
    pub ok: ::core::option::Option<bool>,
    #[prost(message, optional, tag = "10")]
    pub try_preacept_reply: ::core::option::Option<TryPreAcceptReplyOpt>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Reply {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLeaderRequest {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLeaderReply {
    #[prost(int32, tag = "1")]
    pub leader: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum LogStatus {
    Preparing = 0,
    PreAccepted = 1,
    PreAcceptedEq = 2,
    Accepted = 3,
    Commited = 4,
    Executed = 5,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MsgType {
    PreAccept = 0,
    PreAcceptReply = 1,
    Accept = 2,
    AcceptReply = 3,
    Commit = 4,
    Prepare = 5,
    PrepareReply = 6,
    TyrPreaccept = 7,
    TryRepacceptReply = 8,
}
#[doc = r" Generated client implementations."]
pub mod communication_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct CommunicationClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl CommunicationClient<tonic::transport::Channel> {
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
    impl<T> CommunicationClient<T>
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
        pub async fn communication(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::Msg>,
        ) -> Result<tonic::Response<super::Reply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/epaxos_rpc.Communication/Communication");
            self.inner
                .client_streaming(request.into_streaming_request(), path, codec)
                .await
        }
    }
    impl<T: Clone> Clone for CommunicationClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for CommunicationClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "CommunicationClient {{ ... }}")
        }
    }
}
#[doc = r" Generated client implementations."]
pub mod client_service_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct ClientServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ClientServiceClient<tonic::transport::Channel> {
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
    impl<T> ClientServiceClient<T>
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
        pub async fn propose(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::super::sepaxos_rpc::ClientMsg>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::super::sepaxos_rpc::ClientMsgReply>>,
            tonic::Status,
        > {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/epaxos_rpc.ClientService/Propose");
            self.inner
                .streaming(request.into_streaming_request(), path, codec)
                .await
        }
        pub async fn get_leader(
            &mut self,
            request: impl tonic::IntoRequest<super::GetLeaderRequest>,
        ) -> Result<tonic::Response<super::GetLeaderReply>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/epaxos_rpc.ClientService/GetLeader");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for ClientServiceClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for ClientServiceClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "ClientServiceClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod communication_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with CommunicationServer."]
    #[async_trait]
    pub trait Communication: Send + Sync + 'static {
        async fn communication(
            &self,
            request: tonic::Request<tonic::Streaming<super::Msg>>,
        ) -> Result<tonic::Response<super::Reply>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct CommunicationServer<T: Communication> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: Communication> CommunicationServer<T> {
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
    impl<T, B> Service<http::Request<B>> for CommunicationServer<T>
    where
        T: Communication,
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
                "/epaxos_rpc.Communication/Communication" => {
                    #[allow(non_camel_case_types)]
                    struct CommunicationSvc<T: Communication>(pub Arc<T>);
                    impl<T: Communication> tonic::server::ClientStreamingService<super::Msg> for CommunicationSvc<T> {
                        type Response = super::Reply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::Msg>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).communication(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = CommunicationSvc(inner);
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
    impl<T: Communication> Clone for CommunicationServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: Communication> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Communication> tonic::transport::NamedService for CommunicationServer<T> {
        const NAME: &'static str = "epaxos_rpc.Communication";
    }
}
#[doc = r" Generated server implementations."]
pub mod client_service_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with ClientServiceServer."]
    #[async_trait]
    pub trait ClientService: Send + Sync + 'static {
        #[doc = "Server streaming response type for the Propose method."]
        type ProposeStream: futures_core::Stream<
                Item = Result<super::super::sepaxos_rpc::ClientMsgReply, tonic::Status>,
            > + Send
            + Sync
            + 'static;
        async fn propose(
            &self,
            request: tonic::Request<tonic::Streaming<super::super::sepaxos_rpc::ClientMsg>>,
        ) -> Result<tonic::Response<Self::ProposeStream>, tonic::Status>;
        async fn get_leader(
            &self,
            request: tonic::Request<super::GetLeaderRequest>,
        ) -> Result<tonic::Response<super::GetLeaderReply>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct ClientServiceServer<T: ClientService> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: ClientService> ClientServiceServer<T> {
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
    impl<T, B> Service<http::Request<B>> for ClientServiceServer<T>
    where
        T: ClientService,
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
                "/epaxos_rpc.ClientService/Propose" => {
                    #[allow(non_camel_case_types)]
                    struct ProposeSvc<T: ClientService>(pub Arc<T>);
                    impl<T: ClientService>
                        tonic::server::StreamingService<super::super::sepaxos_rpc::ClientMsg>
                        for ProposeSvc<T>
                    {
                        type Response = super::super::sepaxos_rpc::ClientMsgReply;
                        type ResponseStream = T::ProposeStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<
                                tonic::Streaming<super::super::sepaxos_rpc::ClientMsg>,
                            >,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).propose(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = ProposeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/epaxos_rpc.ClientService/GetLeader" => {
                    #[allow(non_camel_case_types)]
                    struct GetLeaderSvc<T: ClientService>(pub Arc<T>);
                    impl<T: ClientService> tonic::server::UnaryService<super::GetLeaderRequest> for GetLeaderSvc<T> {
                        type Response = super::GetLeaderReply;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetLeaderRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_leader(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = GetLeaderSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
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
    impl<T: ClientService> Clone for ClientServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: ClientService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: ClientService> tonic::transport::NamedService for ClientServiceServer<T> {
        const NAME: &'static str = "epaxos_rpc.ClientService";
    }
}
