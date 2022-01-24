-------------------------- MODULE QEPaxos --------------------------

EXTENDS Naturals, Bags, FiniteSets, Sequences, TLC, Integers

-----------------------------------------------------------------------------

(*********************************************************************************)
(* Constant parameters:                                                          *)
(*       Commands: the set of all possible commands                              *)
(*       Replicas: the set of all EPaxos replicas                                *)
(*       FastQuorums(r): the set of all fast quorums where r is a command leader *)
(*       SlowQuorums(r): the set of all slow quorums where r is a command leader *)
(*********************************************************************************)

CONSTANTS Commands, Replicas, FQ_Size

ASSUME IsFiniteSet(Replicas)  

(***************************************************************************)
(* Quorum conditions:                                                      *)
(*  (simplified)                                                           *)
(***************************************************************************)

\* The set of all quorums. This just calculates simple majorities
\*
\*ASSUME \A r \in Replicas:
\*  /\ SlowQuorums(r) \subseteq SUBSET Replicas
\*  /\ \A SQ \in SlowQuorums(r): 
\*    /\ r \in SQ
\*    /\ Cardinality(SQ) = (Cardinality(Replicas) \div 2) + 1
    
    
(***************************************************************************)
(* Special none command                                                    *)
(***************************************************************************)

none == CHOOSE c : c \notin Commands


(***************************************************************************)
(* The instance space                                                      *)
(***************************************************************************)

Instances == Replicas \X (1..Cardinality(Commands))

(***************************************************************************)
(* The possible status of a command in the log of a replica.               *)
(***************************************************************************)

Status == {"not-seen", "pre-accepted", "accepted", "pre-committed", "committed"}


(***************************************************************************)
(* All possible protocol messages:                                         *)
(***************************************************************************)

Message ==
        [type: {"pre-accept"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas,
        cmd: Commands \cup {none}, deps: SUBSET Instances]
  \cup  [type: {"accept"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas,
        cmd: Commands \cup {none}, deps: SUBSET Instances]
  \cup  [type: {"commit"},
        inst: Instances, ballot: Nat \X Replicas,
        cmd: Commands \cup {none}, deps: SUBSET Instances]
  \cup  [type: {"pre-commit"},
        inst: Instances, ballot: Nat \X Replicas,
        cmd: Commands \cup {none}, deps: SUBSET Instances]
  \cup  [type: {"prepare"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas]
  \cup  [type: {"pre-accept-reply"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas,
        deps: SUBSET Instances, committed: SUBSET Instances]
  \cup  [type: {"accept-reply"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas]
  \cup  [type: {"prepare-reply"}, src: Replicas, dst: Replicas,
        inst: Instances, ballot: Nat \X Replicas, prev_ballot: Nat \X Replicas,
        status: Status,
        cmd: Commands \cup {none}, deps: SUBSET Instances]
        
        
 

(*******************************************************************************)
(* Variables:                                                                  *)
(*                                                                             *)
(*          comdLog = the commands log at each replica                         *)
(*          proposed = command that have been proposed                         *)
(*          executed = the log of executed commands at each replica            *)
(*          sentMsg = sent (but not yet received) messages                     *)
(*          crtInst = the next instance available for a command                *)
(*                    leader                                                   *)
(*          leaderOfInst = the set of instances each replica has               *)
(*                         started but not yet finalized                       *)
(*          committed = maps commands to set of commit attributs               *)
(*                      tuples                                                 *)
(*          ballots = largest ballot number used by any                        *)
(*                    replica                                                  *)
(*          preparing = set of instances that each replica is                  *)
(*                      currently preparing (i.e. recovering)                  *) 
(*                                                                             *)
(*                                                                             *)
(*******************************************************************************)

 
VARIABLES cmdLog, proposed, executed, sentMsg, crtInst, leaderOfInst,
          committed, ballots, preparing, quorum

TypeOK ==
    /\ cmdLog \in [Replicas -> SUBSET [inst: Instances, 
                                       status: Status,
                                       ballot: Nat \X Replicas,
                                       cmd: Commands \cup {none},
                                       deps: SUBSET Instances]]
    /\ proposed \in SUBSET Commands
    /\ executed \in [Replicas -> SUBSET (Nat \X Commands)]
\*    /\ sentMsg \in SUBSET Message
    /\ crtInst \in [Replicas -> Nat]
    /\ leaderOfInst \in [Replicas -> SUBSET Instances]
    /\ committed \in [Instances -> SUBSET ((Commands \cup {none}) \X
                                           (SUBSET Instances) \X 
                                           Nat)]
    /\ ballots \in Nat
    /\ preparing \in [Replicas -> SUBSET Instances]
    
vars == << cmdLog, proposed, executed, sentMsg, crtInst, leaderOfInst, 
           committed, ballots, preparing, quorum>>

(***************************************************************************)
(* Initial state predicate                                                 *)
(***************************************************************************)

Init ==
  /\ sentMsg = {}
  /\ cmdLog = [r \in Replicas |-> {}]
  /\ proposed = {}
  /\ executed = [r \in Replicas |-> {}]
  /\ crtInst = [r \in Replicas |-> 1]
  /\ leaderOfInst = [r \in Replicas |-> {}]
  /\ committed = [i \in Instances |-> {}]
  /\ ballots = 0
  /\ preparing = [r \in Replicas |-> {}]
  /\ quorum = Replicas



(***************************************************************************)
(* Actions                                                                 *)
(***************************************************************************)

StartPhase1(C, cleader, inst, ballot, oldMsg) ==                                                
    LET newDeps == {rec.inst: rec \in cmdLog[cleader]} 
        oldRecs == {rec \in cmdLog[cleader] : rec.inst = inst} IN
        /\ cmdLog' = [cmdLog EXCEPT ![cleader] = (@ \ oldRecs) \cup 
                                {[inst   |-> inst,
                                  status |-> "pre-accepted",
                                  ballot |-> ballot,
                                  cmd    |-> C,
                                  deps   |-> newDeps]}]
        /\ leaderOfInst' = [leaderOfInst EXCEPT ![cleader] = @ \cup {inst}]
        /\ sentMsg' = (sentMsg \ oldMsg) \cup 
                                [type  : {"pre-accept"},
                                  src   : {cleader},
                                  dst   : quorum \ {cleader},
                                  inst  : {inst},
                                  ballot: {ballot},
                                  cmd   : {C},
                                  deps  : {newDeps}]
         

Propose(C, cleader) ==
    LET newInst == <<cleader, crtInst[cleader]>> 
        newBallot == <<ballots, cleader>> 
    IN  /\ proposed' = proposed \cup {C}
        /\ StartPhase1(C, cleader, newInst, newBallot, {})
        /\ crtInst' = [crtInst EXCEPT ![cleader] = @ + 1]
        /\ UNCHANGED << executed, committed, ballots, preparing, quorum >>

Phase1Reply(replica) ==
    /\ \E msg \in sentMsg:
        /\ msg.type = "pre-accept"
        /\ msg.dst = replica
        /\ LET oldRec == {rec \in cmdLog[replica]: rec.inst = msg.inst} IN
            /\ (\A rec \in oldRec : 
                (rec.ballot = msg.ballot \/rec.ballot[1] < msg.ballot[1]))
            /\ LET newDeps == msg.deps \cup 
                            ({t.inst: t \in cmdLog[replica]} \ {msg.inst})
                   instCom == {t.inst: t \in {tt \in cmdLog[replica] :
                              tt.status \in {"committed", "pre-committed","executed"}}} IN
\*                /\ Print("try to reply phase 1",TRUE)
                /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ oldRec) \cup
                                    {[inst   |-> msg.inst,
                                      status |-> "pre-accepted",
                                      ballot |-> msg.ballot,
                                      cmd    |-> msg.cmd,
                                      deps   |-> newDeps]}]
                /\ sentMsg' = (sentMsg \ {msg}) \cup
                                    {[type  |-> "pre-accept-reply",
                                      src   |-> replica,
                                      dst   |-> msg.src,
                                      inst  |-> msg.inst,
                                      ballot|-> msg.ballot,
                                      deps  |-> newDeps,
                                      committed|-> instCom]}
                /\ UNCHANGED << proposed, crtInst, executed, leaderOfInst,
                                committed, ballots, preparing, quorum >>
                                        
Phase1Fast(cleader, i) ==
    /\ i \in leaderOfInst[cleader]
    /\ \E record \in cmdLog[cleader]:
        /\ record.inst = i
        /\ record.status = "pre-accepted"
        /\ record.ballot[1] = ballots
        /\ LET replies == {msg \in sentMsg: 
                                /\ msg.inst = i
                                /\ msg.type = "pre-accept-reply"
                                /\ msg.dst = cleader
                                /\ msg.src \in Replicas
                                /\ msg.ballot = record.ballot}
                                IN
            /\ (\A replica \in (Replicas \ {cleader}): 
                    \E msg \in replies: msg.src = replica)
            /\ LET r == CHOOSE r \in replies : TRUE IN
                /\ LET localCom == {t.inst:
                            t \in {tt \in cmdLog[cleader] : 
                                 tt.status \in {"committed", "pre-committed","executed", "acceptted"}}}
                        allDeps == UNION {msg.deps : msg \in replies}
                        sum(x) == {msg \in replies : x \in msg.deps}
                        finalDeps == {inst \in allDeps : Cardinality(sum(inst)) >= FQ_Size}          
                        
                       extCom == UNION {msg.committed: msg \in replies} IN
                       (r.deps \subseteq (localCom \cup extCom))
                /\ cmdLog' = [cmdLog EXCEPT ![cleader] = (@ \ {record}) \cup 
                                        {[inst   |-> i,
                                          status |-> "pre-committed",
                                          ballot |-> record.ballot,
                                          cmd    |-> record.cmd,
                                          deps   |-> r.deps]}]
                /\ sentMsg' = (sentMsg \ replies) \cup
                            {[type  |-> "pre-commit",
                            inst    |-> i,
                            ballot  |-> record.ballot,
                            cmd     |-> record.cmd,
                            deps    |-> r.deps]}
                /\ leaderOfInst' = [leaderOfInst EXCEPT ![cleader] = @ \ {i}]
                /\ committed' = [committed EXCEPT ![i] = 
                                            @ \cup {<<record>>}]
                /\ UNCHANGED << proposed, executed, crtInst, ballots, preparing, quorum >>
                                        
Phase1Slow(cleader, i) ==
    /\ i \in leaderOfInst[cleader]
    /\ \E record \in cmdLog[cleader]:
        /\ record.inst = i
        /\ record.status = "pre-accepted"
        /\ record.ballot[1] = ballots
        /\ LET replies == {msg \in sentMsg: 
                                /\ msg.inst = i 
                                /\ msg.type = "pre-accept-reply" 
                                /\ msg.dst = cleader 
                                /\ msg.src \in quorum
                                /\ msg.ballot = record.ballot} IN
            /\ (\A replica \in (quorum \ {cleader}): \E msg \in replies: msg.src = replica)
            /\ LET finalDeps == UNION {msg.deps : msg \in replies} IN    
                /\ cmdLog' = [cmdLog EXCEPT ![cleader] = (@ \ {record}) \cup 
                                        {[inst   |-> i,
                                          status |-> "accepted",
                                          ballot |-> record.ballot,
                                          cmd    |-> record.cmd,
                                          deps   |-> finalDeps ]}]
                /\ (sentMsg' = (sentMsg \ replies) \cup
                            [type : {"accept"},
                            src : {cleader},
                            dst : quorum \ {cleader},
                            inst : {i},
                            ballot: {record.ballot},
                            cmd : {record.cmd},
                            deps : {finalDeps}])
                /\ UNCHANGED << proposed, executed, crtInst, leaderOfInst,
                                committed, ballots, preparing, quorum >>

AcceptReply(replica) ==
    \E msg \in sentMsg: 
        /\ msg.type = "accept"
        /\ msg.dst = replica
        /\ msg.ballot[1] = ballots
        /\ LET oldRec == {rec \in cmdLog[replica]: rec.inst = msg.inst} IN
            /\ (\A rec \in oldRec: (rec.ballot = msg.ballot \/ 
                                    rec.ballot[1] < msg.ballot[1]))
            /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ oldRec) \cup
                                {[inst   |-> msg.inst,
                                  status |-> "accepted",
                                  ballot |-> msg.ballot,
                                  cmd    |-> msg.cmd,
                                  deps   |-> msg.deps]}]
            /\ sentMsg' = (sentMsg \ {msg}) \cup
                                {[type  |-> "accept-reply",
                                  src   |-> replica,
                                  dst   |-> msg.src,
                                  inst  |-> msg.inst,
                                  ballot|-> msg.ballot]}
            /\ UNCHANGED << proposed, crtInst, executed, leaderOfInst,
                            committed, ballots, preparing, quorum >>

AcceptFinalize(cleader, i) ==
    /\ i \in leaderOfInst[cleader]
    /\ \E record \in cmdLog[cleader]:
        /\ record.inst = i
        /\ record.status = "accepted"
        /\ LET replies == {msg \in sentMsg: 
                                /\ msg.inst = i 
                                /\ msg.type = "accept-reply" 
                                /\ msg.dst = cleader 
                                /\ msg.src \in quorum 
                                /\ msg.ballot = record.ballot} IN
            /\ (\A replica \in (quorum \ {cleader}): \E msg \in replies: 
                                                        msg.src = replica)
            /\ cmdLog' = [cmdLog EXCEPT ![cleader] = (@ \ {record}) \cup 
                                    {[inst   |-> i,
                                      status |-> "committed",
                                      ballot |-> record.ballot,
                                      cmd    |-> record.cmd,
                                      deps   |-> record.deps ]}]
            /\ sentMsg' = (sentMsg \ replies) \cup
                        {[type  |-> "commit",
                        inst    |-> i,
                        ballot  |-> record.ballot,
                        cmd     |-> record.cmd,
                        deps    |-> record.deps]}
            /\ committed' = [committed EXCEPT ![i] = @ \cup 
                               {<<record>>}]
            /\ leaderOfInst' = [leaderOfInst EXCEPT ![cleader] = @ \ {i}]
            /\ UNCHANGED << proposed, executed, crtInst, ballots, preparing, quorum >>                                                                    
                                
Commit(replica, cmsg) ==
    LET oldRec == {rec \in cmdLog[replica] : rec.inst = cmsg.inst} IN
        /\ \A rec \in oldRec : (rec.status \notin {"committed", "executed"} /\ 
                                rec.ballot[1] <= cmsg.ballot[1])
        /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ oldRec) \cup 
                                    {[inst     |-> cmsg.inst,
                                      status   |-> "committed",
                                      ballot   |-> cmsg.ballot,
                                      cmd      |-> cmsg.cmd,
                                      deps     |-> cmsg.deps]}]
        /\ committed' = [committed EXCEPT ![cmsg.inst] = @ \cup 
                               {<<cmsg>>}]
        /\ UNCHANGED << proposed, executed, crtInst, leaderOfInst,
                        sentMsg, ballots, preparing, quorum >>

PreCommit(replica, cmsg) ==
    LET oldRec == {rec \in cmdLog[replica] : rec.inst = cmsg.inst} IN
        /\ \A rec \in oldRec : (rec.status \notin {"committed", "pre-committed", "executed"} /\ 
                                rec.ballot[1] <= cmsg.ballot[1])
        /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ oldRec) \cup 
                                    {[inst     |-> cmsg.inst,
                                      status   |-> "pre-committed",
                                      ballot   |-> cmsg.ballot,
                                      cmd      |-> cmsg.cmd,
                                      deps     |-> cmsg.deps]}]
        /\ committed' = [committed EXCEPT ![cmsg.inst] = @ \cup 
                               {<<cmsg>>}]
        /\ UNCHANGED << proposed, executed, crtInst, leaderOfInst,
                        sentMsg, ballots, preparing, quorum >>                                
                                
                                
                                
(***************************************************************************)
(* Recovery actions                                                        *)
(***************************************************************************)
 
SendPrepare(replica, i) ==
    /\ i \notin leaderOfInst[replica]
    /\ \E rec \in cmdLog[replica] : rec.inst = i
    /\ LET rec == CHOOSE r \in cmdLog[replica] : r.inst = i IN
               /\ rec.status \in {"committed", "executed"}
               /\ sentMsg' = sentMsg \cup
                            [type   : {"prepare"},
                             src    : {replica},
                             dst    : quorum,
                             inst   : {i},
                             cmd    : {rec.cmd},
                             ballot : {<< ballots, replica >>}]
    /\ ballots' = ballots + 1
    /\ preparing' = [preparing EXCEPT ![replica] = @ \cup {i}]
    /\ UNCHANGED << cmdLog, proposed, executed, crtInst,
                    leaderOfInst, committed, quorum >>


ReplyPrepare(replica) ==
    \E msg \in sentMsg : 
        /\ msg.type = "prepare"
        /\ msg.dst = replica
        /\ \/ \E rec \in cmdLog[replica] : 
                /\ rec.inst = msg.inst
                /\ msg.ballot[1] > rec.ballot[1]
                /\ sentMsg' = (sentMsg \ {msg}) \cup
                            {[type  |-> "prepare-reply",
                              src   |-> replica,
                              dst   |-> msg.src,
                              inst  |-> rec.inst,
                              ballot|-> msg.ballot,
                              prev_ballot|-> rec.ballot,
                              status|-> rec.status,
                              cmd   |-> rec.cmd,
                              deps  |-> rec.deps]}
                 /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ {rec}) \cup
                            {[inst  |-> rec.inst,
                              status|-> rec.status,
                              ballot|-> msg.ballot,
                              cmd   |-> rec.cmd,
                              deps  |-> rec.deps]}]
                 /\ IF rec.inst \in leaderOfInst[replica] THEN
                        /\ leaderOfInst' = [leaderOfInst EXCEPT ![replica] = 
                                                                @ \ {rec.inst}]
                        /\ UNCHANGED << proposed, executed, committed,
                                        crtInst, ballots, preparing, quorum >>
                    ELSE UNCHANGED << proposed, executed, committed, crtInst,
                                      ballots, preparing, leaderOfInst, quorum >>
                        
           \/ /\ ~(\E rec \in cmdLog[replica] : rec.inst = msg.inst)
              /\ LET newDeps == msg.deps \cup 
                            ({t.inst: t \in cmdLog[replica]} \ {msg.inst}) IN
                    /\ sentMsg' = (sentMsg \ {msg}) \cup
                            {[type  |-> "prepare-reply",
                              src   |-> replica,
                              dst   |-> msg.src,
                              inst  |-> msg.inst,
                              ballot|-> msg.ballot,
                              prev_ballot|-> << 0, replica >>,
                              status|-> "prepare",
                              cmd   |-> msg.cmd,
                              deps  |-> newDeps]}
                    /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@) \cup
                            {[inst  |-> msg.inst,
                              status|-> msg.status,
                              ballot|-> msg.ballot,
                              cmd   |-> msg.cmd,
                              deps  |-> newDeps]}]
                    /\ UNCHANGED << proposed, executed, committed, crtInst, ballots,
                              leaderOfInst, preparing, quorum >> 

                                


\* handle prepare reply,  
PrepareFinalize(replica, i) ==
    /\ i \in preparing[replica]
    /\ \E rec \in cmdLog[replica] :
       /\ rec.inst = i
       /\ rec.status \notin {"committed", "executed"}
       /\ LET replies == {msg \in sentMsg : 
                        /\ msg.inst = i
                        /\ msg.type = "prepare-reply"
                        /\ msg.dst = replica
                        /\ msg.ballot = rec.ballot} IN 
            /\ (\A rep \in quorum : \E msg \in replies : msg.src = rep)
            /\  \/ \E com \in replies :
                        /\ (com.status \in {"executed"})
                        /\ preparing' = [preparing EXCEPT ![replica] = @ \ {i}]
                        /\ sentMsg' = sentMsg \ replies
                        /\ UNCHANGED << cmdLog, proposed, executed, crtInst, leaderOfInst,
                                        committed, ballots, quorum >>
                \/ /\ ~(\E msg \in replies : msg.status \in {"executed"})
                   /\ \E acc \in replies :
                        /\ \/ acc.status = "accepted"
                           \/ acc.status = "committed"
                        /\ (\A msg \in (replies \ {acc}) : 
                            (msg.prev_ballot[1] <= acc.prev_ballot[1] \/ 
                             msg.status # "accepted"))
                        /\ sentMsg' = (sentMsg \ replies) \cup
                                 [type  : {"accept"},
                                  src   : {replica},
                                  dst   : quorum \ {replica},
                                  inst  : {i},
                                  ballot: {rec.ballot},
                                  cmd   : {acc.cmd},
                                  deps  : {acc.deps}]
                        /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ {rec}) \cup
                                {[inst  |-> i,
                                  status|-> "accepted",
                                  ballot|-> rec.ballot,
                                  cmd   |-> acc.cmd,
                                  deps  |-> acc.deps]}]
                         /\ preparing' = [preparing EXCEPT ![replica] = @ \ {i}]
                         /\ leaderOfInst' = [leaderOfInst EXCEPT ![replica] = @ \cup {i}]
                         /\ UNCHANGED << proposed, executed, crtInst, committed, ballots, quorum >>
                \/ /\ ~(\E msg \in replies : 
                        msg.status \in {"accepted", "committed", "executed"})
                   /\ LET preaccepts == {msg \in replies : msg.status = "pre-accepted" \/ msg.status = "prepared"} IN
\*                        /\ \A p1, p2 \in preaccepts :
\*                                p1.cmd = p2.cmd /\ p1.deps = p2.deps
\*                        /\ ~(\E pl \in preaccepts : pl.src = i[1])
                        /\ LET allDeps == UNION {msg.deps : msg \in replies} 
                                  sum(x) == {msg \in replies : x \in msg.deps}
                                  finalDeps == {inst \in allDeps : Cardinality(sum(inst)) >= (BagCardinality(quorum) \div 2)} IN
                            \/ /\ \E inst \in allDeps : inst[2] \in quorum /\ cmdLog[inst[2]][inst[1]].status /= "committed"
                               /\ UNCHANGED << cmdLog, proposed, executed, sentMsg, crtInst, leaderOfInst, committed, ballots, preparing, quorum>>
                            \/ LET pac == CHOOSE pac \in preaccepts : TRUE IN
                               /\ sentMsg' = (sentMsg \ replies) \cup
                                         [type  : {"accept"},
                                          src   : {replica},
                                          dst   : quorum \ {replica},
                                          inst  : {i},
                                          ballot: {rec.ballot},
                                          cmd   : {pac.cmd},
                                          deps  : {pac.deps}]
                               /\ cmdLog' = [cmdLog EXCEPT ![replica] = (@ \ {rec}) \cup
                                        {[inst  |-> i,
                                          status|-> "accepted",
                                          ballot|-> rec.ballot,
                                          cmd   |-> pac.cmd,
                                          deps  |-> pac.deps]}]
                               /\ preparing' = [preparing EXCEPT ![replica] = @ \ {i}]
                               /\ leaderOfInst' = [leaderOfInst EXCEPT ![replica] = @ \cup {i}]
                               /\ UNCHANGED << proposed, executed, crtInst, committed, ballots, quorum >>
                         
                \/  /\ \A msg \in replies : msg.status = "not-seen"
                    /\ StartPhase1(none, replica, i, rec.ballot, replies)
                    /\ preparing' = [preparing EXCEPT ![replica] = @ \ {i}]
                    /\ UNCHANGED << proposed, executed, crtInst, committed, ballots, quorum >>
                                


(***************************************************************************)
(* Action groups                                                           *)
(***************************************************************************)        

CommandLeaderAction ==
    \/ \E C \in (Commands \ proposed) :
            \E cleader \in Replicas : Propose(C, cleader)
    \/ \E cleader \in Replicas : \E inst \in leaderOfInst[cleader] :
            \/ Phase1Fast(cleader, inst) /\ Cardinality(quorum) = Cardinality(Replicas)
            \/ Phase1Slow(cleader, inst) /\ Cardinality(quorum) < Cardinality(Replicas)
            \/ AcceptFinalize(cleader, inst)
            
ReplicaAction ==
    \E replica \in Replicas :
         \/ Phase1Reply(replica)
         \/ AcceptReply(replica)
         \/ \E cmsg \in sentMsg : (cmsg.type = "pre-commit" /\ PreCommit(replica, cmsg))
         \/ \E cmsg \in sentMsg : (cmsg.type = "commit" /\ Commit(replica, cmsg))
         \/ \E i \in Instances : 
            /\ crtInst[i[1]] > i[2] (* This condition states that the instance has *) 
                                    (* been started by its original owner          *)
            /\ SendPrepare(replica, i)
         \/ ReplyPrepare(replica)

Fail(replica) == /\ replica \in quorum
                    /\ Cardinality(quorum) > (Cardinality(Replicas) \div 2) + 2
                    /\ quorum' = quorum \ {replica}
                    /\ ballots' = ballots + 1  
                    /\ UNCHANGED << cmdLog, proposed, executed, sentMsg, crtInst, leaderOfInst, 
                                        committed, preparing>>

Restart(replica) == /\ replica \notin quorum
                    /\ quorum' = quorum \cup {replica}
                    /\ ballots' = ballots + 1
                    /\ UNCHANGED << cmdLog, proposed, executed, sentMsg, crtInst, leaderOfInst, 
                                        committed, preparing>>
      


(***************************************************************************)
(* Next action                                                             *)
(***************************************************************************)

Next == 
    \/ CommandLeaderAction
    \/ ReplicaAction
    \/ \E i \in Replicas : Fail(i)
    \/ \E i \in Replicas : Restart(i)


(***************************************************************************)
(* The complete definition of the algorithm                                *)
(***************************************************************************)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* Theorems                                                                *)
(***************************************************************************)

Nontriviality ==
    \A i \in Instances :
        [](\A C \in committed[i] : C \in proposed \/ C = none)

Stability ==
    \A replica \in Replicas :
        \A i \in Instances :
            \A C \in Commands :
                []((\E rec1 \in cmdLog[replica] :
                    /\ rec1.inst = i
                    /\ rec1.cmd = C
                    /\ rec1.status \in {"committed", "executed"}) =>
                    [](\E rec2 \in cmdLog[replica] :
                        /\ rec2.inst = i
                        /\ rec2.cmd = C
                        /\ rec2.status \in {"committed", "executed"}))

Consistency ==
    \A i \in Instances :
        LET x == {msg \in committed[i] : \A dep \in msg.deps : committed[dep].ballot <= msg.ballot} IN
        [](Cardinality(x) <= 1)

THEOREM Spec => ([]TypeOK) /\ Nontriviality /\ Stability /\ Consistency
=============================================================================