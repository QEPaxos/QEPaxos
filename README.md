# SEPaxos
Leaderless Consensus in 1 RTT 


server参数
1、replica_id 
2、server_type: raft, epaxos, sepaxos
3、batch_size: default 1
4、replica_num 3 or 5
5、thrifty bool
6、wide_area bool

throuthput
参数：
1、msg_count
2、conflict
3、repica_num 3 or 5

latency client
参数：
1: replica_id to connect
2: replica_nums
3: server_type: epaxos, sepaxos

wide area latency client
参数：
1: replica_id to connect
2: replica_nums
3: server_type: epaxos, sepaxos