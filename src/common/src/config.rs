use std::collections::HashMap;

pub struct ServerConfig {
    pub peer_num: usize,
    pub fast_quorum_size: usize,
    pub paxos_quorum_size: usize,
    pub server_ids: Vec<i32>,
    pub server_addrs: HashMap<i32, String>,
    pub propose_server_addrs: HashMap<i32, String>,
    pub test_msg_count_per_server: usize,
    pub preferred_peer: Vec<Vec<i32>>,
}

/// all server ids start from 0
impl ServerConfig {
    pub fn new(replicas: i32) -> Self {
        if replicas == 3 {
            let server_ids = vec![0, 1, 2];
            let addr0 = String::from("http://192.168.50.10:20011");
            let addr1 = String::from("http://192.168.50.11:20021");
            let addr2 = String::from("http://192.168.50.14:20031");
            let mut server_addrs = HashMap::new();
            server_addrs.insert(0, addr0);
            server_addrs.insert(1, addr1);
            server_addrs.insert(2, addr2);

            let propose_addr0 = String::from("http://192.168.50.10:20012");
            let propose_addr1 = String::from("http://192.168.50.11:20022");
            let propose_addr2 = String::from("http://192.168.50.14:20032");
            let mut propose_server_addrs = HashMap::new();
            propose_server_addrs.insert(0, propose_addr0);
            propose_server_addrs.insert(1, propose_addr1);
            propose_server_addrs.insert(2, propose_addr2);
            Self {
                peer_num: 3,
                fast_quorum_size: 2,
                paxos_quorum_size: 2,
                server_ids,
                server_addrs,
                propose_server_addrs,
                test_msg_count_per_server: 100_0000,
                preferred_peer: vec![vec![0, 1], vec![0, 1], vec![0, 1]],
            }
        } else {
            let server_ids = vec![0, 1, 2, 3, 4];
            let addr0 = String::from("http://192.168.50.10:20011");
            let addr1 = String::from("http://192.168.50.11:20021");
            let addr2 = String::from("http://192.168.50.12:20031");
            let addr3 = String::from("http://192.168.50.13:20041");
            let addr4 = String::from("http://192.168.50.14:20051");
            let mut server_addrs = HashMap::new();
            server_addrs.insert(0, addr0);
            server_addrs.insert(1, addr1);
            server_addrs.insert(2, addr2);
            server_addrs.insert(3, addr3);
            server_addrs.insert(4, addr4);

            let propose_addr0 = String::from("http://192.168.50.10:20012");
            let propose_addr1 = String::from("http://192.168.50.11:20022");
            let propose_addr2 = String::from("http://192.168.50.12:20032");
            let propose_addr3 = String::from("http://192.168.50.13:20042");
            let propose_addr4 = String::from("http://192.168.50.14:20052");
            let mut propose_server_addrs = HashMap::new();
            propose_server_addrs.insert(0, propose_addr0);
            propose_server_addrs.insert(1, propose_addr1);
            propose_server_addrs.insert(2, propose_addr2);
            propose_server_addrs.insert(3, propose_addr3);
            propose_server_addrs.insert(4, propose_addr4);
            Self {
                peer_num: 5,
                fast_quorum_size: 3,
                paxos_quorum_size: 3,
                server_ids,
                server_addrs,
                propose_server_addrs,
                test_msg_count_per_server: 100_0000,
                preferred_peer: vec![
                    vec![0, 1, 2],
                    vec![0, 1, 2],
                    vec![0, 1, 2],
                    vec![0, 1, 2],
                    vec![0, 1, 2],
                ],
            }
        }
    }
}
