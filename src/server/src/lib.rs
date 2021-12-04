mod fail_test;
mod latency_client;
mod raft_test_client;
mod server;
mod throughput_client;

use rpc::sepaxos_rpc::ClientMsgReply;
use tonic::Streaming;

pub async fn wait_replies(
    result: &mut Streaming<ClientMsgReply>,
    msg_num: usize,
    rsp: &mut Vec<i32>,
) -> usize {
    let mut success: usize = 0;
    for _i in 0..msg_num {
        if let Some(msg) = result.message().await.unwrap() {
            // println!("receive a msg {:?}", msg);
            if msg.success {
                success += 1;
            }
        }
    }
    success
}
