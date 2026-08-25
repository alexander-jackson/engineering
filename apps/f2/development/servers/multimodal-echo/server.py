import socket
import threading
from flask import Flask

app = Flask(__name__)


@app.route("/<content>")
def echo(content):
    return f"Echo {content}", 200


def handle(conn):
    with conn:
        while chunk := conn.recv(4096):
            conn.sendall(chunk)


def run_tcp():
    with socket.socket() as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(("0.0.0.0", 8080))
        s.listen()
        while True:
            conn, _ = s.accept()
            threading.Thread(target=handle, args=(conn,), daemon=True).start()


threading.Thread(target=run_tcp, daemon=True).start()
app.run(host="0.0.0.0", port=8081)
