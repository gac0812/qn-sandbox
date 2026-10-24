import socket, os

# AF_UNIX availability probe under nono --block-net (spike1, E2)
p = "/tmp/ux.sock"
try:
    os.unlink(p)
except FileNotFoundError:
    pass

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(p)
server.listen(1)

client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
client.connect(p)
conn, _ = server.accept()
client.send(b"ping")
data = conn.recv(4)
print("UNIX_SOCKET_OK" if data == b"ping" else "MISMATCH")
