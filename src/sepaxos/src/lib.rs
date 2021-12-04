use common::Instance;
use rpc::sepaxos_rpc::{ClientMsg, Msg};

pub mod err;
pub mod execution;
pub mod peer;
pub mod peer_communication;

#[derive(Debug)]
pub enum PeerMsg {
    Msg(Msg),
    ClientMsg(ClientMsg),
    TimeOut(Instance),
}
