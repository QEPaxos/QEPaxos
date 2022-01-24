use std::collections::HashMap;

use crate::err::Result;
use crate::execution::ExecEngine;
use crate::peer_communication::{
    run_propose_server, run_server, ProposeServer, RpcClient, RpcServer,
};
use crate::{peer, ClientMsg, PeerMsg};
use common::config::ServerConfig;
use common::{convert_ip_addr, Instance};
use tokio::sync::mpsc::{channel, Sender, UnboundedReceiver, UnboundedSender};
use tokio::task::spawn_blocking;
use tracing::info;

use rpc::qepaxos_rpc::{ClientMsgReply, LogStatus, Msg, MsgType};

// logs matrix
pub static mut LOGS: Vec<Vec<Option<LogEntry>>> = Vec::new();
pub static mut EXECUTED_UP_TO: Vec<i32> = Vec::new();
pub static mut CRT_INSTANCE: Vec<i32> = Vec::new();

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub responses: Vec<Msg>,
    pub status: LogStatus,
    pub msg: Msg,
    pub can_fast_commit: bool,
    pub accept_count: usize,
    pub leader_responded: bool,
    // use for exec
    pub dfn: i32,
    pub low: i32,
}

impl LogEntry {
    pub fn new(responses: Vec<Msg>, status: LogStatus, msg: Msg) -> Self {
        Self {
            responses,
            status,
            msg,
            can_fast_commit: true,
            accept_count: 0,
            leader_responded: false,
            dfn: -1,
            low: -1,
        }
    }
}

pub struct Peer {
    id: i32,
    // sn ballot
    ballot: i32,
    sn: i32,
    //thrifty
    thrifty: bool,
    preferred_peer: Vec<i32>,
    alive_replicas: Vec<i32>,
    replica_mask: Vec<u8>,
    // fast quorum size
    fast_quorom_size: usize,
    paxos_quorum_size: usize,
    test_msg_count_per_server: usize,
    peer_num: usize,
    deps: HashMap<String, Vec<i32>>,
    commited_up_to: Vec<i32>,
    instance: i32,
    sender_to_self: UnboundedSender<PeerMsg>,
    receiver: UnboundedReceiver<PeerMsg>,
    // used to init peer rpc
    peer_addrs: HashMap<i32, String>,
    propose_addr: HashMap<i32, String>,
    wide_area: bool,
    wide_peer_addr: HashMap<i32, String>,
    wide_propose_addr: HashMap<i32, String>,
    //sender to peer
    peer_senders: HashMap<i32, Sender<Msg>>,

    // send put result back to client
    client_senders: UnboundedSender<ClientMsgReply>,
    execute: bool,
    batch_size: usize,
    batch_request: Vec<ClientMsg>,
}

impl Peer {
    pub fn new(
        id: i32,
        config: ServerConfig,
        sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<PeerMsg>,
        client_senders: UnboundedSender<ClientMsgReply>,
        execute: bool,
        batch_size: usize,
        thrifty: bool,
        wide_area: bool,
    ) -> Self {
        // init ballot
        let peer_num = config.peer_num;
        let mut quorum: u8 = 0;
        let mask = vec![
            0b0000_0001,
            0b0000_0010,
            0b0000_0100,
            0b0000_1000,
            0b0001_0000,
        ];

        let mut alive_replicas = Vec::new();
        for i in 0..peer_num {
            quorum |= mask[i];
            if i != id as usize {
                alive_replicas.push(i as i32);
            }
        }
        tracing::info!("init quorum = {}, {}", quorum, quorum as i32);
        tracing::info!(
            "prefeered_peer {:?}",
            config.preferred_peer[id as usize].clone()
        );
        Self {
            id,
            ballot: 0 + quorum as i32,
            sn: 0,
            thrifty,
            preferred_peer: config.preferred_peer[id as usize].clone(),
            alive_replicas,
            replica_mask: mask,
            fast_quorom_size: config.fast_quorum_size,
            paxos_quorum_size: config.paxos_quorum_size,
            peer_num,
            deps: HashMap::new(),
            test_msg_count_per_server: config.test_msg_count_per_server,
            commited_up_to: vec![0; config.peer_num],
            instance: 0,
            sender_to_self: sender,
            receiver,
            peer_addrs: config.server_addrs,
            propose_addr: config.propose_server_addrs.clone(),
            wide_peer_addr: config.wide_private_server_addr.clone(),
            wide_propose_addr: config.wide_private_propose_addr.clone(),
            peer_senders: HashMap::new(),
            client_senders,
            execute,
            batch_size,
            batch_request: Vec::new(),
            wide_area,
        }
    }

    pub async fn init_and_run(&mut self, receiver: UnboundedReceiver<ClientMsgReply>) {
        // init global logs
        unsafe {
            LOGS = vec![vec![None; self.test_msg_count_per_server]; self.peer_num];

            CRT_INSTANCE = vec![1; self.peer_num];
            EXECUTED_UP_TO = vec![1; self.peer_num];
        }
        // init rpc
        self.init_peer_rpc(self.sender_to_self.clone(), receiver)
            .await;
        tracing::info!("init rpc done");
        // init exec thread
        if self.execute {
            let mut exec = ExecEngine::new(
                self.client_senders.clone(),
                self.sender_to_self.clone(),
                self.peer_num,
                100000,
                self.id as usize,
            );
            spawn_blocking(move || exec.run());
        }

        self.run().await;
    }

    fn get_and_update_deps(
        &mut self,
        commands: &Vec<ClientMsg>,
        self_deps: &mut Vec<i32>,
        peer_id: i32,
        instance: i32,
    ) {
        for kv in commands.iter() {
            match self.deps.get_mut(&kv.key) {
                Some(dep) => {
                    // match
                    (0..self.peer_num).for_each(|peer| {
                        if peer != peer_id as usize {
                            if dep[peer] < self_deps[peer] {
                                dep[peer] = self_deps[peer];
                            } else {
                                self_deps[peer] = dep[peer];
                            }
                        } else {
                            if dep[peer] < instance {
                                dep[peer] = instance;
                            }
                        }
                    });
                }
                None => {
                    // insert key with it's deps
                    let mut new_dep = self_deps.clone();
                    new_dep[peer_id as usize] = instance;
                    self.deps.insert(kv.key.clone(), new_dep);
                }
            }
        }
    }

    fn update_deps(
        &mut self,
        commands: &Vec<ClientMsg>,
        deps: &Vec<i32>,
        replica: i32,
        instance: i32,
    ) {
        for kv in commands.iter() {
            match self.deps.get_mut(&kv.key) {
                Some(dep) => {
                    // update deps
                    for (peer_id, dep_instance) in dep.iter_mut().enumerate() {
                        let commit_instance = deps.get(peer_id).unwrap();
                        if peer_id != replica as usize {
                            if *dep_instance < *commit_instance {
                                *dep_instance = *commit_instance;
                            }
                        } else {
                            if *dep_instance < instance {
                                *dep_instance = instance;
                            }
                        }
                    }
                }
                None => {
                    // insert key with it's deps
                    let mut dep = deps.clone();
                    dep[replica as usize] = instance;
                    self.deps.insert(kv.key.clone(), dep);
                }
            }
        }
    }

    async fn handle_msg(&mut self, msg: Msg) {
        let (sn, mut quorum) = self.get_sn_quorum(msg.ballot);
        if sn < self.sn || (sn == self.sn && msg.ballot != self.ballot) {
            // reply out of date
            let outofdate = Msg {
                entry_type: MsgType::OutOfDate.into(),
                from: self.id,
                replica: self.id,
                instance: msg.instance,
                ballot: self.ballot,
                deps: Vec::new(),
                commands: Vec::new(),
                status: None,
                ok: None,
            };
            // tracing::info!("out of date reply to{}", msg.from);
            self.send_to_peer(outofdate, msg.from).await;
        } else {
            // update new ballot
            if msg.ballot != self.ballot {
                self.sn = sn;
                if (quorum & self.replica_mask[self.id as usize]) == 0 {
                    quorum != self.replica_mask[self.id as usize];
                }
                self.ballot = self.sn << 8 + quorum as i32;
            }
            match msg.entry_type() {
                MsgType::PreAccept => self.process_preaccept(msg).await,
                MsgType::PreAcceptReply => self.process_preaccept_response(msg).await,
                MsgType::Accept => self.process_accept(msg).await,
                MsgType::AcceptReply => self.process_accept_response(msg).await,
                MsgType::Commit => self.process_commit(msg),
                MsgType::Prepare => self.handle_prepare(msg).await,
                MsgType::PrepareReply => self.handle_prepare_response(msg).await,
                MsgType::PreCommit => self.process_pre_commit(msg),
                MsgType::OutOfDate => self.handle_outofdate(msg).await,
            }
        }
    }

    async fn process_client_msg(&mut self, client_msg: ClientMsg) {
        // tracing::info!("handling client msg key = {:?}", client_msg.key);
        // insert into batch
        self.batch_request.push(client_msg);
        if self.batch_request.len() == self.batch_size {
            let instance = self.instance + 1;
            self.instance += 1;
            let mut deps: Vec<i32> = vec![0; self.peer_num];
            deps[self.id as usize] = instance - 1;
            let commands = self.batch_request.clone();
            self.get_and_update_deps(&commands, &mut deps, self.id, instance);
            // init deps & pre accept msg
            let pre_accept_msg = Msg {
                entry_type: MsgType::PreAccept.into(),
                from: self.id,
                replica: self.id,
                instance,
                deps,
                commands,
                ballot: self.ballot,
                status: None,
                ok: None,
            };
            // insert to logs
            let log_entry =
                LogEntry::new(Vec::new(), LogStatus::PreAccepted, pre_accept_msg.clone());
            unsafe {
                LOGS[self.id as usize][instance as usize] = Some(log_entry);

                CRT_INSTANCE[self.id as usize] += 1;
            }
            // broadcast pre-accept msg
            // tracing::info!("boradcast preaccept msg = {:?}", pre_accept_msg);
            self.broadcast_to_peers(&pre_accept_msg).await;
            self.batch_request.clear();
        }
    }

    async fn process_preaccept(&mut self, msg: Msg) {
        let replica = msg.replica;
        let instance = msg.instance;

        unsafe {
            if CRT_INSTANCE[replica as usize] < instance + 1 {
                CRT_INSTANCE[replica as usize] = instance + 1;
            }
            let entry_option = LOGS[replica as usize][instance as usize].as_mut();
            match entry_option {
                Some(entry) => {
                    if entry.msg.ballot >= msg.ballot {
                        return;
                    } else {
                        // todo insert to LOGS and reply
                        // insert into logs
                        let entry = LogEntry::new(Vec::new(), LogStatus::PreAccepted, msg.clone());
                        LOGS[replica as usize][instance as usize] = Some(entry);
                        let mut response = msg;
                        // update deps
                        // tracing::info!("self deps = {:?}", self.deps);
                        // tracing::info!("old msg dep = {:?}", response.deps);
                        self.get_and_update_deps(
                            &response.commands,
                            &mut response.deps,
                            response.from,
                            response.instance,
                        );
                        // tracing::info!("after update dep = {:?}", response.deps);

                        let to = response.from;
                        response.from = self.id;
                        response.entry_type = MsgType::PreAcceptReply.into();
                        // tracing::info!("pre accept reply to{}", to);
                        self.send_to_peer(response, to).await;
                    }
                }
                None => {
                    // insert into logs
                    let entry = LogEntry::new(Vec::new(), LogStatus::PreAccepted, msg.clone());
                    LOGS[replica as usize][instance as usize] = Some(entry);
                    let mut response = msg;
                    // update deps
                    // tracing::info!("self deps = {:?}", self.deps);
                    // tracing::info!("old msg dep = {:?}", response.deps);
                    self.get_and_update_deps(
                        &response.commands,
                        &mut response.deps,
                        response.from,
                        response.instance,
                    );
                    // tracing::info!("after update dep = {:?}", response.deps);

                    let to = response.from;
                    response.from = self.id;
                    response.entry_type = MsgType::PreAcceptReply.into();
                    // tracing::info!("preaccept reply to{}", to);
                    self.send_to_peer(response, to).await;
                }
            }
        }
    }

    async fn process_preaccept_response(&mut self, msg: Msg) {
        unsafe {
            let mut log_entry = LOGS[msg.replica as usize][msg.instance as usize]
                .as_mut()
                .unwrap();
            if log_entry.status != LogStatus::PreAccepted {
                return;
            }
            log_entry.responses.push(msg.clone());
            if log_entry.responses.len() == self.fast_quorom_size - 1 && log_entry.can_fast_commit {
                if self.thrifty {
                    // if (self.id as usize) < self.fast_quorom_size - 1 {
                    //     //
                    // } else {
                    //     for i in 0..self.fast_quorom_size {}
                    // }
                    log_entry.status = LogStatus::Commited;
                    let mut commit_msg = log_entry.msg.clone();
                    commit_msg.entry_type = MsgType::Commit.into();
                    if !self.execute {
                        for iter in log_entry.msg.commands.iter() {
                            let rsp = ClientMsgReply {
                                command_id: iter.command_id,
                                success: true,
                            };
                            if let Err(e) = self.client_senders.send(rsp) {
                                tracing::info!("send client reply channel error {}", e);
                            }
                        }
                    }
                    // tracing::info!("fast commit instance = {:?}", commit_msg.instance);
                    self.broadcast_to_peers(&commit_msg).await;
                } else {
                    // check if can commit at fast path
                    for response in log_entry.responses.iter() {
                        if response.deps != log_entry.msg.deps {
                            log_entry.can_fast_commit = false;
                            return;
                        }
                    }
                    // commit at fast path
                    log_entry.status = LogStatus::Commited;
                    let mut commit_msg = log_entry.msg.clone();
                    commit_msg.entry_type = MsgType::Commit.into();
                    if !self.execute {
                        for iter in log_entry.msg.commands.iter() {
                            let rsp = ClientMsgReply {
                                command_id: iter.command_id,
                                success: true,
                            };
                            if let Err(e) = self.client_senders.send(rsp) {
                                tracing::info!("send client reply channel error {}", e);
                            }
                        }
                    }
                    // tracing::info!("fast commit instance = {:?}", commit_msg.instance);
                    self.broadcast_to_peers(&commit_msg).await;
                }
            } else if log_entry.responses.len() == self.alive_replicas.len()
                && log_entry.status != LogStatus::Commited
            {
                // commit at slow path
                // tracing::info!("slow commit instance {}", msg.instance);
                // calculate votes for each dep
                let id = self.id;
                let fq_size = self.fast_quorom_size;
                (0..self.peer_num).for_each(|index| {
                    // collect all the deps for index
                    if index != id as usize {
                        let mut dep_response = Vec::new();
                        for response in log_entry.responses.iter() {
                            dep_response.push(response.deps[index]);
                        }
                        dep_response.push(log_entry.msg.deps[index]);
                        dep_response.sort();
                        // tracing::info!("index = {}, deps = {:?}", index, dep_response);
                        // find the largest index with FQ_Size votes
                        //todo
                        log_entry.msg.deps[index] = dep_response[fq_size - 1];
                    }
                });
                if self.alive_replicas.len() == self.peer_num - 1 {
                    // pre commit
                    // boradcast pre commit to all peers
                    log_entry.status = LogStatus::PreCommited;
                    let mut commit_msg = log_entry.msg.clone();
                    commit_msg.entry_type = MsgType::PreCommit.into();
                    // tracing::info!("slow commit msg = {:?}", commit_msg);
                    self.broadcast_to_peers(&commit_msg).await;

                    // reply to client
                    if !self.execute {
                        for iter in log_entry.msg.commands.iter() {
                            let rsp = ClientMsgReply {
                                command_id: iter.command_id,
                                success: true,
                            };
                            if let Err(e) = self.client_senders.send(rsp) {
                                tracing::info!("send client reply channel error {}", e);
                            }
                        }
                    }
                } else {
                    // accept
                    // boradcast pre commit to all peers
                    log_entry.status = LogStatus::Accepted;
                    let mut accept_msg = log_entry.msg.clone();
                    accept_msg.entry_type = MsgType::Accept.into();
                    // tracing::info!("slow commit msg = {:?}", commit_msg);
                    self.broadcast_to_peers(&accept_msg).await;
                }
            }
        }
    }

    async fn process_accept(&mut self, msg: Msg) {
        // update deps
        self.update_deps(&msg.commands, &msg.deps, msg.replica, msg.instance);
        // just put msg into logs and reply response
        let to = msg.from;
        let replica = msg.replica;
        let instance = msg.instance;
        unsafe {
            if CRT_INSTANCE[msg.replica as usize] < msg.instance + 1 {
                CRT_INSTANCE[msg.replica as usize] = msg.instance + 1;
            }
            let log = LOGS[replica as usize][instance as usize].as_mut();
            match log {
                Some(log_entry) => {
                    log_entry.msg = msg;
                    log_entry.status = LogStatus::Accepted;
                }
                None => {
                    // insert new log entry
                    let entry = LogEntry::new(Vec::new(), LogStatus::Accepted, msg);

                    LOGS[replica as usize][instance as usize] = Some(entry);
                }
            }
        }
        // send back response
        let response = Msg {
            entry_type: MsgType::AcceptReply.into(),
            from: self.id,
            replica,
            instance,
            deps: Vec::new(),
            commands: Vec::new(),
            ballot: self.ballot,
            status: None,
            ok: None,
        };
        // tracing::info!("accept response to {}", to);
        self.send_to_peer(response, to).await;
    }

    async fn process_accept_response(&mut self, msg: Msg) {
        //
        unsafe {
            let entry = LOGS[msg.replica as usize][msg.instance as usize]
                .as_mut()
                .unwrap();
            entry.accept_count += 1;
            if entry.accept_count == self.paxos_quorum_size {
                // broadcast commit
                let mut commit_msg = entry.msg.clone();
                commit_msg.entry_type = MsgType::Commit.into();
                // tracing::info!("slow commit msg = {:?}", commit_msg);
                self.broadcast_to_peers(&commit_msg).await;
            }
        }
    }

    fn process_commit(&mut self, msg: Msg) {
        // update deps
        self.update_deps(&msg.commands, &msg.deps, msg.replica, msg.instance);

        // insert to logs
        unsafe {
            if CRT_INSTANCE[msg.replica as usize] < msg.instance + 1 {
                CRT_INSTANCE[msg.replica as usize] = msg.instance + 1;
            }
            let instance = msg.instance;
            let replica = msg.replica;
            let entry = LOGS[replica as usize][instance as usize].as_mut();
            // tracing::info!("got commit {}-{}", replica, instance);
            match entry {
                Some(log_entry) => {
                    log_entry.msg = msg;
                    log_entry.status = LogStatus::Commited;
                }
                None => {
                    let log_entry = LogEntry::new(Vec::new(), LogStatus::Commited, msg);
                    LOGS[replica as usize][instance as usize] = Some(log_entry);
                }
            }
        }
    }

    fn process_pre_commit(&mut self, msg: Msg) {
        // update deps
        self.update_deps(&msg.commands, &msg.deps, msg.replica, msg.instance);

        // insert to logs
        unsafe {
            if CRT_INSTANCE[msg.replica as usize] < msg.instance + 1 {
                CRT_INSTANCE[msg.replica as usize] = msg.instance + 1;
            }
            let instance = msg.instance;
            let replica = msg.replica;
            let entry = LOGS[replica as usize][instance as usize].as_mut();
            // tracing::info!("got commit {}-{}", replica, instance);
            match entry {
                Some(log_entry) => {
                    log_entry.msg = msg;
                    log_entry.status = LogStatus::PreCommited;
                }
                None => {
                    let log_entry = LogEntry::new(Vec::new(), LogStatus::PreCommited, msg);
                    LOGS[replica as usize][instance as usize] = Some(log_entry);
                }
            }
        }
    }

    fn get_sn_quorum(&mut self, ballot: i32) -> (i32, u8) {
        (ballot >> 8, ballot as u8)
    }

    async fn handle_outofdate(&mut self, msg: Msg) {
        // check ballot sn
        let ballot = msg.ballot;
        tracing::info!("got out of date msg {:?}", msg);
        let (sn, quorum) = self.get_sn_quorum(ballot);
        if sn >= self.sn {
            // update ballot
            self.sn = sn + 1;
            let mut new_quorum = quorum;
            if (self.replica_mask[self.id as usize] & quorum) == 0 {
                new_quorum |= self.replica_mask[self.id as usize];
            }
            self.ballot = self.sn >> 8 + new_quorum as i32;
            // retry op
            unsafe {
                let mut entry = LOGS[self.id as usize][msg.instance as usize]
                    .as_mut()
                    .unwrap();
                entry.msg.ballot = self.ballot;
                let mut new_msg = entry.msg.clone();
                match entry.status {
                    LogStatus::Preparing => new_msg.entry_type = MsgType::Prepare.into(),
                    LogStatus::PreAccepted => new_msg.entry_type = MsgType::PreAccept.into(),
                    LogStatus::PreAcceptedEq => todo!(),
                    LogStatus::Accepted => new_msg.entry_type = MsgType::Accept.into(),
                    LogStatus::Commited => new_msg.entry_type = MsgType::Commit.into(),
                    LogStatus::Executed => {}
                    LogStatus::PreCommited => new_msg.entry_type = MsgType::PreCommit.into(),
                }
                self.broadcast_to_peers(&new_msg).await;
            }
        }
    }

    async fn handle(&mut self, peer_msg: PeerMsg) {
        // tracing::info!("handle msg {:?}", peer_msg);
        match peer_msg {
            PeerMsg::Msg(msg) => self.handle_msg(msg).await,
            PeerMsg::ClientMsg(msg) => self.process_client_msg(msg).await,
            PeerMsg::TimeOut(inst) => self.handle_timeout(inst).await,
        }
    }

    async fn handle_timeout(&mut self, inst: Instance) {
        let replica = inst.replica;
        tracing::info!("handle timeout, sn{}", self.sn);
        // update ballot
        let mut new_quorum = self.replica_mask[self.id as usize];
        unsafe {
            // update quorum
            let inst_entry = LOGS[replica as usize][inst.instance].as_mut().unwrap();
            let inst_ballot = inst.ballot;
            let (inst_sn, _) = self.get_sn_quorum(inst_ballot);
            if self.sn < inst_sn + 1 {
                self.sn = inst_sn + 1;
            }

            for response in inst_entry.responses.iter() {
                new_quorum |= self.replica_mask[response.from as usize];
            }

            self.ballot = self.sn << 8 + new_quorum as i32;
        }
        if replica == self.id {
            //range from last committed to crt
            let start_inst = self.commited_up_to[self.id as usize];
            unsafe {
                // retry all non-decide op
                let last_inst = CRT_INSTANCE[self.id as usize];
                for i in start_inst..last_inst {
                    if let Some(log_entry) = LOGS[replica as usize][i as usize].as_mut() {
                        // if got enough responses in quorum
                        // update ballot in log entry
                        log_entry.msg.ballot = self.ballot;
                        if log_entry.responses.len() == self.alive_replicas.len() {
                            // update deps
                            let fq_size = self.fast_quorom_size;
                            (0..self.peer_num).for_each(|index| {
                                // collect all the deps for index
                                if index != self.id as usize {
                                    let mut dep_response = Vec::new();
                                    for response in log_entry.responses.iter() {
                                        dep_response.push(response.deps[index]);
                                    }
                                    dep_response.push(log_entry.msg.deps[index]);
                                    dep_response.sort();
                                    // tracing::info!("index = {}, deps = {:?}", index, dep_response);
                                    // find the largest index with FQ_Size votes
                                    //todo
                                    log_entry.msg.deps[index] = dep_response[fq_size - 1];
                                }
                            });
                            // boradcast commit to all peers
                            log_entry.status = LogStatus::Accepted;
                            let mut accept_msg = log_entry.msg.clone();
                            accept_msg.entry_type = MsgType::Accept.into();
                            // tracing::info!("slow commit msg = {:?}", commit_msg);
                            self.broadcast_to_peers(&accept_msg).await;
                        }
                    }
                }
            }
        } else {
            self.recovery(inst.replica, inst.instance).await;
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(msg) = self.receiver.recv().await {
                self.handle(msg).await;
            }
        }
    }

    async fn send_to_peer(&mut self, msg: Msg, to: i32) {
        // tracing::info!("send to peer {}", to);
        self.peer_senders.get(&to).unwrap().send(msg).await;
    }

    async fn broadcast_to_peers(&self, msg: &Msg) {
        if self.thrifty {
            for replica in self.preferred_peer.iter() {
                self.peer_senders
                    .get(replica)
                    .unwrap()
                    .send(msg.clone())
                    .await;
            }
        } else {
            for replica in self.alive_replicas.iter() {
                // tracing::info!("msg {:?},alive replica{}", msg, *replica);
                let send_msg = msg.clone();
                self.peer_senders.get(replica).unwrap().send(send_msg).await;
                // sender.send(send_msg).await;
            }
        }
    }

    /////////////////////////////////////////////////////////////////////////////////
    /////below are the recovery functions

    async fn recovery(&mut self, replica: i32, instance: usize) {
        // broadcast prepare to all peers
        tracing::info!("recovery instance {}-{}", replica, instance);
        unsafe {
            match LOGS[replica as usize][instance].as_mut() {
                Some(entry) => {
                    if entry.status == LogStatus::Commited || entry.status == LogStatus::Executed {
                        return;
                    }
                    entry.msg.ballot = self.ballot;
                    entry.msg.entry_type = MsgType::Prepare.into();
                    // clear response
                    entry.responses = Vec::new();
                    entry.status = LogStatus::Preparing;
                }
                None => {
                    // insert a none msg
                    let msg = Msg {
                        entry_type: MsgType::Prepare.into(),
                        from: self.id,
                        replica,
                        instance: instance as i32,
                        ballot: self.ballot,
                        deps: Vec::new(),
                        commands: Vec::new(),
                        status: None,
                        ok: None,
                    };
                    let entry = LogEntry::new(Vec::new(), LogStatus::PreAccepted, msg);
                    LOGS[replica as usize][instance] = Some(entry);
                }
            }

            // broadcast to all peers
            let mut prepare_msg = LOGS[replica as usize][instance]
                .as_ref()
                .unwrap()
                .msg
                .clone();
            prepare_msg.from = self.id;
            self.broadcast_to_peers(&prepare_msg).await;
        }
    }

    async fn handle_prepare(&mut self, msg: Msg) {
        // if have not received a msg at instance
        tracing::info!("got a prepare msg {:?}", msg);
        unsafe {
            let to = msg.from;
            let replica = msg.replica;
            let instance = msg.instance;
            match LOGS[replica as usize][instance as usize].as_mut() {
                Some(entry) => {
                    let mut ok = true;
                    if msg.ballot < entry.msg.ballot {
                        ok = false;
                    } else {
                        entry.msg.ballot = msg.ballot;
                    }
                    let reply = Msg {
                        entry_type: MsgType::PrepareReply.into(),
                        from: self.id,
                        replica,
                        instance,
                        ballot: entry.msg.ballot,
                        deps: entry.msg.deps.clone(),
                        commands: entry.msg.commands.clone(),
                        status: entry.msg.status,
                        ok: Some(ok),
                    };
                    tracing::info!("prepare reply to{}", to);
                    self.send_to_peer(reply, to).await;
                }
                None => {
                    // insert to logs & reply ok
                    let ballot = msg.ballot;
                    let entry = LogEntry::new(Vec::new(), LogStatus::Preparing, msg);
                    LOGS[replica as usize][instance as usize] = Some(entry);
                    let reply_ok = Msg {
                        entry_type: MsgType::PrepareReply.into(),
                        from: self.id,
                        replica,
                        instance,
                        ballot,
                        deps: Vec::new(),
                        commands: Vec::new(),
                        status: None,
                        ok: Some(true),
                    };
                    //
                    tracing::info!("prepare reply to{}", to);
                    self.send_to_peer(reply_ok, to).await;
                }
            }
        }
    }

    async fn handle_prepare_response(&mut self, msg: Msg) {
        unsafe {
            let replica = msg.replica;
            let instance = msg.instance;
            match LOGS[replica as usize][instance as usize].as_mut() {
                Some(entry) => {
                    if entry.status != LogStatus::Preparing {
                        return;
                    }
                    if !msg.ok() {
                        // TODO: there is probably another active leader, back off and retry later
                        if entry.msg.ballot < msg.ballot {
                            entry.msg.ballot = msg.ballot;
                        }
                        return;
                    }
                    // insert into response
                    entry.responses.push(msg.clone());
                    if msg.status() == LogStatus::Commited || msg.status() == LogStatus::Executed {
                        // insert into logs
                        entry.status = LogStatus::Commited;
                        entry.msg = msg.clone();
                        // entry.msg.commands = msg.commands;
                        // entry.msg.ballot = msg.ballot;
                        // entry.msg.deps = msg.deps;
                        // let commit_msg = Msg {
                        //     entry_type: MsgType::Commit.into(),
                        //     from: self.id,
                        //     replica,
                        //     instance,
                        //     ballot: msg.ballot,
                        //     deps: entry.msg.deps.clone(),
                        //     commands: entry.msg.commands.clone(),
                        //     status: None,
                        //     ok: None,
                        // };
                        // self.broadcast_to_peers(&commit_msg).await;
                    } else if msg.status() == LogStatus::Accepted {
                        if entry.msg.commands.len() == 0 {
                            entry.msg.status = msg.status;
                            entry.msg.commands = msg.commands.clone();
                            entry.msg.deps = msg.deps.clone();
                        }
                        // saft to go to accept phase
                        entry.status = LogStatus::Accepted;
                        let accept_msg = Msg {
                            entry_type: MsgType::Accept.into(),
                            from: self.id,
                            replica: msg.replica,
                            instance: msg.instance,
                            ballot: self.ballot,
                            deps: msg.deps,
                            commands: msg.commands,
                            status: None,
                            ok: None,
                        };
                        self.broadcast_to_peers(&accept_msg).await;
                    } else if msg.status() == LogStatus::PreAccepted {
                        if entry.msg.commands.len() == 0 && msg.commands.len() != 0 {
                            entry.msg.status = msg.status;
                            entry.msg.commands = msg.commands;
                            entry.msg.deps = msg.deps.clone();
                        }
                        if msg.from == msg.replica {
                            // leader reply?
                            entry.leader_responded = true;
                        }
                        match LOGS[msg.from as usize][msg.deps[msg.from as usize] as usize].as_ref()
                        {
                            Some(dep_entry) => {
                                if dep_entry.status != LogStatus::Commited
                                    || dep_entry.status != LogStatus::Executed
                                {
                                    entry.status = LogStatus::PreAccepted;
                                    return;
                                }
                            }
                            None => {
                                entry.status = LogStatus::PreAccepted;
                                return;
                            }
                        }
                    }
                    if entry.responses.len() == self.alive_replicas.len() {
                        let entry_msg = &mut entry.msg;
                        if entry_msg.commands.len() == 0 {
                            // try to finalize with no op
                            entry_msg.deps[entry_msg.replica as usize] = entry_msg.instance - 1;
                            let accept_msg = Msg {
                                entry_type: MsgType::Accept.into(),
                                from: self.id,
                                replica: entry_msg.replica,
                                instance: entry_msg.instance,
                                ballot: entry_msg.ballot,
                                deps: entry_msg.deps.clone(),
                                commands: entry_msg.commands.clone(),
                                status: None,
                                ok: None,
                            };
                            self.broadcast_to_peers(&accept_msg).await;
                        } else {
                            if !entry.leader_responded {
                                // finalize dep
                                let fq_size = self.fast_quorom_size;
                                (0..self.peer_num).for_each(|index| {
                                    // collect all the deps for index
                                    let mut dep_response = Vec::new();
                                    for response in entry.responses.iter() {
                                        dep_response.push(response.deps[index]);
                                    }
                                    dep_response.push(entry.msg.deps[index]);
                                    dep_response.sort();
                                    entry.msg.deps[index] = dep_response[fq_size - 1];
                                });
                                // boradcast commit to all peers
                                entry.status = LogStatus::Accepted;
                                let mut accept_msg = entry.msg.clone();
                                accept_msg.entry_type = MsgType::Commit.into();
                                self.broadcast_to_peers(&accept_msg).await;
                            } else {
                                // leader responded, wait for leader to decide
                                entry.status = LogStatus::PreAccepted;
                                return;
                                // let pre_accept_msg = Msg {
                                //     entry_type: MsgType::PreAccept.into(),
                                //     from: self.id,
                                //     replica: entry_msg.replica,
                                //     instance: entry_msg.instance,
                                //     ballot: entry_msg.ballot,
                                //     deps: entry_msg.deps.clone(),
                                //     commands: entry_msg.commands.clone(),
                                //     status: None,
                                //     ok: None,
                                // };
                                // self.broadcast_to_peers(&pre_accept_msg).await;
                            }
                        }
                    }
                }
                None => return,
            }
        }
    }

    // fn find_preaccept_deps(&mut self, msg: &Msg) {
    //     if let Some(entry) = self.logs[msg.replica as usize].get(&msg.instance) {
    //         if entry.msg.commands.len() > 0 {
    //             if entry.status >= LogStatus::Accepted {
    //                 return;
    //             }
    //             if entry.msg.deps == msg.deps {
    //                 // already preaccepted
    //                 return;
    //             }
    //         }
    //         for i in 0..(self.peer_num - 1) {
    //             for instance in self.executed_up_to[i]..CRT_INSTANCE[i] {
    //                 if instance == msg.instance && i == msg.replica as usize {
    //                     // no need to check past instance in replica's row
    //                     break;
    //                 }
    //                 if instance == msg.deps[i as usize] {
    //                     continue;
    //                 }
    //                 match self.logs[i as usize].get(&instance) {
    //                     Some(conflict_entry) => {
    //                         if conflict_entry.msg.commands.len() == 0 {
    //                             continue;
    //                         } else if conflict_entry.msg.deps[msg.replica as usize] >= msg.instance
    //                         {
    //                             continue;
    //                         } else if conflict_commands(&conflict_entry.msg.commands, &msg.commands)
    //                         {
    //                             // if find a conflict
    //                         }
    //                     }
    //                     None => {
    //                         continue;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // }

    //////////////////////////////////////////////////////////////////////////////////
    //////init helper function
    async fn init_peer_rpc(
        &mut self,
        sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<ClientMsgReply>,
    ) -> Result<()> {
        // init peer rpc
        let mut listen_ip = if !self.wide_area {
            self.peer_addrs.get(&self.id).unwrap().clone()
        } else {
            self.wide_peer_addr.get(&self.id).unwrap().clone()
        };
        listen_ip = convert_ip_addr(listen_ip, false);
        info!("server listen ip {}", listen_ip);
        let server = RpcServer::new(listen_ip, sender.clone());

        tokio::spawn(async move {
            run_server(server).await;
        });
        // init rpc client to connect to other peers
        for (id, ip) in self.peer_addrs.iter() {
            if *id != self.id {
                tracing::info!("init client connect to {}", ip);
                // let mut client = PeerCommunicationClient::connect(ip).await?;
                let (send_to_server, server_receiver) = channel::<Msg>(10000);
                //init client
                let mut client = RpcClient::new(ip.clone()).await;
                tokio::spawn(async move {
                    client.run_client(server_receiver).await;
                });
                self.peer_senders.insert(id.clone(), send_to_server);
            }
        }
        // init propose rpc server
        let propose_ip = if !self.wide_area {
            convert_ip_addr(self.propose_addr.get(&self.id).unwrap().clone(), false)
        } else {
            convert_ip_addr(self.wide_propose_addr.get(&self.id).unwrap().clone(), false)
        };
        let propose_server = ProposeServer::new(propose_ip, sender, receiver);
        tokio::spawn(async move {
            run_propose_server(propose_server).await;
        });
        Ok(())
    }
}
