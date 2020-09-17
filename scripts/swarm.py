#!/usr/bin/env python3

import subprocess
import os
import atexit
import networkx as nx


G = nx.ladder_graph(10)

procs = []
for node, adj in G.adjacency():
    port = 7000 + node
    peers = ','.join(map(lambda port: '127.0.0.1:{}'.format(7000 + port), adj))
    if node == 0:
        peers += ',127.0.0.1:6999'
    print(port, peers)
    p = subprocess.Popen(['sh', '-c', 'target/debug/poe_core --port {} --peers {}'.format(port, peers)])
    procs.append(p)


@atexit.register
def cleanup():
    print("Cleanup started!")
    for p in procs:
        p.terminate()
        p.wait()
    print("Cleanup done!")


for p in procs:
    p.wait()
