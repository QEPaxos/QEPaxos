use std::collections::HashMap;

use crate::err::Result;
use crate::execution::ExecEngine;
use crate::peer_communication::{
    run_propose_server, run_server, ProposeServer, RpcClient, RpcServer,
};
use crate::{convert_ip_addr, ClientMsg, PeerMsg};
use common::config::ServerConfig;
use tokio::sync::mpsc::{channel, Sender, UnboundedReceiver, UnboundedSender};
use tokio::task::spawn_blocking;
use tracing::info;

use rpc::epaxos_rpc::{LogStatus, Msg, MsgType};
use rpc::sepaxos_rpc::ClientMsgReply;

// logs matrix
pub static mut LOGS: Vec<Vec<Option<LogEntry>>> = Vec::new();
pub static mut EXECUTED_UP_TO: Vec<i32> = Vec::new();

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub responses: Vec<Msg>,
    pub status: LogStatus,
    pub msg: Msg,
    pub pre_accept_count: usize,
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
            pre_accept_count: 0,
            leader_responded: false,
            dfn: -1,
            low: -1,
        }
    }
}

pub struct Peer {
    id: i32,
    ballot: i32,
    thrifty: bool,
    preferred_peer: Vec<i32>,
    // fast quorum size
    fast_quorom_size: usize,
    paxos_quorum_size: usize,
    test_msg_count_per_server: usize,
    peer_num: usize,
    deps: HashMap<String, Vec<i32>>,
    commited_up_to: Vec<i32>,
    crt_instance: Vec<i32>,
    instance: i32,
    sender_to_self: UnboundedSender<PeerMsg>,
    receiver: UnboundedReceiver<PeerMsg>,
    // used to init peer rpc
    peer_addrs: HashMap<i32, String>,
    propose_addr: String,
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
    ) -> Self {
        Self {
            id,
            ballot: 1,
            thrifty,
            preferred_peer: config.preferred_peer[id as usize].clone(),
            fast_quorom_size: config.fast_quorum_size,
            paxos_quorum_size: config.paxos_quorum_size,
            peer_num: config.peer_num,
            deps: HashMap::new(),
            test_msg_count_per_server: config.test_msg_count_per_server,
            commited_up_to: vec![0; config.peer_num],
            crt_instance: vec![0; config.peer_num],
            instance: 0,
            sender_to_self: sender,
            receiver,
            peer_addrs: config.server_addrs,
            propose_addr: config.propose_server_addrs.get(&id).unwrap().clone(),
            peer_senders: HashMap::new(),
            client_senders,
            execute,
            batch_size,
            batch_request: Vec::new(),
        }
    }

    pub async fn init_and_run(&mut self, receiver: UnboundedReceiver<ClientMsgReply>) {
        // init global logs
        unsafe {
            LOGS = vec![vec![None; self.test_msg_count_per_server]; self.peer_num];

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
                self.peer_num,
                100,
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

    async fn handle_msg(&mut self, msg: Msg) {
        match msg.entry_type() {
            MsgType::PreAccept => self.process_preaccept(msg).await,
            MsgType::PreAcceptReply => self.process_preaccept_response(msg).await,
            MsgType::Accept => self.process_accept(msg).await,
            MsgType::AcceptReply => self.process_accept_response(msg).await,
            MsgType::Commit => self.process_commit(msg),
            MsgType::Prepare => todo!(),
            MsgType::PrepareReply => todo!(),
            MsgType::TyrPreaccept => todo!(),
            MsgType::TryRepacceptReply => todo!(),
        }
    }

    async fn process_client_msg(&mut self, client_msg: ClientMsg) {
        // tracing::info!("handling client msg key = {:?}", client_msg.key);
        // insert to batch request
        self.batch_request.push(client_msg);
        if self.batch_request.len() == self.batch_size {
            let instance = self.instance + 1;
            self.instance += 1;
            self.crt_instance[self.id as usize] += 1;
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
                try_preacept_reply: None,
            };
            // insert to logs
            let log_entry =
                LogEntry::new(Vec::new(), LogStatus::PreAccepted, pre_accept_msg.clone());
            unsafe {
                LOGS[self.id as usize][instance as usize] = Some(log_entry);
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
        if self.crt_instance[replica as usize] < instance {
            self.crt_instance[replica as usize] = instance;
        }
        unsafe {
            let entry_option = LOGS[replica as usize][instance as usize].as_mut();
            match entry_option {
                Some(entry) => {
                    if entry.msg.ballot >= msg.ballot {
                        return;
                    } else {
                        // todo insert to LOGS and reply
                    }
                }
                None => {
                    if instance >= self.crt_instance[replica as usize] {
                        self.crt_instance[replica as usize] = instance + 1;
                    }
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
                    self.send_to_peer(response, to).await;
                }
            }
        }
    }

    async fn process_preaccept_response(&mut self, msg: Msg) {
        unsafe {
            let log_entry = LOGS[msg.replica as usize][msg.instance as usize]
                .as_mut()
                .unwrap();
            log_entry.responses.push(msg.clone());
            let mut conflict = false;
            if log_entry.responses.len() == self.fast_quorom_size - 1 {
                // check if can commit at fast path
                for response in log_entry.responses.iter() {
                    if response.deps != log_entry.msg.deps {
                        conflict = true;
                        for i in 0..self.peer_num {
                            if log_entry.msg.deps[i] < response.deps[i] {
                                log_entry.msg.deps[i] = response.deps[i];
                            }
                        }
                    }
                }
                if conflict {
                    // commit at slow path
                    // println!("conflict send accept");
                    log_entry.status = LogStatus::Accepted;
                    let mut accept_msg = log_entry.msg.clone();
                    accept_msg.entry_type = MsgType::Accept.into();
                    // broadcast accept msg
                    self.broadcast_to_peers(&accept_msg).await;
                } else {
                    // commit at fast path
                    log_entry.status = LogStatus::Commited;
                    let mut commit_msg = log_entry.msg.clone();
                    commit_msg.entry_type = MsgType::Commit.into();
                    // tracing::info!("fast commit msg = {:?}", commit_msg);
                    if !self.execute {
                        for iter in log_entry.msg.commands.iter() {
                            let rsp = ClientMsgReply {
                                command_id: iter.command_id,
                                success: true,
                            };
                            if let Err(e) = self.client_senders.send(rsp) {
                                println!("send client reply channel error {}", e);
                            }
                        }
                    }
                    self.broadcast_to_peers(&commit_msg).await;
                }
            }
        }
    }

    async fn process_accept(&mut self, msg: Msg) {
        if self.crt_instance[msg.replica as usize] < msg.instance {
            self.crt_instance[msg.replica as usize] = msg.instance + 1;
        }
        // update deps

        // just put msg into logs and reply response
        let to = msg.from;
        let replica = msg.replica;
        let instance = msg.instance;
        unsafe {
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
            try_preacept_reply: None,
        };
        self.send_to_peer(response, to).await;
    }

    async fn process_accept_response(&mut self, msg: Msg) {
        //
        unsafe {
            let entry = LOGS[msg.replica as usize][msg.instance as usize]
                .as_mut()
                .unwrap();
            entry.pre_accept_count += 1;
            if entry.pre_accept_count == self.paxos_quorum_size - 1 {
                // commit
                entry.status = LogStatus::Commited;
                let mut commit_msg = entry.msg.clone();
                commit_msg.entry_type = MsgType::Commit.into();
                if !self.execute {
                    for iter in entry.msg.commands.iter() {
                        let rsp = ClientMsgReply {
                            command_id: iter.command_id,
                            success: true,
                        };
                        if let Err(e) = self.client_senders.send(rsp) {
                            println!("send client reply channel error {}", e);
                        }
                    }
                }
                // tracing::info!("slow commit msg = {:?}", commit_msg);
                self.broadcast_to_peers(&commit_msg).await;
            }
        }
    }

    fn process_commit(&mut self, msg: Msg) {
        if self.crt_instance[msg.replica as usize] < msg.instance {
            self.crt_instance[msg.replica as usize] = msg.instance;
        }
        // update deps
        for kv in msg.commands.iter() {
            match self.deps.get_mut(&kv.key) {
                Some(dep) => {
                    // update deps
                    for (peer_id, dep_instance) in dep.iter_mut().enumerate() {
                        let commit_instance = msg.deps.get(peer_id).unwrap();
                        if peer_id != msg.from as usize {
                            if *dep_instance < *commit_instance {
                                *dep_instance = *commit_instance;
                            }
                        } else {
                            if *dep_instance < msg.instance {
                                *dep_instance = msg.instance;
                            }
                        }
                    }
                }
                None => {
                    // insert key with it's deps
                    let mut dep = msg.deps.clone();
                    dep[msg.from as usize] = msg.instance;
                    self.deps.insert(kv.key.clone(), dep);
                }
            }
        }

        // insert to logs
        unsafe {
            let instance = msg.instance;
            let replica = msg.replica;
            let entry = LOGS[replica as usize][instance as usize].as_mut();
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

    async fn handle(&mut self, peer_msg: PeerMsg) {
        // tracing::info!("handle msg {:?}", peer_msg);
        match peer_msg {
            PeerMsg::Msg(msg) => self.handle_msg(msg).await,
            PeerMsg::ClientMsg(msg) => self.process_client_msg(msg).await,
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
        self.peer_senders.get(&to).unwrap().send(msg).await;
    }

    async fn broadcast_to_peers(&self, msg: &Msg) {
        for (_to, sender) in self.peer_senders.iter() {
            let send_msg = msg.clone();
            sender.send(send_msg).await;
        }
    }

    /////////////////////////////////////////////////////////////////////////////////
    /////below are the recovery functions

    async fn recovery(&mut self, replica: i32, instance: i32) {
        // broadcast prepare to all peers
        unsafe {
            match LOGS[replica as usize][instance as usize].as_mut() {
                Some(entry) => {
                    // increase ballot
                    let ballot = entry.msg.ballot + 1;
                    entry.msg.ballot = ballot;
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
                        instance,
                        ballot: 1,
                        deps: Vec::new(),
                        commands: Vec::new(),
                        status: None,
                        ok: None,
                        try_preacept_reply: None,
                    };
                    let entry = LogEntry::new(Vec::new(), LogStatus::PreAccepted, msg);
                    LOGS[replica as usize][instance as usize] = Some(entry);
                }
            }

            // broadcast to all peers
            self.broadcast_to_peers(
                &LOGS[replica as usize][instance as usize]
                    .as_ref()
                    .unwrap()
                    .msg,
            )
            .await;
        }
    }

    async fn handle_prepare(&mut self, msg: Msg) {
        // if have not received a msg at instance
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
                        try_preacept_reply: None,
                    };
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
                        try_preacept_reply: None,
                    };
                    //
                    self.send_to_peer(reply_ok, to).await;
                }
            }
        }
    }

    async fn handle_prepare_response(&mut self, msg: Msg) {
        // wait for at least f + 1 replys
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
                    if msg.status() == LogStatus::Commited || msg.status() == LogStatus::Executed {
                        // insert into logs
                        entry.status = LogStatus::Commited;
                        entry.msg.commands = msg.commands;
                        entry.msg.ballot = msg.ballot;
                        entry.msg.deps = msg.deps;
                        let commit_msg = Msg {
                            entry_type: MsgType::Commit.into(),
                            from: self.id,
                            replica,
                            instance,
                            ballot: msg.ballot,
                            deps: entry.msg.deps.clone(),
                            commands: entry.msg.commands.clone(),
                            status: None,
                            ok: None,
                            try_preacept_reply: None,
                        };
                        self.broadcast_to_peers(&commit_msg).await;
                    } else if msg.status() == LogStatus::Accepted {
                        if entry.msg.commands.len() == 0 || entry.msg.ballot < msg.ballot {
                            entry.msg.status = msg.status;
                            entry.msg.commands = msg.commands;
                            entry.msg.deps = msg.deps;
                        }
                    } else if msg.status() == LogStatus::PreAccepted
                        || msg.status() == LogStatus::PreAcceptedEq
                    {
                        if entry.msg.commands.len() == 0 {
                            entry.msg.status = msg.status;
                            entry.msg.commands = msg.commands;
                            entry.msg.deps = msg.deps;
                        } else if msg.deps == entry.msg.deps {
                            entry.pre_accept_count += 1;
                        } else if msg.status() == LogStatus::PreAcceptedEq {
                            entry.msg.status = msg.status;
                            entry.msg.commands = msg.commands;
                            entry.msg.deps = msg.deps;
                        }
                        if msg.from == msg.replica {
                            // leader reply?
                            entry.leader_responded = true;
                        }
                    }
                    if entry.responses.len() >= self.paxos_quorum_size {
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
                                try_preacept_reply: None,
                            };
                            self.broadcast_to_peers(&accept_msg).await;
                        } else {
                            if entry_msg.status() == LogStatus::Accepted
                                || (!entry.leader_responded
                                    && entry.pre_accept_count >= self.paxos_quorum_size - 1
                                    && entry_msg.status() == LogStatus::PreAcceptedEq)
                            {
                                // saft to go to accept phase
                                entry.status = LogStatus::Accepted;
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
                                    try_preacept_reply: None,
                                };
                                self.broadcast_to_peers(&accept_msg).await;
                            } else if !entry.leader_responded
                                && entry.pre_accept_count >= (self.peer_num / 2 + 1) / 2
                            {
                                // send try pre accept
                            } else {
                                // leader responded, safe to start phase 1
                                entry.status = LogStatus::PreAccepted;
                                let pre_accept_msg = Msg {
                                    entry_type: MsgType::PreAccept.into(),
                                    from: self.id,
                                    replica: entry_msg.replica,
                                    instance: entry_msg.instance,
                                    ballot: entry_msg.ballot,
                                    deps: entry_msg.deps.clone(),
                                    commands: entry_msg.commands.clone(),
                                    status: None,
                                    ok: None,
                                    try_preacept_reply: None,
                                };
                                self.broadcast_to_peers(&pre_accept_msg).await;
                            }
                        }
                    }
                }
                None => return,
            }
        }
    }

    fn find_preaccept_deps(&mut self, msg: &Msg) {
        if let Some(entry) = self.logs[msg.replica as usize].get(&msg.instance) {
            if entry.msg.commands.len() > 0 {
                if entry.status >= LogStatus::Accepted {
                    return;
                }
                if entry.msg.deps == msg.deps {
                    // already preaccepted
                    return;
                }
            }
            for i in 0..(self.peer_num - 1) {
                for instance in self.executed_up_to[i]..self.crt_instance[i] {
                    if instance == msg.instance && i == msg.replica as usize {
                        // no need to check past instance in replica's row
                        break;
                    }
                    if instance == msg.deps[i as usize] {
                        continue;
                    }
                    match self.logs[i as usize].get(&instance) {
                        Some(conflict_entry) => {
                            if conflict_entry.msg.commands.len() == 0 {
                                continue;
                            } else if conflict_entry.msg.deps[msg.replica as usize] >= msg.instance
                            {
                                continue;
                            } else if conflict_commands(&conflict_entry.msg.commands, &msg.commands)
                            {
                                // if find a conflict
                            }
                        }
                        None => {
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn handle_try_preaccept(&mut self, msg: Msg) {
        let replica = msg.replica;
        let instance = msg.instance;
        let to = msg.from;
        match self.logs[replica as usize].get_mut(&instance) {
            Some(entry) => if entry.msg.commands.len() == 0 {},
            None => {
                // insert new entry
            }
        }
    }

    // fn handle_try_preaccept_response(&mut self) {}

    //////////////////////////////////////////////////////////////////////////////////
    //////init helper function
    async fn init_peer_rpc(
        &mut self,
        sender: UnboundedSender<PeerMsg>,
        receiver: UnboundedReceiver<ClientMsgReply>,
    ) -> Result<()> {
        // init peer rpc
        let mut listen_ip = self.peer_addrs.get(&self.id).unwrap().clone();
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
        let propose_ip = convert_ip_addr(self.propose_addr.clone(), false);
        let propose_server = ProposeServer::new(propose_ip, sender, receiver);
        tokio::spawn(async move {
            run_propose_server(propose_server).await;
        });
        Ok(())
    }
}
