use common::{config::ServerConfig, KeyGenerator};
use raft::{peer::Peer, PeerMsg};
use rpc::sepaxos_rpc::{ClientMsg, ClientMsgReply};
use sepaxos::peer_communication::ProposeClient;
use std::{env, time::Duration};
use tokio::{
    sync::mpsc::{channel, unbounded_channel, Sender},
    time::{interval, sleep, Instant},
};
use tonic::Streaming;
use tracing;

pub static mut RECV: Vec<i32> = Vec::new();

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let msg_count = args[1].parse::<usize>().unwrap();
    let conflict = args[2].parse::<u32>().unwrap();
    let replicas = args[3].parse::<i32>().unwrap();
    println!("msg count = {}", msg_count);
    let config = ServerConfig::new(replicas);

    let mut server_senders: Vec<Sender<ClientMsg>> = Vec::new();
    let mut result_stream: Vec<Streaming<ClientMsgReply>> = Vec::new();
    unsafe {
        RECV = vec![0; replicas as usize];
    }
    // init clients
    for id in config.server_ids.iter() {
        // let id = 0;
        let propose_ip = config.propose_server_addrs.get(&id).unwrap().clone();

        let mut client = ProposeClient::new(propose_ip).await;
        let (send_to_server, server_receiver) = channel::<ClientMsg>(msg_count);
        result_stream.push(client.run_client(server_receiver).await);
        server_senders.push(send_to_server);

        //let mut result_stream = client.run_client(server_receiver).await;
        sleep(Duration::from_secs(1)).await;
    }

    let (peer_done_notify, mut notify_receiver) = channel::<bool>(10);

    let key_gen = KeyGenerator::new(conflict, 16);
    let mut keys = Vec::new();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            unsafe {
                println!("RECV {:?}", RECV);
            }
        }
    });
    for i in 0..config.peer_num {
        keys.push(key_gen.gen_request(msg_count, i));
        let tmp_sender = peer_done_notify.clone();
        let mut stream = result_stream.pop().unwrap();
        tokio::spawn(async move {
            let start = Instant::now();
            for _j in 0..msg_count {
                if let Some(reply) = stream.message().await.unwrap() {
                    // let command_id = reply.command_id;
                    // println!("recv id = {}", command_id);
                    unsafe {
                        RECV[i] += 1;
                    }
                }
            }
            println!("receive time = {}", start.elapsed().as_millis());
            // println!("receive done");
            tmp_sender.send(true).await;
        });
    }
    // send msgs
    let start = Instant::now();
    for i in 0..msg_count {
        for server_id in 0..server_senders.len() {
            let msg = ClientMsg {
                key: keys[server_id][i].clone(),
                value: key_gen.value.clone(),
                command_id: i as i32,
            };
            server_senders[server_id].send(msg).await;
            // sender.send(msg).await;
        }
    }

    // wait for reply
    let replica_num = config.peer_num;
    for _i in 0..replica_num {
        notify_receiver.recv().await;
    }
    let total_time = start.elapsed().as_millis();
    println!("average time = {}", total_time);
    println!(
        "throughput = {}/s",
        (replicas as u128) * (msg_count as u128) * 1000 / total_time
    );

    Ok(())
}
