use std::{env, time::Duration};

use common::config::ServerConfig;
use rpc::sepaxos_rpc::{ClientMsg, ClientMsgReply};
use sepaxos::peer_communication::ProposeClient;
use time;
use tokio::{sync::mpsc::channel, time::Instant};
use tonic::Streaming;
use tracing;

pub static mut START_INSTANT: Vec<Instant> = Vec::new();

async fn wait_for_raft_reply(mut stream: Streaming<ClientMsgReply>, msg_count: usize) -> Vec<u128> {
    let mut result: Vec<u128> = vec![0; msg_count];
    unsafe {
        for _i in 0..msg_count {
            if let Some(reply) = stream.message().await.unwrap() {
                let command_id = reply.command_id;
                // println!("command id = {}", command_id);
                let time = START_INSTANT[command_id as usize].elapsed().as_micros();
                println!("time = {}", time);
                result[reply.command_id as usize] = time;
            }
        }
    }
    result
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let id = args[1].parse::<i32>().unwrap();
    let msg_count = args[2].parse::<usize>().unwrap();
    // let id = 0;
    tracing::info!("id = {}", id);
    let config = ServerConfig::new(5);
    let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();

    let client_msg = ClientMsg {
        key: "test_key".to_string(),
        value: "test_value".to_string(),
        command_id: 0,
    };
    let mut client = ProposeClient::new(propose_ip).await;
    let (send_to_server, server_receiver) = channel::<ClientMsg>(1);
    let mut result_stream = client.run_client(server_receiver).await;
    // sleep(Duration::from_secs(1)).await;
    let mut msg = client_msg.clone();
    msg.command_id = 0;
    send_to_server.send(msg).await;
    let start = Instant::now();
    if let Some(reply) = result_stream.message().await.unwrap() {
        let command_id = reply.command_id;
        let time = start.elapsed().as_micros();
        println!("time {}", time);
    }

    unsafe {
        START_INSTANT.reserve(msg_count);
    }
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

    let result = wait_for_raft_reply(result_stream, msg_count).await;
    let mut total_time = 0;
    for i in 0..msg_count {
        total_time += result[i];
        //println!("time {} {}", i, result[i]);
    }
    println!("average time = {}", total_time / msg_count as u128);

    Ok(())
}
