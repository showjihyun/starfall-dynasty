#!/usr/bin/env python
"""원문 프레임 몇 개를 **표준 라이브러리만으로** 보낸다 (qa 소유).

왜 있는가: 측정 중에는 빌드를 하지 않는다(계약 G-c). `tools/bots` 에 없는 한 건짜리 관찰
(예: 퇴화 쿼터니언 `aim_* = 0` → `aim_degenerate_total`, SC-33 / I-39)을 봇을 고치지 않고 만들기 위해서다.
판정하지 않는다 — 보낸 것과 받은 `COMMAND_RESULT` 를 그대로 찍는다.

    python tests/e2e/raw_ws_send.py --token <bots token> --frame '{"command_id":"...", ...}' [--frame ...]

`command_id` 가 `"@new"` 면 새 UUIDv7 비슷한 값(시간 앞자리 + 난수)으로 바꾼다.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import socket
import struct
import sys
import time
import uuid

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    except (AttributeError, ValueError):
        pass


def uuid7() -> str:
    ms = int(time.time() * 1000)
    rand = int.from_bytes(os.urandom(10), "big")
    v = (ms << 80) | (0x7 << 76) | ((rand >> 64) & 0xFFF) << 64 | (0b10 << 62) | (rand & ((1 << 62) - 1))
    return str(uuid.UUID(int=v))


def send_frame(sock: socket.socket, payload: bytes, opcode: int = 0x1) -> None:
    mask = os.urandom(4)
    n = len(payload)
    head = bytes([0x80 | opcode])
    if n < 126:
        head += bytes([0x80 | n])
    elif n < 65536:
        head += bytes([0x80 | 126]) + struct.pack(">H", n)
    else:
        head += bytes([0x80 | 127]) + struct.pack(">Q", n)
    sock.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))


def recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("closed")
        buf += chunk
    return buf


def recv_frame(sock: socket.socket) -> tuple[int, bytes]:
    b0, b1 = recv_exact(sock, 2)
    n = b1 & 0x7F
    if n == 126:
        n = struct.unpack(">H", recv_exact(sock, 2))[0]
    elif n == 127:
        n = struct.unpack(">Q", recv_exact(sock, 8))[0]
    if b1 & 0x80:
        mask = recv_exact(sock, 4)
        data = bytes(b ^ mask[i % 4] for i, b in enumerate(recv_exact(sock, n)))
    else:
        data = recv_exact(sock, n)
    return b0 & 0x0F, data


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--addr", default="127.0.0.1:8080")
    ap.add_argument("--token", required=True)
    ap.add_argument("--frame", action="append", required=True)
    ap.add_argument("--listen", type=float, default=2.0, help="보낸 뒤 받는 시간(초)")
    ap.add_argument("--burst", action="store_true",
                    help="프레임 사이에 기다리지 않고 한 번에 보낸다 — 같은 tick 도착을 노린다(SC-31 (5,3))")
    ap.add_argument("--show-ack", action="store_true", help="스냅샷의 ack_input_seq 변화를 찍는다")
    args = ap.parse_args()
    host, port = args.addr.split(":")
    sock = socket.create_connection((host, int(port)), timeout=10)
    key = base64.b64encode(os.urandom(16)).decode()
    sock.sendall((
        f"GET /ws HTTP/1.1\r\nHost: {args.addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
        f"Authorization: Bearer {args.token}\r\n\r\n").encode())
    resp = b""
    while b"\r\n\r\n" not in resp:
        resp += sock.recv(4096)
    status = resp.split(b"\r\n", 1)[0].decode()
    print(f"handshake: {status}")
    if " 101 " not in status:
        return 3
    sock.settimeout(0.2)
    state: dict = {"ack": "unset"}

    def pump(seconds: float, want_ready: bool = False) -> bool:
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                op, data = recv_frame(sock)
            except socket.timeout:
                continue
            except ConnectionError:
                print("connection closed by server")
                return False
            if op == 0x9:
                send_frame(sock, data, 0xA)
            elif op == 0x8:
                print(f"close frame: {data[:2].hex()} {data[2:].decode(errors='replace')}")
                return False
            elif op == 0x1:
                msg = json.loads(data)
                t = msg.get("message_type")
                if t == "WORLD_SNAPSHOT":
                    ack = msg.get("payload", {}).get("ack_input_seq")
                    if args.show_ack and ack != state.get("ack"):
                        print(f"snapshot tick={msg.get('tick')} ack_input_seq={ack}")
                        state["ack"] = ack
                    continue
                print(f"recv {t} tick={msg.get('tick')} payload={json.dumps(msg.get('payload'), ensure_ascii=False)}")
                if want_ready and t == "SESSION_READY":
                    return True
        return not want_ready

    if not pump(10.0, want_ready=True):
        print("SESSION_READY 없음")
        return 1
    for raw in args.frame:
        obj = json.loads(raw)
        if obj.get("command_id") == "@new":
            obj["command_id"] = uuid7()
        text = json.dumps(obj)
        send_frame(sock, text.encode())
        print(f"sent {text}")
        if not args.burst:
            pump(0.1)
    pump(args.listen)
    send_frame(sock, struct.pack(">H", 1000), 0x8)
    pump(1.0)
    sock.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
