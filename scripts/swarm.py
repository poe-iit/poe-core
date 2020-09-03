#!/usr/bin/env python3

import subprocess
import os

# an array of valid ports
ports = []
for i in range(0, 10):
    ports.append(8000 + i)


pairs = []
for port in ports:
    peers = []
    for peer in ports:
        if peer == port:
            continue
        peers.append(peer)
    pairs.append((port, peers))


procs = []
try:
    for pair in pairs:
        peers = ','.join(map(lambda port: '127.0.0.1:{}'.format(port), pair[1]))
        p = subprocess.Popen(['sh', '-c', 'target/debug/poe_core --port {} --peers {}'.format(pair[0], peers)])
        procs.append(p)
    for p in procs:
        # p.join()
        pass
except KeyboardInterrupt:
    for proc in procs:
        proc.terminate()
