use rpc::{raft_rpc::RaftMsg, sepaxos_rpc::ClientMsg};
use tokio::sync::mpsc::Sender;

pub mod peer;
pub mod peer_communication;

pub enum PeerMsg {
    ClientMsg(ClientMsg),
    Peer(RaftMsg),
    GetLeader(Sender<i32>),
    TimeOut(),
}
