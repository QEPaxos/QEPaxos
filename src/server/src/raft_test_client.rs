use common::config::ServerConfig;
use qepaxos::peer_communication::ProposeClient;
use raft::peer::Peer;
use rpc::qepaxos_rpc::{ClientMsg, ClientMsgReply, GetLeaderRequest};
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

async fn latency_test(msg_count: usize, propose_ip: String, config: ServerConfig) {
    let mut client = ProposeClient::new(propose_ip).await;
    let (send_to_server, server_receiver) = channel::<ClientMsg>(1);
    let mut result_stream = client.run_client(server_receiver).await;
    // sleep(Duration::from_secs(1)).await;
    // println!("start to send msg");
    let msg_to_send = ClientMsg {
        key: "test_key".to_string(),
        value: "test_value".to_string(),
        command_id: 0,
    };
    let mut result: Vec<u128> = Vec::new();
    println!("try to send msg to leader");
    for i in 0..200 {
        let start = Instant::now();
        let mut to_send = msg_to_send.clone();
        to_send.command_id = i;
        send_to_server.send(to_send).await;
        if let Some(reply) = result_stream.message().await.unwrap() {
            let time = start.elapsed().as_micros();
            println!("time = {}", time);
            result.push(time);
        }
    }

    result.sort();
    // median
    println!("median = {}", result[99]);
    // p99
    let mut p99 = 0;
    for i in 190..200 {
        p99 += result[i];
    }
    println!("p99 = {}", p99 / 10);
}

async fn throughput_test(
    client_msg: ClientMsg,
    msg_count: i32,
    propose_ip: String,
    config: ServerConfig,
) {
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
    let wide_area = args[2].parse::<bool>().unwrap();
    let msg_count = args[3].parse::<i32>().unwrap();
    let id = 0;
    let config = if wide_area {
        ServerConfig::new_wide_config()
    } else {
        ServerConfig::new(5)
    };
    let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();

    println!("try to get leader id {}", propose_ip.clone());
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
    let leader_ip = config.propose_server_addrs.get(&leader_id).unwrap().clone();
    if test_type.eq_ignore_ascii_case("latency") {
        latency_test(200, leader_ip, config).await;
    } else {
        let client_msg = ClientMsg {
            key: "test_key".to_string(),
            value: String::from("0").repeat(16),
            command_id: 0,
        };
        throughput_test(client_msg, msg_count, leader_ip, config).await;
    }

    Ok(())
}
