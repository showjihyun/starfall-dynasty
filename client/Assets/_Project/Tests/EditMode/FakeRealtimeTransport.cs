// A transport with no socket, so the dispatch, pending-command and reconnect logic can be
// tested without a server and without waiting on real time.
//
// It lives in the test assembly on purpose: shipping a fake in Starfall.Net would invite
// production code to branch on it.

using System;
using System.Collections.Generic;

namespace Starfall.Tests.EditMode
{
    using Starfall.Net;

    public sealed class FakeRealtimeTransport : IRealtimeTransport
    {
        readonly List<Action> _queued = new List<Action>();

        public TransportState State { get; private set; } = TransportState.Disconnected;

        public event Action Opened;
        public event Action<string> MessageReceived;
        public event Action<DisconnectInfo> Disconnected;

        /// <summary>Every frame Send was asked to write, in order.</summary>
        public List<string> Sent { get; } = new List<string>();

        /// <summary>How many times Connect was called, including reconnects.</summary>
        public int ConnectCount { get; private set; }

        public string LastUrl { get; private set; }
        public string LastToken { get; private set; }
        public string LastCloseReason { get; private set; }
        public int CloseBlockingCalls { get; private set; }
        public bool Disposed { get; private set; }

        /// <summary>Set to make Send report a full queue.</summary>
        public bool RefuseSend { get; set; }

        public void Connect(string url, string bearerToken)
        {
            ConnectCount++;
            LastUrl = url;
            LastToken = bearerToken;
            State = TransportState.Connecting;
        }

        public bool Send(string message)
        {
            if (State != TransportState.Open || RefuseSend) return false;
            Sent.Add(message);
            return true;
        }

        public void Close(string reason)
        {
            LastCloseReason = reason;
            CompleteOpenClose(new DisconnectInfo(DisconnectKind.Local, 1000, reason));
        }

        public bool CloseBlocking(string reason, int timeoutMs)
        {
            CloseBlockingCalls++;
            Close(reason);
            return true;
        }

        public void Pump()
        {
            // Copy first: a handler may queue more work, and that belongs to the next pump -
            // the real transport behaves the same way.
            Action[] batch = _queued.ToArray();
            _queued.Clear();
            for (int i = 0; i < batch.Length; i++) batch[i]();
        }

        public void Dispose() { Disposed = true; }

        // -------------------------------------------------------------- test driving

        /// <summary>Simulates the socket coming up. Delivered on the next Pump.</summary>
        public void SimulateOpen()
        {
            State = TransportState.Open;
            _queued.Add(() => { Action handler = Opened; if (handler != null) handler(); });
        }

        /// <summary>Simulates one inbound frame. Delivered on the next Pump.</summary>
        public void SimulateMessage(string json)
        {
            _queued.Add(() => { Action<string> handler = MessageReceived; if (handler != null) handler(json); });
        }

        /// <summary>Simulates the connection ending. Delivered on the next Pump.</summary>
        public void SimulateDisconnect(DisconnectKind kind, int? closeCode, string detail)
        {
            State = TransportState.Disconnected;
            CompleteOpenClose(new DisconnectInfo(kind, closeCode, detail));
        }

        void CompleteOpenClose(DisconnectInfo info)
        {
            State = TransportState.Disconnected;
            _queued.Add(() => { Action<DisconnectInfo> handler = Disconnected; if (handler != null) handler(info); });
        }
    }
}
