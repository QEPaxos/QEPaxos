use rpc::{qepaxos_rpc::ClientMsg, raft_rpc::RaftMsg};
use tokio::sync::mpsc::Sender;

pub mod peer;
pub mod peer_communication;

pub enum PeerMsg {
    ClientMsg(ClientMsg),
    Peer(RaftMsg),
    GetLeader(Sender<i32>),
    TimeOut(),
}
