use common::config::ServerConfig;
use raft::peer::Peer;
use rpc::sepaxos_rpc::{ClientMsg, ClientMsgReply, GetLeaderRequest};
use sepaxos::peer_communication::ProposeClient;
use std::{env, time::Duration};
use tokio::{
    sync::mpsc::channel,
    time::{sleep, Instant},
};
use tonic::Streaming;
use tracing;

pub static mut START_INSTANT: Vec<Instant> = Vec::new();

async fn wait_for_raft_reply(mut stream: Streaming<ClientMsgReply>, msg_count: usize) -> Vec<u128> {
    let mut result: Vec<u128> = vec![0; msg_count];
    unsafe {
        for _i in 0..msg_count {
            if let Some(reply) = stream.message().await.unwrap() {
                let command_id = reply.command_id;
                let time = START_INSTANT[command_id as usize].elapsed().as_micros();

                result[reply.command_id as usize] = time;
            }
        }
    }
    result
}

async fn latency_test(msg_count: usize, leader_id: i32, config: ServerConfig) {
    let propose_ip = config.propose_server_addrs.get(&leader_id).unwrap().clone();

    let client_msg = ClientMsg {
        key: "test_key".to_string(),
        value: "test_value".to_string(),
        command_id: 0,
    };
    let mut client = ProposeClient::new(propose_ip).await;
    let (send_to_server, server_receiver) = channel::<ClientMsg>(1);
    let result_stream = client.run_client(server_receiver).await;
    // sleep(Duration::from_secs(1)).await;
    println!("start to send msg");
    tokio::spawn(async move {
        unsafe {
            for i in 0..msg_count {
                let mut msg = client_msg.clone();
                msg.command_id = i as i32;
                send_to_server.send(msg).await;
                START_INSTANT.push(Instant::now());
            }
        }
    });
    println!("wait for reply");
    let result = wait_for_raft_reply(result_stream, msg_count).await;
    let mut total_time = 0;
    for i in 0..msg_count {
        total_time += result[i];
    }
    println!("average time = {}", total_time / msg_count as u128);
}

async fn throughput_test(
    client_msg: ClientMsg,
    msg_count: i32,
    leader_id: i32,
    config: ServerConfig,
) {
    let propose_ip = config.propose_server_addrs.get(&leader_id).unwrap().clone();

    let mut client = ProposeClient::new(propose_ip).await;
    let (send_to_server, server_receiver) = channel::<ClientMsg>(10000);
    let mut result = client.run_client(server_receiver).await;
    // sleep(Duration::from_secs(1)).await;

    // send msgs
    let start = Instant::now();
    for i in 0..msg_count {
        let mut msg = client_msg.clone();
        msg.command_id = i;
        send_to_server.send(msg).await;
    }
    // wait for reply
    for i in 0..msg_count {
        if let Some(reply) = result.message().await.unwrap() {
            // println!("receive a msg {:?}", reply);
        }
    }
    let total_time = start.elapsed().as_millis();

    println!("throughput = {}", (msg_count as u128) * 1000 / total_time);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    // let id = args[1].parse::<i32>().unwrap();
    let test_type = args[1].parse::<String>().unwrap();
    let id = 0;
    let config = ServerConfig::new(5);
    let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();

    let mut client = ProposeClient::new(propose_ip).await;
    let mut leader_id = 0;
    loop {
        leader_id = client
            .client
            .get_leader(GetLeaderRequest::default())
            .await
            .unwrap()
            .into_inner()
            .leader;
        if leader_id >= 0 {
            break;
        }
        tracing::info!("leader id = {}", leader_id);
        sleep(Duration::from_millis(300)).await;
    }
    println!("leader is {}", leader_id);
    if test_type.eq_ignore_ascii_case("latency") {
        latency_test(10000, leader_id, config).await;
    } else {
        let client_msg = ClientMsg {
            key: "test_key".to_string(),
            value: String::from("0").repeat(16),
            command_id: 0,
        };
        throughput_test(client_msg, 100000, leader_id, config).await;
    }

    Ok(())
}
