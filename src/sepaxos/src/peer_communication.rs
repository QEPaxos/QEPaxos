use std::pin::Pin;
use std::sync::Arc;

use crate::PeerMsg;

use super::err::Result as SE_Result;
use rpc::sepaxos_rpc::{
    client_service_client::ClientServiceClient,
    client_service_server::{ClientService, ClientServiceServer},
    communication_client::CommunicationClient,
    communication_server::{Communication, CommunicationServer},
    ClientMsg, ClientMsgReply, Msg, Reply,
};
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status, Streaming};
use tracing::info;

pub struct RpcClient {
    addr_to_connect: String,
    client: CommunicationClient<Channel>,
}

impl RpcClient {
    pub async fn new(addr: String) -> Self {
        loop {
            match CommunicationClient::connect(addr.clone()).await {
                Ok(client) => {
                    return Self {
                        addr_to_connect: addr,
                        client,
                    };
                }
                Err(_) => {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    pub async fn run_client(&mut self, receiver: Receiver<Msg>) {
        // let mut re = receiver;
        // let stream = async_stream::stream! {
        //     while let Some(item) = re.recv().await {
        //         yield item;
        //     }
        // };
        let receiver = ReceiverStream::new(receiver);

        let _response = self.client.communication(receiver).await;
    }
}

pub struct RpcServer {
    addr_to_listen: String,
    sender: UnboundedSender<PeerMsg>,
}

impl RpcServer {
    pub fn new(addr_to_listen: String, sender: UnboundedSender<PeerMsg>) -> Self {
        Self {
            addr_to_listen,
            sender,
        }
    }
}

pub async fn run_server(rpc_server: RpcServer) {
    tracing::info!("run server with ip {}", rpc_server.addr_to_listen);
    let addr = rpc_server.addr_to_listen.parse().unwrap();

    tracing::info!("PeerServer listening on: {:?}", addr);

    let server = CommunicationServer::new(rpc_server);

    match Server::builder().add_service(server).serve(addr).await {
        Ok(_) => tracing::info!("rpc server start done"),
        Err(e) => panic!("rpc server start fail {}", e),
    }
}

#[tonic::async_trait]
impl Communication for RpcServer {
    async fn communication(
        &self,
        request: Request<Streaming<Msg>>,
    ) -> Result<Response<Reply>, Status> {
        let mut stream = request.into_inner();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            while let Some(peer_request) = stream.next().await {
                // info!("server receive a msg {:?}", peer_request.clone().unwrap().msg.unwrap());
                match peer_request {
                    Ok(msg) => {
                        sender.send(PeerMsg::Msg(msg));
                    }
                    Err(_) => {
                        //todo handle network err
                    }
                };
            }
        });
        let reply = Reply {};
        Ok(Response::new(reply))
    }
}

pub struct ProposeClient {
    pub client: ClientServiceClient<Channel>,
}

impl ProposeClient {
    pub async fn new(addr: String) -> Self {
        loop {
            match ClientServiceClient::connect(addr.clone()).await {
                Ok(client) => {
                    return Self { client };
                }
                Err(_) => {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    pub async fn run_client(&mut self, receiver: Receiver<ClientMsg>) -> Streaming<ClientMsgReply> {
        let receiver = ReceiverStream::new(receiver);

        self.client.propose(receiver).await.unwrap().into_inner()
    }
}

pub struct ProposeServer {
    addr_to_listen: String,
    sender: UnboundedSender<PeerMsg>,
    receiver: Arc<Mutex<UnboundedReceiver<ClientMsgReply>>>,
}

impl ProposeServer {
    pub fn new(
        addr_to_listen: String,
        sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<ClientMsgReply>,
    ) -> Self {
        Self {
            addr_to_listen,
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }
}

pub async fn run_propose_server(propose_server: ProposeServer) {
    let addr = propose_server.addr_to_listen.parse().unwrap();

    tracing::info!("ProposeServer listening on: {:?}", addr);

    let server = ClientServiceServer::new(propose_server);

    match Server::builder().add_service(server).serve(addr).await {
        Ok(_) => println!("propose rpc server start done"),
        Err(e) => panic!("propose rpc server start fail {}", e),
    }
}

#[tonic::async_trait]
impl ClientService for ProposeServer {
    type ProposeStream =
        Pin<Box<dyn Stream<Item = Result<ClientMsgReply, Status>> + Send + Sync + 'static>>;

    async fn propose(
        &self,
        request: tonic::Request<tonic::Streaming<ClientMsg>>,
    ) -> Result<tonic::Response<Self::ProposeStream>, tonic::Status> {
        let mut stream = request.into_inner();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            while let Some(peer_request) = stream.next().await {
                // info!("server receive a msg {:?}", peer_request.clone().unwrap());
                match peer_request {
                    Ok(msg) => {
                        sender.send(PeerMsg::ClientMsg(msg));
                    }
                    Err(_) => {
                        //todo handle network err
                    }
                };
            }
        });
        // reply to client
        let arc_receiver = self.receiver.clone();
        let output = async_stream::try_stream! {
            let mut receiver = arc_receiver.lock().await;
            while let Some(reply) = receiver.recv().await{
                yield reply;
            }
        };
        Ok(Response::new(Box::pin(output) as Self::ProposeStream))
    }

    async fn get_leader(
        &self,
        request: tonic::Request<rpc::sepaxos_rpc::GetLeaderRequest>,
    ) -> Result<tonic::Response<rpc::sepaxos_rpc::GetLeaderReply>, tonic::Status> {
        todo!()
    }
}
