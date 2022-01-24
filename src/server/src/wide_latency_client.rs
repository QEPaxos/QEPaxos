use std::{env, time::Duration};

use common::config::ServerConfig;
use common::KeyGenerator;
use common::CONFLICT_KEY;
use epaxos::peer_communication::ProposeClient as epaxos_client;
use qepaxos::peer_communication::ProposeClient as qepaxos_client;
use rpc::qepaxos_rpc::{ClientMsg, ClientMsgReply};
use time;
use tokio::{
    sync::mpsc::{channel, Sender},
    time::{sleep, Instant},
};
use tonic::Streaming;
use tracing;

pub static msg_count: usize = 1000;
// async fn wait_for_raft_reply(mut stream: Streaming<ClientMsgReply>, msg_count: usize) -> Vec<u128> {
//     let mut result: Vec<u128> = vec![0; msg_count];
//     unsafe {
//         for _i in 0..msg_count {
//             if let Some(reply) = stream.message().await.unwrap() {
//                 let command_id = reply.command_id;
//                 // println!("command id = {}", command_id);
//                 let time = START_INSTANT[command_id as usize].elapsed().as_micros();
//                 println!("time = {}", time);
//                 result[reply.command_id as usize] = time;
//             }
//         }
//     }
//     result
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let id_to_connect = args[1].parse::<i32>().unwrap();
    let replica_nums = args[2].parse::<i32>().unwrap();
    let conflict_rate = args[3].parse::<u32>().unwrap();
    let server_type = args[4].parse::<String>().unwrap();

    // let id = 0;
    println!("id = {}", id_to_connect);
    println!("replica_nums = {}", replica_nums);
    let config = ServerConfig::new_wide_config();
    let propose_ip = config
        .propose_server_addrs
        .get(&id_to_connect)
        .unwrap()
        .clone();

    let mut server_senders: Vec<Sender<ClientMsg>> = Vec::new();

    let (send_to_leader, leader_receiver) = channel::<ClientMsg>(msg_count);
    println!("connect to leader {}", propose_ip);

    if server_type.eq_ignore_ascii_case("qepaxos") {
        let mut leader_client = qepaxos_client::new(propose_ip).await;
        let mut result_stream: Streaming<ClientMsgReply> =
            leader_client.run_client(leader_receiver).await;
        let mut latency_sender: Sender<ClientMsg> = send_to_leader;

        // init clients
        println!("connect to other replica");
        for id in config.server_ids.iter() {
            let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();
            println!("connect to replica{}", propose_ip);
            let mut client = qepaxos_client::new(propose_ip).await;
            let (send_to_server, server_receiver) = channel::<ClientMsg>(msg_count);
            if *id == id_to_connect {
                // result_stream = client.run_client(server_receiver).await;
                // latency_sender = send_to_server;
                continue;
            } else {
                let result = client.run_client(server_receiver).await;
                server_senders.push(send_to_server);
            }

            //let mut result_stream = client.run_client(server_receiver).await;
            // sleep(Duration::from_secs(1)).await;
        }
        println!("connect to other replica done");
        let key_gen = KeyGenerator::new(conflict_rate, 16);
        let mut keys = key_gen.gen_request(100000, 100);

        tokio::spawn(async move {
            let mut i = 0;
            loop {
                let msg = ClientMsg {
                    key: keys[i].clone(),
                    value: key_gen.value.clone(),
                    command_id: i as i32,
                };
                for server_id in 0..server_senders.len() {
                    server_senders[server_id].send(msg.clone()).await;
                    // sender.send(msg).await;
                }
                sleep(Duration::from_millis(20)).await;
                i += 1;
            }
        });

        let msg_to_send = ClientMsg {
            key: CONFLICT_KEY.to_string(),
            value: String::from("test_value"),
            command_id: 1,
        };
        let mut result: Vec<u128> = Vec::new();
        println!("try to send msg to leader");
        sleep(Duration::from_millis(30)).await;
        for i in 0..200 {
            let start = Instant::now();
            let mut to_send = msg_to_send.clone();
            to_send.command_id = i;
            latency_sender.send(to_send).await;
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
    } else if server_type.eq_ignore_ascii_case("epaxos") {
        let mut leader_client = epaxos_client::new(propose_ip).await;
        let mut result_stream: Streaming<ClientMsgReply> =
            leader_client.run_client(leader_receiver).await;
        let mut latency_sender: Sender<ClientMsg> = send_to_leader;

        // init clients
        println!("connect to other replica");
        for id in config.server_ids.iter() {
            let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();
            println!("connect to replica{}", propose_ip);
            let mut client = epaxos_client::new(propose_ip).await;
            let (send_to_server, server_receiver) = channel::<ClientMsg>(msg_count);
            if *id == id_to_connect {
                // result_stream = client.run_client(server_receiver).await;
                // latency_sender = send_to_server;
                continue;
            } else {
                let result = client.run_client(server_receiver).await;
                server_senders.push(send_to_server);
            }

            //let mut result_stream = client.run_client(server_receiver).await;
            // sleep(Duration::from_secs(1)).await;
        }
        println!("connect to other replica done");
        let key_gen = KeyGenerator::new(conflict_rate, 16);
        let mut keys = key_gen.gen_request(100000, 100);

        tokio::spawn(async move {
            let mut i = 0;
            loop {
                let msg = ClientMsg {
                    key: keys[i].clone(),
                    value: key_gen.value.clone(),
                    command_id: i as i32,
                };
                for server_id in 0..server_senders.len() {
                    server_senders[server_id].send(msg.clone()).await;
                    // sender.send(msg).await;
                }
                sleep(Duration::from_millis(20)).await;
                i += 1;
            }
        });

        let msg_to_send = ClientMsg {
            key: CONFLICT_KEY.to_string(),
            value: String::from("test_value"),
            command_id: 1,
        };
        let mut result: Vec<u128> = Vec::new();
        println!("try to send msg to leader");
        sleep(Duration::from_millis(30)).await;
        for i in 0..200 {
            let start = Instant::now();
            let mut to_send = msg_to_send.clone();
            to_send.command_id = i;
            latency_sender.send(to_send).await;
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

    Ok(())
}
