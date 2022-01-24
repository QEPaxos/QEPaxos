use std::{
    cmp::min,
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use crate::{
    peer_communication::{run_propose_server, run_server, ProposeServer, RpcClient, RpcServer},
    PeerMsg,
};
use common::{config::ServerConfig, convert_ip_addr};
use futures::future::{AbortHandle, Abortable};
use log::info;
use rpc::{
    raft_rpc::{raft_server::Raft, Entry, RaftMsg, RaftMsgType, State},
    sepaxos_rpc::{ClientMsg, ClientMsgReply, MsgType},
};
use tokio::{
    sync::{
        mpsc::{channel, unbounded_channel, Sender, UnboundedReceiver, UnboundedSender},
        Notify,
    },
    time::{interval, timeout},
};

fn gen_election_timeout(min: i32, max: i32) -> i32 {
    let ran: i32 = rand::random::<i32>() % (max - min);
    ran + min
}

// pub static mut self.entries: BTreeMap<i32, Entry> = BTreeMap::new();

pub struct Peer {
    id: i32,
    wide_area: bool,
    config: ServerConfig,
    batch_size: i32,
    log_index: i32,
    last_applied: i32,
    commit_index: i32,
    current_term: i32,
    //current_response_count: i32,
    current_leader: i32,
    state: State,
    vote_for: i32,
    total_votes: usize,
    last_log_index: i32,
    match_index: Vec<i32>,
    next_index: Vec<i32>,
    set_timer: Arc<Notify>,
    timer_handle: AbortHandle,
    appending: bool,
    entries: BTreeMap<i32, Entry>,
    // peer infos
    peer_num: usize,
    quorum_size: usize,
    peers: HashMap<i32, Sender<RaftMsg>>,
    //
    sender: UnboundedSender<PeerMsg>,
    receiver: UnboundedReceiver<PeerMsg>,
    // client reply
    client_reply: UnboundedSender<ClientMsgReply>,
}

impl Peer {
    pub fn new(
        id: i32,
        config: ServerConfig,
        sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<PeerMsg>,
        client_reply: UnboundedSender<ClientMsgReply>,
        exec: bool,
        batch: usize,
        wide_area: bool,
    ) -> Self {
        let peer_num = config.peer_num;
        let quorum_size = config.paxos_quorum_size;
        let (timeout_handle, timeout_registration) = AbortHandle::new_pair();
        let timeout_sender = sender.clone();
        let elec_gen_election_timeout = gen_election_timeout(1000, 2000) + 1000;
        tracing::info!("elec timeout = {}", elec_gen_election_timeout);
        // run timeout task
        let set_timer = Arc::new(Notify::new());
        let notified = set_timer.clone();
        tokio::spawn(Abortable::new(
            async move {
                loop {
                    if let Err(_) = timeout(
                        Duration::from_millis(elec_gen_election_timeout as u64),
                        notified.notified(),
                    )
                    .await
                    {
                        // println!("time out");
                        timeout_sender.send(PeerMsg::TimeOut());
                    }
                    continue;
                }
            },
            timeout_registration,
        ));
        Self {
            id,
            wide_area,
            config,
            batch_size: batch as i32,
            log_index: 0,
            last_applied: 0,
            commit_index: 0,
            current_term: 0,
            current_leader: -1,
            state: State::Follower,
            vote_for: -1,
            total_votes: 0,
            last_log_index: 0,
            appending: false,
            entries: BTreeMap::new(),
            match_index: vec![0; peer_num],
            next_index: vec![0; peer_num],
            set_timer,
            timer_handle: timeout_handle,
            peer_num,
            quorum_size,
            peers: HashMap::new(),
            sender,
            receiver,
            client_reply,
        }
    }

    pub async fn init_and_run(&mut self, receiver: UnboundedReceiver<ClientMsgReply>) {
        // init rpc
        // let (client_sender, mut client_receiver) = unbounded_channel::<PeerMsg>();
        self.init_peer_rpc(self.sender.clone(), receiver).await;

        self.run().await;
    }

    pub async fn run(&mut self) {
        loop {
            match self.receiver.recv().await.unwrap() {
                PeerMsg::ClientMsg(client_msg) => self.handle_client(client_msg).await,
                PeerMsg::Peer(raft_msg) => self.handle_raft_msgs(raft_msg).await,
                PeerMsg::TimeOut() => self.handle_timeout().await,
                PeerMsg::GetLeader(sender) => {
                    tracing::info!("process get leader {}", self.current_leader);
                    sender.send(self.current_leader).await;
                }
            }
        }
    }

    async fn handle_raft_msgs(&mut self, msg: RaftMsg) {
        match msg.entry_type() {
            RaftMsgType::Append => self.handle_append_msg(msg).await,
            RaftMsgType::AppendResponse => self.handle_append_reply(msg).await,
            RaftMsgType::RequestVote => self.handle_request_vote(msg).await,
            RaftMsgType::RequestVoteResponse => self.handle_request_vote_response(msg),
        }
    }

    async fn handle_timeout(&mut self) {
        if self.state != State::Leader {
            self.convert_to_candinate();
            self.request_vote().await;
        } else {
            // heartbeat
            let heartbeat = RaftMsg {
                entry_type: RaftMsgType::Append.into(),
                replica: self.id,
                prev_log_index: -1,
                prev_log_term: 0,
                term: self.current_term,
                entries: Vec::new(),
                leader_commit: self.commit_index,
                success: false,
                entry_len: 0,
            };
            self.broadcast_to_peers(heartbeat).await;
        }
    }

    async fn handle_client(&mut self, msg: ClientMsg) {
        // tracing::info!("handle client msg");
        if self.state != State::Leader {
            return;
        }
        let index = self.log_index + 1;
        self.log_index = index;
        // new entry
        let entry = Entry {
            term: self.current_term,
            index,
            msg: Some(msg),
        };
        // insert to entries
        // self.entries.insert(index, entry);

        self.entries.insert(index, entry);

        if !self.appending && index >= self.last_log_index + self.batch_size {
            self.start_append_logs().await;
        }
    }

    async fn start_append_logs(&mut self) {
        // tracing::info!("start append logs {}", self.last_log_index);
        if self.log_index >= self.last_log_index + self.batch_size {
            self.set_timer();
            // init msg
            let mut prev_log_term = 0;

            if self.last_log_index > 0 {
                prev_log_term = self.entries[&self.last_log_index].term;
            }
            let mut append = RaftMsg {
                entry_type: RaftMsgType::Append.into(),
                replica: self.id,
                prev_log_index: self.last_log_index,
                prev_log_term,
                term: self.current_term,
                entries: Vec::new(),
                leader_commit: self.commit_index,
                success: false,
                entry_len: 0,
            };
            // tracing::info!("self.logindex = {}", self.log_index);

            let mut index = self.last_log_index + 1;
            while index < self.last_log_index + self.batch_size + 1 {
                append.entries.push(self.entries[&index].clone());
                append.entry_len += 1;
                index += 1;
            }

            self.broadcast_to_peers(append).await;
            self.appending = true;
        } else {
            self.appending = false;
        }

        // for index in (self.last_log_index + 1)..(self.last_log_index + self.batch_size + 1) {
        //     // tracing::info!("push ing msg {}", index);

        // }
    }

    async fn handle_append_msg(&mut self, msg: RaftMsg) {
        // tracing::info!(
        //     "handle append msg, term{} index{} replica{}",
        //     msg.term,
        //     msg.prev_log_index,
        //     msg.replica
        // );
        let mut fail = RaftMsg {
            entry_type: RaftMsgType::AppendResponse.into(),
            replica: self.id,
            prev_log_index: 0,
            prev_log_term: 0,
            term: self.current_term,
            entries: Vec::new(),
            leader_commit: 0,
            success: false,
            entry_len: msg.entry_len,
        };
        if msg.term < self.current_term {
            // reply fail
            self.send_to_peer(fail, msg.replica).await;
            return;
        }
        if self.current_term < msg.term {
            self.current_term = msg.term;
        }

        // transition to follower if needed
        self.convert_to_follower(msg.term, msg.replica);
        self.current_leader = msg.replica;
        self.set_timer();
        // a heartbeat
        if msg.prev_log_index == -1 {
            return;
        }
        let prev_index = msg.prev_log_index;
        unsafe {
            if let Some(entry) = self.entries.get(&prev_index) {
                if entry.term != msg.prev_log_term {
                    // reply false
                    self.send_to_peer(fail, msg.replica).await;
                    return;
                } else {
                    let entry_len = msg.entries.len() as i32;
                    // append
                    for (index, iter) in msg.entries.into_iter().enumerate() {
                        self.entries
                            .insert(msg.prev_log_index + 1 + (index as i32), iter);
                    }

                    // check commit index
                    if msg.leader_commit > self.commit_index {
                        self.commit_index = min(msg.leader_commit, entry_len);
                    }
                    // reply success
                    let success = RaftMsg {
                        entry_type: RaftMsgType::AppendResponse.into(),
                        replica: self.id,
                        prev_log_index: msg.prev_log_index,
                        prev_log_term: msg.prev_log_term,
                        term: self.current_term,
                        entries: Vec::new(),
                        leader_commit: 0,
                        success: true,
                        entry_len: msg.entry_len,
                    };
                    self.send_to_peer(success, msg.replica).await;
                }
            } else if msg.prev_log_index == 0 {
                let entry_len = msg.entries.len() as i32;
                // append
                // tracing::info!("msg entries len = {}", msg.entries.len());
                for (index, iter) in msg.entries.into_iter().enumerate() {
                    // tracing::info!(
                    //     "insert into entry index{}",
                    //     msg.prev_log_index + 1 + (index as i32)
                    // );
                    self.entries
                        .insert(msg.prev_log_index + 1 + (index as i32), iter);
                }

                // check commit index
                if msg.leader_commit > self.commit_index {
                    self.commit_index = min(msg.leader_commit, entry_len);
                }
                // reply success
                let success = RaftMsg {
                    entry_type: RaftMsgType::AppendResponse.into(),
                    replica: self.id,
                    prev_log_index: msg.prev_log_index,
                    prev_log_term: msg.prev_log_term,
                    term: self.current_term,
                    entries: Vec::new(),
                    leader_commit: 0,
                    success: true,
                    entry_len: msg.entry_len,
                };
                self.send_to_peer(success, msg.replica).await;
            } else {
                // reply false
                // use prev log index to indicate conflict index
                // tracing::info!("handle append not found, reply false");
                fail.prev_log_index = self.entries.len() as i32;
                self.send_to_peer(fail, msg.replica).await;
                return;
            }
        }
    }

    async fn handle_append_reply(&mut self, msg: RaftMsg) {
        // tracing::info!(
        //     "handle append reply index{}, true{}",
        //     msg.prev_log_index,
        //     msg.success
        // );
        if msg.term > self.current_term {
            self.convert_to_follower(msg.term, -1);
        }
        if msg.term != self.current_term || self.state != State::Leader {
            return;
        }
        if msg.success {
            // update index
            self.match_index[msg.replica as usize] = msg.prev_log_index + msg.entry_len;
            self.next_index[msg.replica as usize] = self.match_index[msg.replica as usize] + 1;

            let mut match_index_copy = self.match_index.clone();
            match_index_copy.sort();
            // tracing::info!("next index {:?}", match_index_copy);
            if match_index_copy[(self.quorum_size) as usize] > self.commit_index {
                self.commit_index = match_index_copy[(self.quorum_size) as usize];
                // tracing::info!("commit index = {}", self.commit_index);
                self.apply_logs();
                self.last_log_index = self.commit_index;
                // start next append phase
                self.start_append_logs().await;
            }
        }
    }

    async fn request_vote(&mut self) {
        let prev_log_index = self.commit_index;
        let mut prev_log_term = 0;
        unsafe {
            if let Some(entry) = self.entries.get(&prev_log_index) {
                prev_log_term = entry.term;
            }
        }
        let request_vote = RaftMsg {
            entry_type: RaftMsgType::RequestVote.into(),
            replica: self.id,
            prev_log_index,
            prev_log_term,
            term: self.current_term,
            entries: Vec::new(),
            leader_commit: 0,
            success: false,
            entry_len: 0,
        };
        // broadcast to peers
        tracing::info!("broadcast request votes");
        self.broadcast_to_peers(request_vote).await;
    }

    async fn handle_request_vote(&mut self, msg: RaftMsg) {
        tracing::info!("handle request vote from {}", msg.replica);
        let mut reply = RaftMsg {
            entry_type: RaftMsgType::RequestVoteResponse.into(),
            replica: self.id,
            prev_log_index: 0,
            prev_log_term: 0,
            term: self.current_term,
            entries: Vec::new(),
            leader_commit: 0,
            success: false,
            entry_len: 0,
        };
        if msg.term < self.current_term {
            // reply false
            self.send_to_peer(reply, msg.replica).await;
        } else if msg.term == self.current_term {
            self.set_timer();
            if self.vote_for != -1 && self.vote_for != msg.replica {
                // reply false
                // send back reply
                self.send_to_peer(reply, msg.replica).await;
            } else {
                let last_log_index = self.last_log_index;
                let mut last_log_term = 0;
                if last_log_index > 0 {
                    unsafe {
                        last_log_term = self.entries.get(&last_log_index).unwrap().term;
                    }
                }
                if msg.prev_log_term < last_log_term {
                    // reply false
                    self.send_to_peer(reply, msg.replica).await;
                } else if msg.prev_log_term == last_log_term && msg.prev_log_term < last_log_index {
                    // reply false
                    self.send_to_peer(reply, msg.replica).await;
                } else {
                    // reply true
                    tracing::info!("reply true to request vote");
                    self.vote_for = msg.replica;
                    reply.success = true;
                    self.send_to_peer(reply, msg.replica).await;
                }
            }
        } else {
            // convert to follower
            self.set_timer();
            self.convert_to_follower(msg.term, -1);
            reply.term = msg.term;
            let last_log_index = self.last_log_index;
            let mut last_log_term = 0;
            if last_log_index > 0 {
                unsafe {
                    last_log_term = self.entries.get(&last_log_index).unwrap().term;
                }
            }
            if msg.prev_log_term < last_log_term {
                // reply false
                self.send_to_peer(reply, msg.replica).await;
            } else if msg.prev_log_term == last_log_term && msg.prev_log_term < last_log_index {
                // reply false
                self.send_to_peer(reply, msg.replica).await;
            } else {
                // reply true
                self.vote_for = msg.replica;
                reply.success = true;
                self.send_to_peer(reply, msg.replica).await;
            }
        }
    }

    fn handle_request_vote_response(&mut self, msg: RaftMsg) {
        tracing::info!("handle vote response from {}", msg.replica);
        if msg.term > self.current_term {
            self.convert_to_follower(msg.term, -1);
            return;
        }

        if self.current_term != msg.term || self.state != State::Candinate {
            tracing::info!(
                "wrong vote reply self term = {}, msg term = {}, state = {:?}",
                self.current_term,
                msg.term,
                self.state
            );
            return;
        }

        if msg.success {
            self.total_votes += 1;
            // if conver to leader
            tracing::info!(
                "success total_votes ={}, quorum size ={}",
                self.total_votes,
                self.quorum_size
            );
            if self.total_votes == self.quorum_size {
                self.convert_to_leader();
            }
        }
    }

    fn apply_logs(&mut self) {
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            // send back to client
            unsafe {
                let client_reply = ClientMsgReply {
                    command_id: self.entries[&self.last_applied]
                        .msg
                        .as_ref()
                        .unwrap()
                        .command_id,
                    success: true,
                };
                // println!("send back to client {}", client_reply.command_id);
                self.client_reply.send(client_reply);
            }
        }
    }

    fn convert_to_leader(&mut self) {
        println!("id = {}, convert to leader", self.id);
        self.current_leader = self.id;
        self.state = State::Leader;
        for i in 0..self.peer_num {
            unsafe {
                self.next_index[i as usize] = (self.entries.len() + 1) as i32;
            }
            self.match_index[i as usize] = 0;
        }
        // set heartbeat timer
        self.timer_handle.abort();
        let (timeout_handle, timeout_registration) = AbortHandle::new_pair();
        let timeout_sender = self.sender.clone();
        //let elec_gen_election_timeout = gen_election_timeout(100, 200);
        let heartbeat_timeout = 500;
        // run timeout task
        self.timer_handle = timeout_handle;
        let notified = self.set_timer.clone();
        tokio::spawn(Abortable::new(
            async move {
                loop {
                    if let Err(_) = timeout(
                        Duration::from_millis(heartbeat_timeout as u64),
                        notified.notified(),
                    )
                    .await
                    {
                        // println!("time out");
                        timeout_sender.send(PeerMsg::TimeOut());
                    }
                    continue;
                }
            },
            timeout_registration,
        ));
    }

    fn convert_to_candinate(&mut self) {
        println!("id = {}, convert to candinate", self.id);
        self.state = State::Candinate;
        self.current_term += 1;
        self.vote_for = self.id;
        self.total_votes = 1;
        // set timer
        self.set_timer();
    }

    fn convert_to_follower(&mut self, term: i32, votefor: i32) {
        // tracing::info!("convert to follower");
        // self.current_leader = votefor;
        self.current_term = term;
        self.vote_for = votefor;
        self.state = State::Follower;
        self.total_votes = 0;
    }

    // helper function
    async fn broadcast_to_peers(&mut self, msg: RaftMsg) {
        for (_id, sender) in self.peers.iter() {
            sender.send(msg.clone()).await;
        }
    }

    async fn send_to_peer(&mut self, msg: RaftMsg, to: i32) {
        let sender = self.peers.get(&to).unwrap();
        sender.send(msg).await;
    }

    fn set_timer(&mut self) {
        self.set_timer.notify_one();
    }

    async fn init_peer_rpc(
        &mut self,
        sender: UnboundedSender<PeerMsg>,
        // client_sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<ClientMsgReply>,
    ) {
        // init peer rpc
        let mut listen_ip = if self.wide_area {
            self.config
                .wide_private_server_addr
                .get(&self.id)
                .unwrap()
                .clone()
        } else {
            self.config.server_addrs.get(&self.id).unwrap().clone()
        };
        listen_ip = convert_ip_addr(listen_ip, false);
        info!("server listen ip {}", listen_ip);
        let server = RpcServer::new(listen_ip, sender.clone());

        tokio::spawn(async move {
            run_server(server).await;
        });
        // init rpc client to connect to other peers
        for (id, ip) in self.config.server_addrs.iter() {
            if *id != self.id {
                tracing::info!("init client connect to {}", ip);
                // let mut client = PeerCommunicationClient::connect(ip).await?;
                let (send_to_server, server_receiver) = channel::<RaftMsg>(10000);
                //init client
                let mut client = RpcClient::new(ip.clone()).await;
                tokio::spawn(async move {
                    client.run_client(server_receiver).await;
                });
                self.peers.insert(id.clone(), send_to_server);
            }
        }
        // init propose rpc server
        let propose_ip = if self.wide_area {
            convert_ip_addr(
                self.config.wide_private_propose_addr[&self.id].clone(),
                false,
            )
        } else {
            convert_ip_addr(self.config.propose_server_addrs[&self.id].clone(), false)
        };
        let propose_server = ProposeServer::new(propose_ip, sender, receiver);
        tokio::spawn(async move {
            run_propose_server(propose_server).await;
        });
    }
}
