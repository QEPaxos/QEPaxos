use rpc::epaxos_rpc::Msg;
use rpc::sepaxos_rpc::ClientMsg;
pub mod err;
pub mod execution;
pub mod peer;
pub mod peer_communication;

#[derive(Debug)]
pub enum PeerMsg {
    Msg(Msg),
    ClientMsg(ClientMsg),
}

// remove or add http:// prefix
pub fn convert_ip_addr(ip: String, add_http: bool) -> String {
    if add_http {
        let prefix = String::from("http://");
        prefix + ip.clone().as_str()
    } else {
        let len = ip.len();
        if len <= 8 {
            return String::from("");
        }
        let result = &ip[7..len];
        result.to_string()
    }
}

// fn conflict(gamma: &Kv, delta: &Kv) -> bool {
//     if gamma.key == delta.key {
//         return true;
//     }
//     false
// }

// fn conflict_commands(batch1: &Vec<Kv>, batch2: &Vec<Kv>) -> bool {
//     for gamma in batch1.iter() {
//         for delta in batch2.iter() {
//             if conflict(gamma, delta) {
//                 return true;
//             }
//         }
//     }
//     false
// }
