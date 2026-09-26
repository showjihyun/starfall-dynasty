// The seam between "a socket" and "the game". Rules: docs/adr/0005 section 5.
//
// Nothing PC-specific may appear in this file. ClientWebSocket, Task, CancellationToken and
// WebSocketException all stay inside PcWebSocketTransport, because the WebGL implementation
// that eventually sits behind this interface has none of them - it has browser callbacks and
// cannot set an Authorization header at all (ADR-0005 section 6).

using System;

namespace Starfall.Net
{
    /// <summary>Connection state as the caller sees it.</summary>
    public enum TransportState
    {
        /// <summary>Never connected, or fully closed.</summary>
        Disconnected,

        /// <summary>Upgrade in progress.</summary>
        Connecting,

        /// <summary>Socket is open. This is not "ready": the session is ready when
        /// SESSION_READY arrives, which is the transport's first inbound frame.</summary>
        Open,

        /// <summary>Close handshake in progress.</summary>
        Closing,
    }

    /// <summary>What ended a connection, as far as the transport can tell.</summary>
    public enum DisconnectKind
    {
        /// <summary>We asked to close.</summary>
        Local,

        /// <summary>The peer closed cleanly.</summary>
        Remote,

        /// <summary>
        /// Anything else: the upgrade was refused, the socket failed, the handshake timed out.
        /// <para>
        /// There is deliberately no "Unauthorized" member. Unity's netstandard2.1 surface has
        /// no <c>ClientWebSocketOptions.CollectHttpResponseDetails</c> (measured), so 401, 503
        /// and a dropped packet are the same <c>WebSocketException</c> here. Inventing a
        /// member we cannot populate would let callers branch on a value that is always
        /// wrong. The evidence for "why was the upgrade refused" lives on the server
        /// (ADR-0005 section 6).
        /// </para>
        /// </summary>
        Failed,
    }

    /// <summary>A connection that ended, with whatever the transport knows about why.</summary>
    public readonly struct DisconnectInfo
    {
        public readonly DisconnectKind Kind;

        /// <summary>WebSocket close code when the peer sent one, otherwise null.</summary>
        public readonly int? CloseCode;

        /// <summary>Diagnostic text for the log. Never parsed.</summary>
        public readonly string Detail;

        public DisconnectInfo(DisconnectKind kind, int? closeCode, string detail)
        {
            Kind = kind;
            CloseCode = closeCode;
            Detail = detail;
        }

        public override string ToString() =>
            Kind + (CloseCode.HasValue ? " code=" + CloseCode.Value : "") +
            (string.IsNullOrEmpty(Detail) ? "" : " " + Detail);
    }

    /// <summary>
    /// A realtime connection carrying one contract message per frame (ADR-0005 section 2).
    /// <para>
    /// <b>Threading:</b> implementations receive on a background thread. They must not invoke
    /// the callbacks from there. Everything queued is delivered from <see cref="Pump"/>, which
    /// the caller runs on the main thread. That is the whole reason this interface has a pump
    /// instead of events that fire whenever.
    /// </para>
    /// </summary>
    public interface IRealtimeTransport : IDisposable
    {
        TransportState State { get; }

        /// <summary>H-14 (architect R4 판정 §B / K-2): how many times <see cref="Send"/> refused
        /// a frame because the outbound queue was full. Observation only - SC-89 does NOT fail
        /// on this being nonzero (K-2: "outbound_queue_full_total은 SC-89의 관측으로 두되 합격
        /// 조건에서 빼고, 0이 아니면 별건 발견으로 연다") - it measures a DIFFERENT thing
        /// (input reaching the server at all) than SC-89 (protocol violations/disconnects).</summary>
        long QueueFullTotal { get; }

        /// <summary>Raised from <see cref="Pump"/> once the socket is open.</summary>
        event Action Opened;

        /// <summary>One inbound frame, already decoded to text. Raised from <see cref="Pump"/>.</summary>
        event Action<string> MessageReceived;

        /// <summary>Raised from <see cref="Pump"/> when the connection ends, for any reason,
        /// including a failed connect attempt.</summary>
        event Action<DisconnectInfo> Disconnected;

        /// <summary>
        /// Starts connecting. Returns immediately; success or failure arrives through
        /// <see cref="Opened"/> or <see cref="Disconnected"/> on a later pump.
        /// </summary>
        /// <param name="url">ws:// or wss:// endpoint.</param>
        /// <param name="bearerToken">Value for the Authorization header, or null for none.
        /// A WebGL implementation cannot honour this and must fail loudly rather than
        /// connecting unauthenticated.</param>
        void Connect(string url, string bearerToken);

        /// <summary>
        /// Queues one text frame. Returns false when the socket is not open, so the caller can
        /// fail the command locally instead of pretending it was sent.
        /// </summary>
        bool Send(string message);

        /// <summary>Starts a normal close (1000). Safe to call when already closed.</summary>
        void Close(string reason);

        /// <summary>
        /// Closes and blocks until the close frame is on the wire or the budget expires.
        /// <para>
        /// Needed because Editor domain reload does not await anything. A reload that unloads
        /// this assembly mid-connection makes the server record TRANSPORT_ERROR instead of
        /// CLIENT_CLOSED, which breaks AC-13(c).
        /// </para>
        /// </summary>
        /// <returns>True when the close completed inside the budget.</returns>
        bool CloseBlocking(string reason, int timeoutMs);

        /// <summary>
        /// Delivers everything queued since the last call, on the calling thread. Call it once
        /// per frame from the main thread.
        /// </summary>
        void Pump();
    }
}
