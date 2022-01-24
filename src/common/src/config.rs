use std::{collections::HashMap, hash::Hash};

pub struct ServerConfig {
    pub peer_num: usize,
    pub fast_quorum_size: usize,
    pub paxos_quorum_size: usize,
    pub server_ids: Vec<i32>,
    pub server_addrs: HashMap<i32, String>,
    pub propose_server_addrs: HashMap<i32, String>,
    pub test_msg_count_per_server: usize,
    pub preferred_peer: Vec<Vec<i32>>,
    pub wide_private_server_addr: HashMap<i32, String>,
    pub wide_private_propose_addr: HashMap<i32, String>,
}

/// all server ids start from 0
impl ServerConfig {
    pub fn new(replicas: i32) -> Self {
        if replicas == 3 {
            let server_ids = vec![0, 1, 2];
            let addr0 = String::from("http://192.168.50.10:20011");
            let addr1 = String::from("http://192.168.50.11:20011");
            let addr2 = String::from("http://192.168.50.12:20011");
            let mut server_addrs = HashMap::new();
            server_addrs.insert(0, addr0);
            server_addrs.insert(1, addr1);
            server_addrs.insert(2, addr2);

            let propose_addr0 = String::from("http://192.168.50.10:20012");
            let propose_addr1 = String::from("http://192.168.50.11:20012");
            let propose_addr2 = String::from("http://192.168.50.12:20012");
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
                wide_private_propose_addr: HashMap::new(),
                wide_private_server_addr: HashMap::new(),
            }
        } else {
            let server_ids = vec![0, 1, 2, 3, 4];
            let addr0 = String::from("http://192.168.50.10:20011");
            let addr1 = String::from("http://192.168.50.11:20011");
            let addr2 = String::from("http://192.168.50.12:20011");
            let addr3 = String::from("http://192.168.50.13:20011");
            let addr4 = String::from("http://192.168.50.15:20011");
            let mut server_addrs = HashMap::new();
            server_addrs.insert(0, addr0);
            server_addrs.insert(1, addr1);
            server_addrs.insert(2, addr2);
            server_addrs.insert(3, addr3);
            server_addrs.insert(4, addr4);

            let propose_addr0 = String::from("http://192.168.50.10:20012");
            let propose_addr1 = String::from("http://192.168.50.11:20012");
            let propose_addr2 = String::from("http://192.168.50.12:20012");
            let propose_addr3 = String::from("http://192.168.50.13:20012");
            let propose_addr4 = String::from("http://192.168.50.15:20012");
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
                wide_private_propose_addr: HashMap::new(),
                wide_private_server_addr: HashMap::new(),
                test_msg_count_per_server: 100_0000,
                preferred_peer: vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![0, 4], vec![0, 1]],
            }
        }
    }

    pub fn new_wide_config() -> Self {
        let server_ids = vec![0, 1, 2, 3, 4];
        let addr0 = String::from("http://175.41.151.192:20011");
        let addr1 = String::from("http://18.183.177.239:20011");
        let addr2 = String::from("http://54.193.150.219:20011");
        let addr3 = String::from("http://13.40.51.62:20011");
        let addr4 = String::from("http://3.238.127.2:20011");
        let mut server_addrs = HashMap::new();
        server_addrs.insert(0, addr0);
        server_addrs.insert(1, addr1);
        server_addrs.insert(2, addr2);
        server_addrs.insert(3, addr3);
        server_addrs.insert(4, addr4);

        let propose_addr0 = String::from("http://175.41.151.192:20012");
        let propose_addr1 = String::from("http://18.183.177.239:20012");
        let propose_addr2 = String::from("http://54.193.150.219:20012");
        let propose_addr3 = String::from("http://13.40.51.62:20012");
        let propose_addr4 = String::from("http://3.238.127.2:20012");
        let mut propose_server_addrs = HashMap::new();
        propose_server_addrs.insert(0, propose_addr0);
        propose_server_addrs.insert(1, propose_addr1);
        propose_server_addrs.insert(2, propose_addr2);
        propose_server_addrs.insert(3, propose_addr3);
        propose_server_addrs.insert(4, propose_addr4);

        let private_addr0 = String::from("http://172.31.28.41:20011");
        let private_addr1 = String::from("http://172.31.41.100:20011");
        let private_addr2 = String::from("http://172.31.24.228:20011");
        let private_addr3 = String::from("http://172.31.6.111:20011");
        let private_addr4 = String::from("http://172.31.77.239:20011");
        let mut private_server_addrs = HashMap::new();
        private_server_addrs.insert(0, private_addr0);
        private_server_addrs.insert(1, private_addr1);
        private_server_addrs.insert(2, private_addr2);
        private_server_addrs.insert(3, private_addr3);
        private_server_addrs.insert(4, private_addr4);

        let private_propose_addr0 = String::from("http://172.31.28.41:20012");
        let private_propose_addr1 = String::from("http://172.31.41.100:20012");
        let private_propose_addr2 = String::from("http://172.31.24.228:20012");
        let private_propose_addr3 = String::from("http://172.31.6.111:20012");
        let private_propose_addr4 = String::from("http://172.31.77.239:20012");
        let mut private_propose_addrs = HashMap::new();
        private_propose_addrs.insert(0, private_propose_addr0);
        private_propose_addrs.insert(1, private_propose_addr1);
        private_propose_addrs.insert(2, private_propose_addr2);
        private_propose_addrs.insert(3, private_propose_addr3);
        private_propose_addrs.insert(4, private_propose_addr4);

        Self {
            peer_num: 5,
            fast_quorum_size: 3,
            paxos_quorum_size: 3,
            server_ids,
            server_addrs,
            propose_server_addrs,
            wide_private_propose_addr: private_propose_addrs,
            wide_private_server_addr: private_server_addrs,
            test_msg_count_per_server: 100_0000,
            preferred_peer: vec![vec![1, 2], vec![0, 2], vec![0, 1], vec![2, 4], vec![2, 3]],
        }
    }
}
