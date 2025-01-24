import os
import socket
import sys

if __name__ == "__main__":
    if len(sys.argv) < 5:
        # ${{ github.sha }} ${{ github.actor }} ${{ github.event.head_commit.message }} ${{ github.run_id }}
        print("Usage: python3 client_test hash author name run_id")
        sys.exit(1)

    conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    conn.connect((os.environ["IP"], int(os.environ["PORT"])))
    conn.send(f"{os.environ['TOKEN']}|build-ours")
    ack = conn.recv(1024).decode()

    conn.send(f"{sys.argv[1]}|{sys.argv[2]}|{sys.argv[3]}|{sys.argv[4]}".encode())

    while True:
        data = conn.recv(1024)
        if not data:
            break
        line = data.decode(errors="ignore")
        if "%*&" in line:
            print(line[:(line.find("%*&"))])
            sys.exit(int(line[(line.find("%*&") + 3):].split("\n")[0]))
        print(line, end = "")
