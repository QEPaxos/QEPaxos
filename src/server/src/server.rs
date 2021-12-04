use std::{env, time::Duration};

use common::config::ServerConfig;
use env_logger::Env;
use epaxos::peer as EPaxosPeer;
use epaxos::PeerMsg as EPaxosMsg;
use raft::peer as RaftPeer;
use raft::PeerMsg as RaftMsg;
use rpc::sepaxos_rpc::{ClientMsg, ClientMsgReply};
use sepaxos::peer::Peer as SEPaxosPeer;
use sepaxos::PeerMsg as SEPaxosMsg;
use tokio::{
    sync::mpsc::{channel, unbounded_channel},
    time::sleep,
};
use tracing;

fn init_env_log() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .is_test(true)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_env_log();
    let args: Vec<String> = env::args().collect();
    let id = args[1].parse::<i32>().unwrap();
    let server_type = args[2].parse::<String>().unwrap();
    let batch_size = args[3].parse::<usize>().unwrap();
    tracing::info!("id = {}, server type = {}", id, server_type);
    let config = ServerConfig::new(3);
    let (sender, receiver) = unbounded_channel::<ClientMsgReply>();
    if server_type.eq_ignore_ascii_case("sepaxos") {
        tracing::info!("starting sepaxos");
        let (sender_to_peer, peer_receiver) = unbounded_channel::<SEPaxosMsg>();
        let mut peer = SEPaxosPeer::new(
            id,
            config,
            sender_to_peer.clone(),
            peer_receiver,
            sender,
            true,
            batch_size,
        );

        peer.init_and_run(receiver).await;
    } else if server_type.eq_ignore_ascii_case("epaxos") {
        tracing::info!("starting epaxos");
        let (sender_to_peer, peer_receiver) = unbounded_channel::<EPaxosMsg>();
        let mut peer = EPaxosPeer::Peer::new(
            id,
            config,
            sender_to_peer.clone(),
            peer_receiver,
            sender,
            true,
            batch_size,
        );

        peer.init_and_run(receiver).await;
    } else if server_type.eq_ignore_ascii_case("raft") {
        tracing::info!("starting raft");
        let (sender_to_peer, peer_receiver) = unbounded_channel::<RaftMsg>();
        let mut peer = RaftPeer::Peer::new(
            id,
            config,
            sender_to_peer.clone(),
            peer_receiver,
            sender,
            true,
            1,
        );

        peer.init_and_run(receiver).await;
    } else {
        tracing::info!("wrong server type");
    }

    Ok(())
}
