use std::time::Duration;
use std::{cmp::Ordering, thread::sleep};

use common::Instance;
use tokio::sync::mpsc::UnboundedSender;

use crate::peer::{CRT_INSTANCE, EXECUTED_UP_TO, LOGS};
use crate::PeerMsg;
use rpc::qepaxos_rpc::{ClientMsgReply, LogStatus};
use tracing;

fn check_safe(replica: i32, instance: usize) -> bool {
    unsafe {
        let entry = LOGS[replica as usize][instance].as_ref().unwrap();
        let ballot = entry.msg.ballot;
        let mut dep_replica = 0;
        for dep in entry.msg.deps.iter() {
            if let Some(dep_entry) = LOGS[dep_replica][*dep as usize].as_ref() {
                if dep_entry.status == LogStatus::Commited
                    || dep_entry.status == LogStatus::PreCommited
                {
                    let dep_ballot = dep_entry.msg.ballot;
                    if dep_ballot >> 8 <= ballot >> 8 {
                        continue;
                    } else {
                        return false;
                    }
                }
            } else {
                return false;
            }
            dep_replica += 1;
        }

        return true;
    }
}
pub struct ExecEngine {
    sender: UnboundedSender<ClientMsgReply>,
    sender_to_peer: UnboundedSender<PeerMsg>,
    replica_num: usize,
    timeout_count: usize,
    id: usize,
}

impl ExecEngine {
    pub fn new(
        sender: UnboundedSender<ClientMsgReply>,
        sender_to_peer: UnboundedSender<PeerMsg>,
        replica_num: usize,
        timeout_count: usize,
        id: usize,
    ) -> Self {
        Self {
            sender,
            sender_to_peer,
            replica_num,
            timeout_count,
            id,
        }
    }

    pub fn run(&mut self) {
        tracing::info!("exec engine start running");
        let mut problem_instance = vec![-1; self.replica_num];
        let mut timeout = vec![0; self.replica_num];
        loop {
            let mut executed = false;
            unsafe {
                for replica in 0..self.replica_num {
                    for instance in EXECUTED_UP_TO[replica]..CRT_INSTANCE[replica] {
                        match LOGS[replica][instance as usize].as_ref() {
                            Some(entry) => {
                                if entry.status == LogStatus::Executed {
                                    EXECUTED_UP_TO[replica] = instance + 1;
                                    continue;
                                } else if entry.status != LogStatus::Commited
                                    && entry.status != LogStatus::PreCommited
                                {
                                    if instance == problem_instance[replica] {
                                        timeout[replica] += 1;
                                        println!("timeout {}-{}", replica, instance);
                                        if timeout[replica] == self.timeout_count {
                                            // todo: notify replica to start recovey, maybe using channel
                                            if instance < CRT_INSTANCE[replica] {
                                                let timeout_instance = Instance {
                                                    replica: replica as i32,
                                                    instance: instance as usize,
                                                    ballot: entry.msg.ballot,
                                                };
                                                let msg = PeerMsg::TimeOut(timeout_instance);
                                                tracing::info!(
                                                    "sending timeout {}-{}",
                                                    replica,
                                                    instance
                                                );
                                                tracing::info!("status {:?}", entry.status);
                                                self.sender_to_peer.send(msg);
                                            }
                                            timeout[replica] = 0;
                                        }
                                    } else {
                                        problem_instance[replica] = instance;
                                        timeout[replica] = 0;
                                    }
                                } else {
                                    self.exec_command(replica, instance);
                                    executed = true;
                                }
                            }
                            None => {
                                if instance == problem_instance[replica] {
                                    timeout[replica] += 1;
                                    if timeout[replica] == self.timeout_count {
                                        // todo: notify replica to start recovey, maybe using channel
                                        if instance < CRT_INSTANCE[replica] {
                                            let timeout_instance = Instance {
                                                replica: replica as i32,
                                                instance: instance as usize,
                                                ballot: 0,
                                            };
                                            let msg = PeerMsg::TimeOut(timeout_instance);
                                            tracing::info!(
                                                "sending timeout {}-{}",
                                                replica,
                                                instance
                                            );
                                            self.sender_to_peer.send(msg);
                                        }
                                        timeout[replica] = 0;
                                    }
                                } else {
                                    problem_instance[replica] = instance;
                                    timeout[replica] = 0;
                                }
                            }
                        }
                    }
                }
            }

            if !executed {
                sleep(Duration::from_micros(10));
            }
        }
    }

    fn exec_command(&mut self, replica: usize, instance: i32) -> bool {
        unsafe {
            match LOGS[replica][instance as usize].as_ref() {
                Some(entry) => {
                    if entry.status == LogStatus::Executed {
                        return true;
                    } else if entry.status != LogStatus::Commited
                        && entry.status != LogStatus::PreCommited
                    {
                        return false;
                    }
                    return self.find_scc(replica, instance);
                }
                None => return false,
            }
        }
    }

    fn find_scc(&mut self, exec_replica: usize, exec_instance: i32) -> bool {
        let mut index = 0;
        let mut visit: i32 = 0;
        let mut stack: Vec<(usize, i32)> = Vec::new();
        stack.push((exec_replica, exec_instance));
        unsafe {
            while visit >= 0 {
                // get
                // tracing::info!("visit = {}", visit);
                let (replica_to_process, instance_to_process) = stack[visit as usize];
                let log_entry = LOGS[replica_to_process][instance_to_process as usize]
                    .as_mut()
                    .unwrap();
                if log_entry.low < 0 {
                    index += 1;
                    log_entry.dfn = index;
                    log_entry.low = index;
                    // let log_entry_deps = &log_entry.msg.deps;
                    for (replica, dep) in log_entry.msg.deps.iter().enumerate() {
                        // tracing::info!("dep = {}", *dep);
                        if *dep == 0 {
                            continue;
                        }
                        loop {
                            match LOGS[replica][*dep as usize] {
                                Some(_) => break,
                                None => sleep(Duration::from_nanos(10)),
                            }
                        }
                        let next = LOGS[replica][*dep as usize].as_mut().unwrap();
                        if next.status == LogStatus::Executed {
                            continue;
                        } else if next.status != LogStatus::Commited
                            && next.status != LogStatus::PreCommited
                        {
                            loop {
                                sleep(Duration::from_nanos(10));
                                if next.status == LogStatus::Commited
                                    || next.status == LogStatus::PreCommited
                                {
                                    break;
                                }
                            }
                        }
                        // check if next in the stack
                        if next.dfn < 0 {
                            // not in the stack
                            stack.push((replica, *dep));
                            visit += 1;
                        } else {
                            if log_entry.low > next.dfn {
                                log_entry.low = next.dfn;
                            }
                        }
                    }
                } else {
                    // get scc, pop & exec
                    if log_entry.dfn == log_entry.low {
                        let mut to_execute: Vec<(usize, i32)> = Vec::new();
                        loop {
                            let (stack_replica, stack_instance) = stack.pop().unwrap();
                            to_execute.push((stack_replica, stack_instance));
                            // tracing::info!(
                            //     "to execute push replica{} instance{}",
                            //     stack_replica, stack_instance
                            // );
                            visit -= 1;
                            // tracing::info!("exec visit = {}", visit);
                            // tracing::info!(
                            //     "stack pop with exec_replica{}, stack replica{} stack instance{}",
                            //     exec_replica,
                            //     stack_replica,
                            //     stack_instance
                            // );
                            if stack_instance == exec_instance && stack_replica == exec_replica {
                                break;
                            }
                        }
                        // sort to execute,
                        to_execute.sort_by(|x, y| {
                            if x.0 < y.0 {
                                Ordering::Greater
                            } else if x.1 < y.1 {
                                Ordering::Less
                            } else {
                                Ordering::Greater
                            }
                        });
                        // to execute & update executed_up_to
                        self.apply(to_execute);
                    } else {
                        visit -= 1;
                    }
                }
            }
            return true;
        }
    }

    // fn scc(
    //     &mut self,
    //     index: &mut i32,
    //     replica: usize,
    //     instance: i32,
    //     stack: &mut Vec<(usize, i32)>,
    // ) -> bool {
    //     // tracing::info!("find scc replica{} instance{}", replica, instance);
    //     unsafe {
    //         *index += 1;
    //         let log_entry = LOGS[replica][instance as usize].as_mut().unwrap();
    //         log_entry.dfn = *index;
    //         log_entry.low = *index;
    //         let log_entry_deps = &log_entry.msg.deps;
    //         for (replica, dep) in log_entry_deps.iter().enumerate() {
    //             for i in EXECUTED_UP_TO[replica]..(*dep + 1) {
    //                 loop {
    //                     match LOGS[replica][i as usize] {
    //                         Some(_) => break,
    //                         None => sleep(Duration::from_millis(10)),
    //                     }
    //                 }
    //                 let next = LOGS[replica][i as usize].as_mut().unwrap();
    //                 if next.status == LogStatus::Executed {
    //                     continue;
    //                 } else if next.status != LogStatus::Commited {
    //                     loop {
    //                         sleep(Duration::from_millis(10));
    //                         if next.status == LogStatus::Commited {
    //                             break;
    //                         }
    //                     }
    //                 }
    //                 // check if next in the stack
    //                 if next.dfn < 0 {
    //                     // not in the stack
    //                     stack.push((replica, i));
    //                     self.scc(index, replica, i, stack);
    //                     if log_entry.low < next.low {
    //                         log_entry.low = next.low;
    //                     }
    //                 } else {
    //                     if log_entry.low > next.dfn {
    //                         log_entry.low = next.dfn;
    //                     }
    //                 }
    //             }
    //         }

    //         // if dfn == low, pop & exec
    //         //let log_entry = &mut self.logs[replica][instance as usize];
    //         if log_entry.dfn == log_entry.low {
    //             let mut to_execute: Vec<(usize, i32)> = Vec::new();
    //             loop {
    //                 let (stack_replica, stack_instance) = stack.pop().unwrap();
    //                 to_execute.push((stack_replica, stack_instance));
    //                 // tracing::info!(
    //                 //     "stack pop with replica{}, stack replica{}",
    //                 //     replica,
    //                 //     stack_replica
    //                 // );
    //                 // tracing::info!(
    //                 //     "stack pop with instance{}, stack instance{}",
    //                 //     instance,
    //                 //     stack_instance
    //                 // );
    //                 if stack_instance == instance && stack_replica == replica {
    //                     break;
    //                 }
    //             }
    //             // sort to execute,
    //             to_execute.sort_by(|x, y| {
    //                 if x.0 < y.0 {
    //                     Ordering::Greater
    //                 } else if x.1 < y.1 {
    //                     Ordering::Less
    //                 } else {
    //                     Ordering::Greater
    //                 }
    //             });
    //             // to execute & update executed_up_to
    //             self.apply(to_execute);
    //         }
    //         true
    //     }
    // }

    fn apply(&mut self, msgs: Vec<(usize, i32)>) {
        unsafe {
            for (replica, instance) in msgs {
                LOGS[replica][instance as usize].as_mut().unwrap().status = LogStatus::Executed;
                // update Execute up to
                let mut executed = EXECUTED_UP_TO[replica];
                while executed <= instance {
                    if LOGS[replica][executed as usize].as_ref().unwrap().status
                        == LogStatus::Executed
                    {
                        EXECUTED_UP_TO[replica] = executed + 1;
                    } else {
                        break;
                    }
                    executed += 1;
                }
                // send back to client

                // tracing::info!("executed {}-{}", replica, instance);
                if replica == self.id {
                    for iter in LOGS[replica][instance as usize]
                        .as_ref()
                        .unwrap()
                        .msg
                        .commands
                        .iter()
                    {
                        let rsp = ClientMsgReply {
                            command_id: iter.command_id,
                            success: true,
                        };
                        if let Err(e) = self.sender.send(rsp) {
                            tracing::info!("send client reply channel error {}", e);
                        }
                    }
                }
            }
        }
    }
}
