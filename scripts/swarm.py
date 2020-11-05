#!/usr/bin/env python3

import subprocess
import os
import atexit
import networkx as nx


G = nx.ladder_graph(3)

procs = []
for node, adj in G.adjacency():
    port = 7000 + node
    peers = map(lambda port: '127.0.0.1:{}'.format(7000 + port), adj)
    print('tokio::spawn(comm_task({}, &[{}], false));'.format(port, ', '.join(map(lambda p: f'"{p}"', peers))))
    # p = subprocess.Popen(['sh', '-c', 'target/debug/poe_core --port {} --peers {}'.format(port, ','.join(peers))])
    # procs.append(p)

exit()

@atexit.register
def cleanup():
    print("Cleanup started!")
    for p in procs:
        p.terminate()
        p.wait()
    print("Cleanup done!")


for p in procs:
    p.wait()
