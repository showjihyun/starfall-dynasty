// PC (Editor and Standalone) realtime transport on System.Net.WebSockets.ClientWebSocket.
//
// This file is the only place in the client that knows about sockets, Tasks or
// CancellationTokens. It calls no Unity API at all - not even Debug.Log - because most of it
// runs on a background task, and Unity API from a background thread is undefined behaviour
// that usually looks like "the Editor froze". Diagnostics leave through ILogSink, which the
// owner drains on the main thread.
//
// WebGL: ClientWebSocket does not exist there and the browser cannot set an Authorization
// header (ADR-0005 section 6). A WebGL implementation is a sibling of this class, not an
// edit to it.

using System;
using System.Collections.Concurrent;
using System.IO;
using System.Net.WebSockets;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace Starfall.Net
{
    public sealed class PcWebSocketTransport : IRealtimeTransport
    {
        /// <summary>Inbound frame ceiling, mirroring the server's (ADR-0005 section 2). The
        /// server enforces it for its own protection; we enforce it so a server bug cannot
        /// make this client allocate without bound.</summary>
        public const int MaxFrameBytes = 16 * 1024;

        /// <summary>Outbound queue depth. Past this the caller is producing faster than the
        /// socket drains, and silently growing the queue would hide it.</summary>
        public const int MaxOutboundQueue = 256;

        readonly ILogSink _log;
        readonly ConcurrentQueue<Action> _mainThread = new ConcurrentQueue<Action>();
        readonly ConcurrentQueue<string> _outbound = new ConcurrentQueue<string>();
        readonly SemaphoreSlim _outboundSignal = new SemaphoreSlim(0);

        // Reused across every frame of a connection: the receive path must not allocate a
        // buffer per frame (spec section 8).
        readonly byte[] _receiveBuffer = new byte[8 * 1024];
        readonly MemoryStream _assembly = new MemoryStream(MaxFrameBytes);

        ClientWebSocket _socket;
        CancellationTokenSource _cancellation;
        int _disconnectReported;
        volatile TransportState _state = TransportState.Disconnected;

        public PcWebSocketTransport(ILogSink log)
        {
            _log = log ?? NullLogSink.Instance;
        }

        public TransportState State => _state;

        public event Action Opened;
        public event Action<string> MessageReceived;
        public event Action<DisconnectInfo> Disconnected;

        public void Connect(string url, string bearerToken)
        {
            if (string.IsNullOrEmpty(url)) throw new ArgumentException("url is required", nameof(url));
            if (_state != TransportState.Disconnected)
                throw new InvalidOperationException("Connect called while " + _state + ". One transport, one connection at a time.");

            _state = TransportState.Connecting;
            Interlocked.Exchange(ref _disconnectReported, 0);
            DrainOutbound();

            _cancellation = new CancellationTokenSource();
            var socket = new ClientWebSocket();
            _socket = socket;

            if (!string.IsNullOrEmpty(bearerToken))
            {
                try
                {
                    // U-5a. The API exists on both the netstandard2.1 compile surface and
                    // Unity's Mono System.dll (measured). Whether Mono treats Authorization as
                    // a restricted header is the one thing only a run can answer, so the
                    // failure is reported rather than thrown into the caller's frame.
                    socket.Options.SetRequestHeader("Authorization", "Bearer " + bearerToken);
                }
                catch (Exception ex)
                {
                    _state = TransportState.Disconnected;
                    ReportDisconnect(new DisconnectInfo(
                        DisconnectKind.Failed, null,
                        "could not set the Authorization header (" + ex.GetType().Name + ": " + ex.Message +
                        "). ADR-0005 section 6 lists the fallback credential paths."));
                    return;
                }
            }

            // Matches the server's 15 s ping / 30 s idle window (ADR-0005 section 2) so a dead
            // peer is noticed here too instead of looking alive for minutes.
            socket.Options.KeepAliveInterval = TimeSpan.FromSeconds(15);

            // Fire and forget: every failure path inside reports through Disconnected,
            // so there is no Task for the caller to await and nothing to observe later.
            _ = RunConnection(socket, url, _cancellation.Token);
        }

        async Task RunConnection(ClientWebSocket socket, string url, CancellationToken token)
        {
            try
            {
                await socket.ConnectAsync(new Uri(url), token).ConfigureAwait(false);
            }
            catch (Exception ex)
            {
                _state = TransportState.Disconnected;
                // 401, 503 and a refused TCP connection are indistinguishable here (U-5c).
                ReportDisconnect(new DisconnectInfo(DisconnectKind.Failed, null, Describe(ex)));
                return;
            }

            _state = TransportState.Open;
            _ = SendLoop(socket, token);
            Post(() => { Action handler = Opened; if (handler != null) handler(); });

            await ReceiveLoop(socket, token).ConfigureAwait(false);
        }

        async Task ReceiveLoop(ClientWebSocket socket, CancellationToken token)
        {
            var segment = new ArraySegment<byte>(_receiveBuffer);

            while (!token.IsCancellationRequested && socket.State == WebSocketState.Open)
            {
                _assembly.SetLength(0);
                WebSocketReceiveResult result;

                try
                {
                    do
                    {
                        result = await socket.ReceiveAsync(segment, token).ConfigureAwait(false);

                        if (result.MessageType == WebSocketMessageType.Close)
                        {
                            _state = TransportState.Closing;
                            try
                            {
                                await socket.CloseOutputAsync(
                                    WebSocketCloseStatus.NormalClosure, "peer closed", CancellationToken.None)
                                    .ConfigureAwait(false);
                            }
                            catch (Exception) { /* the peer is already gone; nothing to salvage */ }

                            _state = TransportState.Disconnected;
                            ReportDisconnect(new DisconnectInfo(
                                DisconnectKind.Remote,
                                result.CloseStatus.HasValue ? (int)result.CloseStatus.Value : (int?)null,
                                result.CloseStatusDescription));
                            return;
                        }

                        if (result.MessageType == WebSocketMessageType.Binary)
                        {
                            // The server counts this as a protocol violation in the other
                            // direction (ADR-0005 section 2); we refuse it for the same reason.
                            await AbortWith(socket, "binary frames are not part of this protocol").ConfigureAwait(false);
                            return;
                        }

                        if (_assembly.Length + result.Count > MaxFrameBytes)
                        {
                            await AbortWith(socket, "inbound frame exceeded " + MaxFrameBytes + " bytes").ConfigureAwait(false);
                            return;
                        }

                        _assembly.Write(_receiveBuffer, 0, result.Count);
                    }
                    while (!result.EndOfMessage);
                }
                catch (OperationCanceledException)
                {
                    _state = TransportState.Disconnected;
                    ReportDisconnect(new DisconnectInfo(DisconnectKind.Local, 1000, "cancelled"));
                    return;
                }
                catch (Exception ex)
                {
                    _state = TransportState.Disconnected;
                    ReportDisconnect(new DisconnectInfo(DisconnectKind.Failed, null, Describe(ex)));
                    return;
                }

                string text = Encoding.UTF8.GetString(_assembly.GetBuffer(), 0, (int)_assembly.Length);
                Post(() => { Action<string> handler = MessageReceived; if (handler != null) handler(text); });
            }

            _state = TransportState.Disconnected;
            ReportDisconnect(new DisconnectInfo(DisconnectKind.Failed, null, "receive loop ended in state " + socket.State));
        }

        async Task AbortWith(ClientWebSocket socket, string detail)
        {
            _log.Warn("starfall.net: closing on protocol violation - " + detail);
            try
            {
                await socket.CloseAsync(WebSocketCloseStatus.ProtocolError, detail, CancellationToken.None)
                    .ConfigureAwait(false);
            }
            catch (Exception) { /* best effort */ }

            _state = TransportState.Disconnected;
            ReportDisconnect(new DisconnectInfo(DisconnectKind.Failed, 1002, detail));
        }

        async Task SendLoop(ClientWebSocket socket, CancellationToken token)
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    await _outboundSignal.WaitAsync(token).ConfigureAwait(false);
                }
                catch (OperationCanceledException) { return; }

                string message;
                if (!_outbound.TryDequeue(out message)) continue;
                if (socket.State != WebSocketState.Open) return;

                byte[] payload = Encoding.UTF8.GetBytes(message);
                try
                {
                    await socket.SendAsync(
                        new ArraySegment<byte>(payload), WebSocketMessageType.Text, true, token)
                        .ConfigureAwait(false);
                }
                catch (OperationCanceledException) { return; }
                catch (Exception ex)
                {
                    // The receive loop owns disconnect reporting; it will see the same failure.
                    _log.Warn("starfall.net: send failed - " + Describe(ex));
                    return;
                }
            }
        }

        public bool Send(string message)
        {
            if (message == null) throw new ArgumentNullException(nameof(message));
            if (_state != TransportState.Open) return false;
            if (_outbound.Count >= MaxOutboundQueue)
            {
                _log.Warn("starfall.net: outbound queue full (" + MaxOutboundQueue + "), refusing to send");
                return false;
            }

            _outbound.Enqueue(message);
            _outboundSignal.Release();
            return true;
        }

        public void Close(string reason)
        {
            ClientWebSocket socket = _socket;
            if (socket == null) return;
            if (_state == TransportState.Disconnected) return;

            _state = TransportState.Closing;
            Task.Run(() => CloseCore(socket, reason));
        }

        public bool CloseBlocking(string reason, int timeoutMs)
        {
            ClientWebSocket socket = _socket;
            if (socket == null || _state == TransportState.Disconnected) return true;

            _state = TransportState.Closing;
            Task close = Task.Run(() => CloseCore(socket, reason));
            bool completed = close.Wait(timeoutMs);

            // Whether or not the close frame made it, stop the loops: this is called when the
            // assembly is about to be unloaded.
            CancelLoops();
            return completed;
        }

        async Task CloseCore(ClientWebSocket socket, string reason)
        {
            try
            {
                if (socket.State == WebSocketState.Open)
                {
                    await socket.CloseAsync(WebSocketCloseStatus.NormalClosure, reason ?? "client closed", CancellationToken.None)
                        .ConfigureAwait(false);
                }
            }
            catch (Exception ex)
            {
                _log.Warn("starfall.net: close failed - " + Describe(ex));
            }
            finally
            {
                _state = TransportState.Disconnected;
                ReportDisconnect(new DisconnectInfo(DisconnectKind.Local, 1000, reason));
            }
        }

        void CancelLoops()
        {
            CancellationTokenSource cancellation = _cancellation;
            if (cancellation == null) return;
            try { cancellation.Cancel(); } catch (ObjectDisposedException) { }
        }

        /// <summary>Exactly one disconnect notification per connection, whichever loop notices
        /// first. Without this the caller's reconnect logic runs twice for one failure and the
        /// attempt counter jumps by two.</summary>
        void ReportDisconnect(DisconnectInfo info)
        {
            if (Interlocked.Exchange(ref _disconnectReported, 1) != 0) return;
            Post(() => { Action<DisconnectInfo> handler = Disconnected; if (handler != null) handler(info); });
        }

        void Post(Action action) => _mainThread.Enqueue(action);

        public void Pump()
        {
            Action action;
            while (_mainThread.TryDequeue(out action))
            {
                action();
            }
        }

        void DrainOutbound()
        {
            string ignored;
            while (_outbound.TryDequeue(out ignored)) { }
        }

        static string Describe(Exception ex)
        {
            var wse = ex as WebSocketException;
            if (wse != null) return "WebSocketException(" + wse.WebSocketErrorCode + "): " + wse.Message;
            return ex.GetType().Name + ": " + ex.Message;
        }

        public void Dispose()
        {
            CancelLoops();

            ClientWebSocket socket = _socket;
            _socket = null;
            if (socket != null)
            {
                try { socket.Dispose(); } catch (Exception) { }
            }

            CancellationTokenSource cancellation = _cancellation;
            _cancellation = null;
            if (cancellation != null)
            {
                try { cancellation.Dispose(); } catch (Exception) { }
            }

            _assembly.Dispose();
            _outboundSignal.Dispose();
            _state = TransportState.Disconnected;
        }
    }
}
