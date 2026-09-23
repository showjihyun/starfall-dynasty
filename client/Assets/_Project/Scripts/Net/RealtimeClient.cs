// Ties the transport to the contract: dispatch, the pending-command list, reconnect.
//
// Main thread only. Every callback it raises comes out of Pump(), which the owning
// MonoBehaviour calls from Update. The transport does the thread marshalling; this class must
// never learn about threads or it will grow locks it does not need.
//
// Rules: docs/adr/0005 sections 3 and 5, invariants I-11, I-15, I-23.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using Newtonsoft.Json;
using Starfall.Contracts;
using Starfall.Contracts.Generated;

namespace Starfall.Net
{
    /// <summary>What the server told us this session is. The client never asserts any of it
    /// (ADR-0008 section 5) - it is handed these values in SESSION_READY.</summary>
    public readonly struct SessionIdentity
    {
        public readonly Guid SessionId;
        public readonly Guid CorrelationId;
        public readonly Guid ActorId;
        public readonly Guid WorldId;
        public readonly int TickHz;
        public readonly string ServerVersion;

        public SessionIdentity(Guid sessionId, Guid correlationId, Guid actorId, Guid worldId, int tickHz, string serverVersion)
        {
            SessionId = sessionId;
            CorrelationId = correlationId;
            ActorId = actorId;
            WorldId = worldId;
            TickHz = tickHz;
            ServerVersion = serverVersion;
        }
    }

    public sealed class RealtimeClient : IDisposable
    {
        readonly IRealtimeTransport _transport;
        readonly ILogSink _log;
        readonly Stopwatch _clock = Stopwatch.StartNew();
        readonly Random _jitter;

        // Built once and reused: the Runtime profile must not allocate settings per frame.
        readonly JsonSerializer _runtimeSerializer;
        readonly JsonSerializerSettings _runtimeSettings;

        readonly Dictionary<string, Action<object>> _handlers =
            new Dictionary<string, Action<object>>(StringComparer.Ordinal);

        string _url;
        string _token;
        bool _wantConnected;
        int _attempt;
        double _reconnectAtMs = -1;
        uint _nextProbeSeq;

        public RealtimeClient(IRealtimeTransport transport, ILogSink log, int jitterSeed)
        {
            _transport = transport ?? throw new ArgumentNullException(nameof(transport));
            _log = log ?? NullLogSink.Instance;
            _jitter = new Random(jitterSeed);

            Pending = new PendingCommands(() => _clock.Elapsed.Ticks);

            _runtimeSettings = ContractJson.CreateRuntime(OnIgnoredMember);
            _runtimeSerializer = JsonSerializer.Create(_runtimeSettings);

            _transport.Opened += OnOpened;
            _transport.MessageReceived += OnMessage;
            _transport.Disconnected += OnDisconnected;

            Register<SessionReadyMessage>("SESSION_READY", OnSessionReady);
            Register<CommandResultMessage>("COMMAND_RESULT", OnCommandResult);
            Register<PingReplyMessage>("PING_REPLY", OnPingReply);
        }

        public PendingCommands Pending { get; }

        /// <summary>The current session, or null before SESSION_READY / after a disconnect.</summary>
        public SessionIdentity? Session { get; private set; }

        /// <summary>Retries since the last SESSION_READY. Resets only there (ADR-0005 section 5).</summary>
        public int Attempt => _attempt;

        public bool IsReady => Session.HasValue;

        /// <summary>Raised on the main thread once the server has told us who we are.</summary>
        public event Action<SessionIdentity> SessionReady;

        /// <summary>Raised when a session ends. The argument is how many commands were dropped.</summary>
        public event Action<int> SessionEnded;

        /// <summary>
        /// Raised when this connection closed with <see cref="ReconnectPolicy.SupersededCloseCode"/>
        /// (4001): the same actor opened a newer session and this one's ship was handed to it.
        /// The client stops reconnecting when this fires (R3 decision 5) - callers use this to
        /// show the player "connected elsewhere" instead of leaving them looking at a stalled
        /// HUD with no explanation.
        /// </summary>
        public event Action SupersededElsewhere;

        /// <summary>Registers a handler for one registry type. Unknown types are warned about
        /// and skipped, never thrown (ADR-0005 section 5).</summary>
        public void Register<T>(string typeName, Action<T> handler) where T : class
        {
            if (string.IsNullOrEmpty(typeName)) throw new ArgumentException("typeName is required", nameof(typeName));
            if (handler == null) throw new ArgumentNullException(nameof(handler));

            _handlers[typeName] = contract =>
            {
                var typed = contract as T;
                if (typed != null) handler(typed);
            };
        }

        /// <summary>Starts connecting and keeps reconnecting until <see cref="Disconnect"/>.</summary>
        public void Connect(string url, string bearerToken)
        {
            _url = url;
            _token = bearerToken;
            _wantConnected = true;
            _attempt = 0;
            _reconnectAtMs = -1;

            if (_transport.State == TransportState.Disconnected) _transport.Connect(_url, _token);
        }

        /// <summary>Stops reconnecting and closes the socket normally.</summary>
        public void Disconnect(ClientCloseReason reason)
        {
            _wantConnected = false;
            _reconnectAtMs = -1;

            if (Session.HasValue) _log.Info(StarfallNetLog.Closing(Session.Value.SessionId, reason));
            _transport.Close(StarfallNetLog.Name(reason));
        }

        /// <summary>Closes and waits, for hooks that cannot await: domain reload, play mode
        /// exit, application quit. Without this the server records TRANSPORT_ERROR instead of
        /// CLIENT_CLOSED (AC-13c).</summary>
        public bool DisconnectBlocking(ClientCloseReason reason, int timeoutMs)
        {
            _wantConnected = false;
            _reconnectAtMs = -1;

            if (Session.HasValue) _log.Info(StarfallNetLog.Closing(Session.Value.SessionId, reason));
            return _transport.CloseBlocking(StarfallNetLog.Name(reason), timeoutMs);
        }

        /// <summary>Drives the transport and the reconnect timer. Once per frame, main thread.</summary>
        public void Pump()
        {
            _transport.Pump();

            if (!_wantConnected || _reconnectAtMs < 0) return;
            if (_transport.State != TransportState.Disconnected) return;
            if (_clock.Elapsed.TotalMilliseconds < _reconnectAtMs) return;

            _reconnectAtMs = -1;
            _transport.Connect(_url, _token);
        }

        /// <summary>
        /// Sends one PING_SERVER and tracks it. Returns the command_id, or null when the
        /// socket is not open - the caller is told rather than queued behind a hope.
        /// </summary>
        public Guid? SendPing()
        {
            if (_transport.State != TransportState.Open) return null;

            Guid commandId = UuidV7.NewGuid();
            uint probeSeq = _nextProbeSeq++;

            var command = new PingServerCommand
            {
                CommandId = commandId,
                CommandType = PingServerCommand.CommandTypeConst,
                SchemaVersion = PingServerCommand.SchemaVersionConst,
                // Diagnostics only. The server never orders or judges by it (I-11).
                ClientSentAt = DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture),
                Payload = new PingServerCommand.PingServerPayload { ProbeSeq = probeSeq },
            };

            string json = ContractJson.Serialize(command);
            if (!_transport.Send(json)) return null;

            Pending.Track(commandId, probeSeq);
            return commandId;
        }

        /// <summary>
        /// Sends an already-built, already-serialized command as-is. For commands that only
        /// ever get ACCEPTED/REJECTED and no type-specific reply (SET_SHIP_CONTROL,
        /// ADR-0011 section 4) - unlike <see cref="SendPing"/> this does NOT register the
        /// command with <see cref="Pending"/>, because Pending.Track assumes a later
        /// OnTypeResult call completes it (I-15's two-step), and a command with no reply would
        /// stay "in flight" there forever. Callers that need to know a specific command_id's
        /// outcome subscribe to <see cref="CommandResultReceived"/> instead.
        /// </summary>
        /// <returns>false when the socket is not open - the caller is told rather than queued
        /// behind a hope (same contract as <see cref="SendPing"/>).</returns>
        public bool TrySendJson(string json)
        {
            return _transport.State == TransportState.Open && _transport.Send(json);
        }

        /// <summary>Raised for every COMMAND_RESULT, regardless of whether the command_id is
        /// tracked in <see cref="Pending"/>. Ping's own accept/reply bookkeeping (Pending) is
        /// unaffected - this is an additional, unconditional observation point for callers
        /// (Starfall.Greybox) that manage their own per-command_id lifecycle, such as flight
        /// input's COMMAND_RESULT{REJECTED} -> drop-from-prediction-history path
        /// (ADR-0012 section 3).</summary>
        public event Action<CommandResultMessage> CommandResultReceived;

        void OnOpened()
        {
            // Deliberately NOT a reset point for _attempt. A server that accepts the socket
            // and closes it immediately would otherwise hold 30 clients in a 500 ms retry loop
            // forever (ADR-0005 section 5).
            _log.Info(StarfallNetLog.Prefix + "socket open, waiting for SESSION_READY (attempt=" +
                      _attempt.ToString(CultureInfo.InvariantCulture) + ")");
        }

        void OnMessage(string json)
        {
            // ContractDispatch.TryRead(string, ...) peeks the type discriminator with a
            // forward-only token scan and, for WORLD_SNAPSHOT (p1-01-ship-movement, R4),
            // deserializes straight from the string - it never builds a JObject tree.
            // Pre-parsing to JObject here (as p0-02 did) would defeat that: it is exactly the
            // 8x-garbage tree this path exists to skip. Every other type still goes through
            // the tree internally, unchanged.
            string typeName;
            object contract;
            string error;
            if (!ContractDispatch.TryRead(json, _runtimeSerializer, out typeName, out contract, out error))
            {
                // Three distinguishable outcomes bottleneck through the same bool: JSON that
                // does not even parse (p0-02's original wording, kept so log-scraping tooling
                // does not need to change), a type we do not know (expected - the server is
                // deployed first), and a type we know but cannot read (a contract break). All
                // three are warnings, and in all three the next frame is still processed
                // (AC-12c) - the connection never comes down over one bad frame.
                string prefix = error != null && error.StartsWith("not valid contract JSON", StringComparison.Ordinal)
                    ? "unreadable frame, ignoring: "
                    : "skipping frame: ";
                _log.Warn(StarfallNetLog.Prefix + prefix + error);
                return;
            }

            Action<object> handler;
            if (!_handlers.TryGetValue(typeName, out handler))
            {
                _log.Warn(StarfallNetLog.Prefix + "no handler registered for '" + typeName + "', continuing");
                return;
            }

            handler(contract);
        }

        void OnSessionReady(SessionReadyMessage message)
        {
            SessionReadyMessage.SessionReadyPayload payload = message.Payload;
            var identity = new SessionIdentity(
                payload.SessionId,
                // SESSION_READY carries the session correlation; the server shares it with
                // SESSION_OPENED and SESSION_CLOSED, which is how QA joins the pair.
                message.CorrelationId ?? Guid.Empty,
                payload.ActorId,
                payload.WorldId,
                payload.TickHz,
                payload.ServerVersion);

            Session = identity;

            _log.Info(StarfallNetLog.SessionReady(
                identity.SessionId, identity.CorrelationId, identity.ActorId,
                identity.WorldId, identity.TickHz, identity.ServerVersion, _attempt));

            // The one reset point.
            _attempt = 0;

            Action<SessionIdentity> handler = SessionReady;
            if (handler != null) handler(identity);
        }

        void OnCommandResult(CommandResultMessage message)
        {
            CommandResultMessage.CommandResultPayload payload = message.Payload;
            bool knownToPending = Pending.OnCommandResult(payload.CommandId, payload.Status, payload.ReasonCode);

            Action<CommandResultMessage> handler = CommandResultReceived;
            if (handler != null) handler(message);

            // A command_id neither Pending nor any CommandResultReceived subscriber recognises
            // is the original diagnostic this guarded: "the server answered something we never
            // sent". Once a subscriber exists (Starfall.Greybox, tracking SET_SHIP_CONTROL
            // outside Pending by design - see TrySendJson), every one of its ~20/s results would
            // otherwise be "unknown to Pending" and flood this warning for expected traffic.
            if (!knownToPending && handler == null)
            {
                _log.Warn(StarfallNetLog.Prefix + "COMMAND_RESULT for unknown command_id=" + payload.CommandId);
            }
        }

        void OnPingReply(PingReplyMessage message)
        {
            PingReplyMessage.PingReplyPayload payload = message.Payload;
            bool orderViolation;
            bool probeMismatch;
            if (!Pending.OnTypeResult(payload.CommandId, payload.ProbeSeq, out orderViolation, out probeMismatch))
            {
                _log.Warn(StarfallNetLog.Prefix + "PING_REPLY for unknown command_id=" + payload.CommandId);
                return;
            }

            if (orderViolation)
                _log.Error(StarfallNetLog.Prefix + "PING_REPLY arrived before COMMAND_RESULT for command_id=" +
                           payload.CommandId + " (violates I-15)");
            if (probeMismatch)
                _log.Error(StarfallNetLog.Prefix + "PING_REPLY probe_seq does not match the command for command_id=" +
                           payload.CommandId);
        }

        void OnDisconnected(DisconnectInfo info)
        {
            Session = null;

            int dropped = Pending.FailAllOnDisconnect();
            // Written even when zero: "nothing was in flight" is evidence too (AC-14).
            _log.Info(StarfallNetLog.DroppedInFlight(dropped));

            Action<int> ended = SessionEnded;
            if (ended != null) ended(dropped);

            if (!_wantConnected)
            {
                _log.Info(StarfallNetLog.Prefix + "disconnected (" + info + "), not reconnecting");
                return;
            }

            if (!ReconnectPolicy.ShouldReconnect(info.CloseCode))
            {
                // R3 decision 5: the server already handed this actor's ship to a newer
                // session in the same tick (SESSION_CLOSED{SUPERSEDED}) before closing this
                // connection with code 4001. Reconnecting would just open a third session that
                // gets closed the same way, and since SESSION_READY is the only point _attempt
                // resets to 0, two clients on the same account would otherwise push each other
                // off forever at the backoff floor. This is a policy stop, not a transient
                // failure - treat it like an explicit Disconnect().
                _wantConnected = false;
                _reconnectAtMs = -1;
                _log.Info(StarfallNetLog.Superseded(info.CloseCode.Value));

                Action superseded = SupersededElsewhere;
                if (superseded != null) superseded();
                return;
            }

            int delayMs = ReconnectPolicy.DelayMs(_attempt, _jitter);
            _log.Info(StarfallNetLog.Reconnect(_attempt, delayMs));
            _log.Info(StarfallNetLog.Prefix + "disconnect detail: " + info);

            _attempt++;
            _reconnectAtMs = _clock.Elapsed.TotalMilliseconds + delayMs;
        }

        void OnIgnoredMember(string path)
        {
            _log.Warn(StarfallNetLog.Prefix + "ignored unknown member at '" + path +
                      "' (server may be newer; message kept)");
        }

        public void Dispose()
        {
            _transport.Opened -= OnOpened;
            _transport.MessageReceived -= OnMessage;
            _transport.Disconnected -= OnDisconnected;
            _transport.Dispose();
        }
    }
}
