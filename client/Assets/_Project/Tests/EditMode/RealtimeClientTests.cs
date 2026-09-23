// SC-45 (unknown message_type keeps going), SC-47 (backoff), and the command state machine
// behind SC-49 and SC-51. EditMode, driven by FakeRealtimeTransport - no server, no waiting.

using System;
using System.Collections.Generic;
using System.Globalization;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Net;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RealtimeClientTests
    {
        FakeRealtimeTransport _transport;
        RecordingLogSink _log;
        RealtimeClient _client;

        [SetUp]
        public void SetUp()
        {
            _transport = new FakeRealtimeTransport();
            _log = new RecordingLogSink();
            _client = new RealtimeClient(_transport, _log, jitterSeed: 1234);
        }

        [TearDown]
        public void TearDown()
        {
            if (_client != null) _client.Dispose();
        }

        // ------------------------------------------------------------------ SC-47

        [Test]
        public void Backoff_BoundaryAttempts_AreAlwaysInsideTheCap()
        {
            int[] attempts = { 0, 1, 5, 62, 63, 64, 100 };
            var random = new Random(20260918);

            foreach (int attempt in attempts)
            {
                int ceiling = ReconnectPolicy.CeilingMs(attempt);
                int delay = ReconnectPolicy.DelayMs(attempt, random);

                TestContext.WriteLine("n=" + attempt.ToString(CultureInfo.InvariantCulture) +
                                      "  ceiling_ms=" + ceiling +
                                      "  delay_ms=" + delay);

                Assert.That(delay, Is.GreaterThan(0),
                    "n=" + attempt + " produced a zero delay. Without the exponent clamp, " +
                    "500L << 62 is 0 in C# and 30 clients reconnect with no delay at all.");
                Assert.That(delay, Is.LessThanOrEqualTo(ReconnectPolicy.CapMs));
                Assert.That(delay, Is.LessThanOrEqualTo(ceiling));
            }

            TestContext.WriteLine("boundary attempts checked: " + attempts.Length);
            Assert.That(attempts.Length, Is.EqualTo(7));
        }

        [Test]
        public void Backoff_CeilingDoublesThenClamps()
        {
            int[] expected = { 500, 1000, 2000, 4000, 8000, 10000, 10000, 10000 };
            for (int attempt = 0; attempt < expected.Length; attempt++)
            {
                int ceiling = ReconnectPolicy.CeilingMs(attempt);
                TestContext.WriteLine("n=" + attempt + " ceiling_ms=" + ceiling + " expected=" + expected[attempt]);
                Assert.That(ceiling, Is.EqualTo(expected[attempt]));
            }

            // 500 * 2^5 = 16000, already past the cap, so nothing above 5 can change anything.
            Assert.That(ReconnectPolicy.CeilingMs(ReconnectPolicy.MaxExponent), Is.EqualTo(ReconnectPolicy.CapMs));
        }

        [Test]
        public void Backoff_IsDeterministicForAFixedSeed()
        {
            var first = new List<int>();
            var second = new List<int>();

            var a = new Random(99);
            var b = new Random(99);
            for (int attempt = 0; attempt < 12; attempt++)
            {
                first.Add(ReconnectPolicy.DelayMs(attempt, a));
                second.Add(ReconnectPolicy.DelayMs(attempt, b));
            }

            TestContext.WriteLine("seed 99 delays: " + string.Join(", ", first));
            Assert.That(second, Is.EqualTo(first));

            // Full jitter, not a fixed schedule: 30 clients dropping together must not line up.
            var distinct = new HashSet<int>(first);
            Assert.That(distinct.Count, Is.GreaterThan(1), "a full-jitter delay must not be constant");
        }

        [Test]
        public void Backoff_RejectsNegativeAttempt()
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => ReconnectPolicy.CeilingMs(-1));
            Assert.Throws<ArgumentNullException>(() => ReconnectPolicy.DelayMs(0, null));
        }

        // ------------------------------------------------------------------ SUPERSEDED (close code 4001, R3 decision 5)
        //
        // Pure function, no transport (01_architect_decisions.md R3 1.3: "재연결 정책은 순수
        // C#이라 전송 계층 없이 테스트 가능할 것이다"). The RealtimeClient-level behaviour tests
        // below cover the wiring; this covers the policy table itself.

        [Test]
        public void ShouldReconnect_IsFalseOnlyForTheSupersededCloseCode()
        {
            int?[] codes = { null, 1000, 1001, 1006, 4000, ReconnectPolicy.SupersededCloseCode, 4002 };
            bool[] expected = { true, true, true, true, true, false, true };

            for (int i = 0; i < codes.Length; i++)
            {
                bool actual = ReconnectPolicy.ShouldReconnect(codes[i]);
                TestContext.WriteLine("close_code=" + (codes[i].HasValue ? codes[i].Value.ToString(CultureInfo.InvariantCulture) : "null") +
                                      " should_reconnect=" + actual);
                Assert.That(actual, Is.EqualTo(expected[i]), "close_code=" + codes[i]);
            }
        }

        [Test]
        public void ShouldReconnect_SupersededCloseCodeIs4001()
        {
            // RFC 6455 application range, ADR-0005's close-code table (R3 1.3): a magic number
            // pinned as a named constant so it appears exactly once in the codebase.
            Assert.That(ReconnectPolicy.SupersededCloseCode, Is.EqualTo(4001));
        }

        // ------------------------------------------------------------------ SC-45

        [Test]
        public void Dispatch_UnknownMessageType_WarnsAndKeepsProcessing()
        {
            ConnectAndOpen();

            var ready = new List<SessionIdentity>();
            _client.SessionReady += ready.Add;

            // An unknown type first, a known one second. The second must still arrive: a
            // client that stops on the first message it does not recognise cannot survive the
            // server being deployed ahead of it.
            _transport.SimulateMessage(UnknownTypeFrame());
            _transport.SimulateMessage(SessionReadyFrame(Guid.NewGuid(), Guid.NewGuid()));
            _client.Pump();

            foreach (string line in _log.Lines) TestContext.WriteLine(line);

            Assert.That(_log.Contains("unknown contract type 'TOTALLY_NEW_MESSAGE'"), Is.True,
                "the unknown type should have produced a warning");
            Assert.That(ready.Count, Is.EqualTo(1),
                "the message after the unknown one must still be handled");
            Assert.That(_client.IsReady, Is.True);
        }

        [Test]
        public void Dispatch_UnreadableFrame_WarnsAndKeepsProcessing()
        {
            ConnectAndOpen();

            _transport.SimulateMessage("{ this is not json");
            _transport.SimulateMessage(SessionReadyFrame(Guid.NewGuid(), Guid.NewGuid()));
            _client.Pump();

            Assert.That(_log.Contains("unreadable frame"), Is.True);
            Assert.That(_client.IsReady, Is.True);
        }

        // ------------------------------------------------------------------ session identity

        [Test]
        public void SessionReady_IsLoggedInTheFormatQaParses()
        {
            ConnectAndOpen();

            Guid sessionId = Guid.Parse("01a0b1c2-4a11-7b22-9c33-0d44e55f6a77");
            Guid correlationId = Guid.Parse("01a0b1c2-4a12-7c01-8d02-1e033f044a05");

            _transport.SimulateMessage(SessionReadyFrame(sessionId, correlationId));
            _client.Pump();

            string expected = "starfall.net: SESSION_READY session_id=" + sessionId.ToString("D") +
                              " correlation_id=" + correlationId.ToString("D");

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.That(_log.Contains(expected), Is.True,
                "QA greps for this exact prefix and field order (02_client_ack.md section 1): " + expected);

            Assert.That(_client.Session.HasValue, Is.True);
            Assert.That(_client.Session.Value.TickHz, Is.EqualTo(20));
            Assert.That(_client.Session.Value.CorrelationId, Is.EqualTo(correlationId));
        }

        // ------------------------------------------------------------------ command state machine

        [Test]
        public void Command_AcceptedIsNotCompleted_UntilTheTypeResultArrives()
        {
            ConnectAndOpen();
            _transport.SimulateMessage(SessionReadyFrame(Guid.NewGuid(), Guid.NewGuid()));
            _client.Pump();

            Guid? commandId = _client.SendPing();
            Assert.That(commandId.HasValue, Is.True);

            var outcomes = new List<CommandOutcome>();
            _client.Pending.Settled += outcomes.Add;

            _transport.SimulateMessage(CommandResultFrame(commandId.Value, "ACCEPTED", null));
            _client.Pump();

            CommandStatus status;
            Assert.That(_client.Pending.TryGetStatus(commandId.Value, out status), Is.True,
                "ACCEPTED must not settle the command (I-15)");
            Assert.That(status, Is.EqualTo(CommandStatus.Accepted));
            Assert.That(outcomes, Is.Empty);

            _transport.SimulateMessage(PingReplyFrame(commandId.Value, 0));
            _client.Pump();

            Assert.That(outcomes.Count, Is.EqualTo(1));
            Assert.That(outcomes[0].Status, Is.EqualTo(CommandStatus.Completed));
            Assert.That(_client.Pending.Count, Is.EqualTo(0));
        }

        [Test]
        public void Command_Rejected_SettlesWithTheReasonCode()
        {
            ConnectAndOpen();
            Guid? commandId = _client.SendPing();

            var outcomes = new List<CommandOutcome>();
            _client.Pending.Settled += outcomes.Add;

            _transport.SimulateMessage(CommandResultFrame(commandId.Value, "REJECTED", "TOO_MANY_IN_FLIGHT"));
            _client.Pump();

            Assert.That(outcomes.Count, Is.EqualTo(1));
            Assert.That(outcomes[0].Status, Is.EqualTo(CommandStatus.Rejected));
            Assert.That(outcomes[0].ReasonCode, Is.EqualTo("TOO_MANY_IN_FLIGHT"));
        }

        [Test]
        public void Command_UnknownReasonCode_IsCarriedThroughNotDropped()
        {
            // A reason code this build has never heard of must still settle the command and
            // reach the caller as data. This is the whole reason the field is a string.
            ConnectAndOpen();
            Guid? commandId = _client.SendPing();

            var outcomes = new List<CommandOutcome>();
            _client.Pending.Settled += outcomes.Add;

            _transport.SimulateMessage(CommandResultFrame(commandId.Value, "REJECTED", "INVENTED_IN_A_LATER_SLICE"));
            _client.Pump();

            Assert.That(outcomes.Count, Is.EqualTo(1));
            Assert.That(outcomes[0].ReasonCode, Is.EqualTo("INVENTED_IN_A_LATER_SLICE"));
        }

        [Test]
        public void Command_ReplyBeforeResult_IsReportedAsAnOrderViolation()
        {
            ConnectAndOpen();
            Guid? commandId = _client.SendPing();

            _transport.SimulateMessage(PingReplyFrame(commandId.Value, 0));
            _client.Pump();

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.That(_log.Contains("violates I-15"), Is.True,
                "a PING_REPLY that overtakes its COMMAND_RESULT is a server ordering bug and " +
                "must be visible, not smoothed over");
        }

        // ------------------------------------------------------------------ SC-51

        [Test]
        public void Disconnect_DropsInFlightCommands_AndSaysSoInOneLine()
        {
            ConnectAndOpen();
            _client.SendPing();
            _client.SendPing();

            _transport.SimulateDisconnect(DisconnectKind.Failed, null, "socket died");
            _client.Pump();

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.That(_log.Contains("starfall.net: dropping 2 in-flight command(s) on disconnect (no resend, I-23)"),
                Is.True);
            Assert.That(_client.Pending.Count, Is.EqualTo(0));

            // Nothing was re-sent: the only frames on the wire are the two original commands.
            Assert.That(_transport.Sent.Count, Is.EqualTo(2));
        }

        [Test]
        public void Disconnect_WithNothingInFlight_StillLogsTheLine()
        {
            // Zero is evidence too. A line that only appears sometimes cannot be used to prove
            // the absence of a resend.
            ConnectAndOpen();
            _transport.SimulateDisconnect(DisconnectKind.Remote, 1000, "peer closed");
            _client.Pump();

            Assert.That(_log.Contains("starfall.net: dropping 0 in-flight command(s) on disconnect (no resend, I-23)"),
                Is.True);
        }

        [Test]
        public void Reconnect_CounterResetsOnlyOnSessionReady()
        {
            _client.Connect("ws://127.0.0.1:8080/ws", "token");
            Assert.That(_client.Attempt, Is.EqualTo(0));

            // Three connections that open and die before SESSION_READY. If opening the socket
            // reset the counter, a server stuck in a restart loop would hold every client at
            // the 500 ms floor forever.
            for (int i = 0; i < 3; i++)
            {
                _transport.SimulateOpen();
                _client.Pump();
                _transport.SimulateDisconnect(DisconnectKind.Failed, null, "closed before ready");
                _client.Pump();
            }

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.That(_client.Attempt, Is.EqualTo(3),
                "the retry counter must keep climbing while no session ever became ready");
            Assert.That(_log.Contains("starfall.net: reconnect attempt=0"), Is.True);
            Assert.That(_log.Contains("starfall.net: reconnect attempt=2"), Is.True);
            Assert.That(_log.Contains("(counter resets only on SESSION_READY)"), Is.True);

            // Now a session actually becomes ready.
            _transport.SimulateOpen();
            _client.Pump();
            _transport.SimulateMessage(SessionReadyFrame(Guid.NewGuid(), Guid.NewGuid()));
            _client.Pump();

            Assert.That(_client.Attempt, Is.EqualTo(0), "SESSION_READY is the only reset point");
        }

        [Test]
        public void Disconnect_AfterExplicitDisconnect_DoesNotReconnect()
        {
            ConnectAndOpen();
            int connectsBefore = _transport.ConnectCount;

            _client.Disconnect(ClientCloseReason.ClientClosed);
            _client.Pump();
            _client.Pump();

            Assert.That(_transport.ConnectCount, Is.EqualTo(connectsBefore),
                "an explicit disconnect must end the reconnect loop");
            Assert.That(_log.Contains("not reconnecting"), Is.True);
        }

        // ------------------------------------------------------------------ SUPERSEDED (close code 4001, R3 decision 5)
        //
        // 01_architect_decisions.md "R3 추가 판정" section 1.3: when the same actor opens a
        // second session, the server hands the ship to the newer session in the same tick and
        // closes the old connection with WebSocket close code 4001. This rule is load-bearing,
        // not optional (leader's brief): SESSION_READY is the only point _attempt resets to 0
        // (OnSessionReady above), so if the client auto-reconnected here, two windows on the
        // same account would push each other off at the ~500 ms backoff floor forever, each
        // cycle writing a permanent SESSION_OPENED/SESSION_CLOSED pair to the history record.

        [Test]
        public void Disconnect_WithSupersededCloseCode_DoesNotReconnect()
        {
            ConnectAndOpen();
            int connectsBefore = _transport.ConnectCount;

            _transport.SimulateDisconnect(DisconnectKind.Remote, ReconnectPolicy.SupersededCloseCode,
                "another session took over");
            _client.Pump();
            _client.Pump();

            Assert.That(_transport.ConnectCount, Is.EqualTo(connectsBefore),
                "close code 4001 must end the reconnect loop, exactly like an explicit Disconnect()");
        }

        [Test]
        public void Disconnect_WithSupersededCloseCode_LogsAGreppableLine()
        {
            ConnectAndOpen();

            _transport.SimulateDisconnect(DisconnectKind.Remote, ReconnectPolicy.SupersededCloseCode, "detail");
            _client.Pump();

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.That(_log.Contains("starfall.net: SUPERSEDED close_code=4001"), Is.True,
                "QA and a developer reading client/Logs/Editor.log both need a single fixed " +
                "line to confirm the client actually stopped, not a screenshot of two windows");
        }

        [Test]
        public void Disconnect_WithSupersededCloseCode_RaisesSupersededElsewhere()
        {
            ConnectAndOpen();

            bool raised = false;
            _client.SupersededElsewhere += () => raised = true;

            _transport.SimulateDisconnect(DisconnectKind.Remote, ReconnectPolicy.SupersededCloseCode, "detail");
            _client.Pump();

            Assert.That(raised, Is.True,
                "the HUD (GreyboxSession) needs an event to show 'connected elsewhere', " +
                "not just a log line nobody is watching live");
        }

        [Test]
        public void Disconnect_WithSupersededCloseCode_StillDropsInFlightCommands()
        {
            // The 4001 branch must not skip the existing disconnect bookkeeping (I-23) - it is
            // an additional decision made after the normal disconnect handling, not a
            // replacement for it.
            ConnectAndOpen();
            _client.SendPing();

            _transport.SimulateDisconnect(DisconnectKind.Remote, ReconnectPolicy.SupersededCloseCode, "detail");
            _client.Pump();

            Assert.That(_log.Contains("starfall.net: dropping 1 in-flight command(s) on disconnect (no resend, I-23)"),
                Is.True);
        }

        [TestCase(null, TestName = "OtherCloseCode_NoCodeAtAll_StillReconnects")]
        [TestCase(1000, TestName = "OtherCloseCode_NormalClosure_StillReconnects")]
        [TestCase(1001, TestName = "OtherCloseCode_GoingAway_StillReconnects")]
        [TestCase(1006, TestName = "OtherCloseCode_AbnormalClosure_StillReconnects")]
        [TestCase(4000, TestName = "OtherCloseCode_AdjacentAppRangeCode_StillReconnects")]
        [TestCase(4002, TestName = "OtherCloseCode_AdjacentAppRangeCode2_StillReconnects")]
        public void Disconnect_WithAnyOtherCloseCode_StillReconnects(int? closeCode)
        {
            // world_full's refused upgrade (no close code), a dropped TCP connection,
            // SLOW_CONSUMER, a normal 1000 close - every close code except 4001 must keep
            // reconnecting exactly as before this change.
            ConnectAndOpen();
            int connectsBefore = _transport.ConnectCount;

            _transport.SimulateDisconnect(DisconnectKind.Remote, closeCode, "detail");
            _client.Pump();

            Assert.That(_client.Attempt, Is.EqualTo(1),
                "close_code=" + closeCode + " must still schedule a reconnect (attempt incremented)");
            // The reconnect itself only fires once the backoff timer elapses, which this test
            // does not advance - Attempt climbing is the observable proof a retry was scheduled
            // (see Reconnect_CounterResetsOnlyOnSessionReady above for the same pattern).
            Assert.That(_transport.ConnectCount, Is.EqualTo(connectsBefore),
                "no immediate reconnect before the backoff delay elapses");
        }

        [Test]
        public void SendPing_WhileNotOpen_ReturnsNullInsteadOfQueueing()
        {
            _client.Connect("ws://127.0.0.1:8080/ws", "token");
            Assert.That(_client.SendPing(), Is.Null,
                "the caller must be told the command did not go out, not left hoping");
            Assert.That(_client.Pending.Count, Is.EqualTo(0));
        }

        [Test]
        public void SendPing_ProducesAValidPingServerCommand()
        {
            ConnectAndOpen();
            Guid? commandId = _client.SendPing();

            Assert.That(_transport.Sent.Count, Is.EqualTo(1));
            string json = _transport.Sent[0];
            TestContext.WriteLine("sent: " + json);

            var command = ContractJson.DeserializeStrict<PingServerCommand>(json);
            Assert.That(command.CommandId, Is.EqualTo(commandId.Value));
            Assert.That(command.CommandType, Is.EqualTo(PingServerCommand.CommandTypeConst));
            Assert.That(command.SchemaVersion, Is.EqualTo(1));
            Assert.That(command.Payload.ProbeSeq, Is.EqualTo(0u));

            // The command the client sends must satisfy the same contract the server checks,
            // including the UuidV7 shape of command_id.
            Assert.That(command.CommandId.ToString("D"), Does.Match(ContractFixtures.UuidV7Pattern()));
        }

        [Test]
        public void SendPing_ProbeSeqIncrements()
        {
            ConnectAndOpen();
            _client.SendPing();
            _client.SendPing();

            var first = ContractJson.DeserializeStrict<PingServerCommand>(_transport.Sent[0]);
            var second = ContractJson.DeserializeStrict<PingServerCommand>(_transport.Sent[1]);

            Assert.That(second.Payload.ProbeSeq, Is.EqualTo(first.Payload.ProbeSeq + 1));
            Assert.That(second.CommandId, Is.Not.EqualTo(first.CommandId));
        }

        // ------------------------------------------------------------------ helpers

        void ConnectAndOpen()
        {
            _client.Connect("ws://127.0.0.1:8080/ws", "token");
            _transport.SimulateOpen();
            _client.Pump();
        }

        static string SessionReadyFrame(Guid sessionId, Guid correlationId) =>
            "{\"message_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"message_type\":\"SESSION_READY\",\"schema_version\":1,\"tick\":1200," +
            "\"correlation_id\":\"" + correlationId.ToString("D") + "\"," +
            "\"payload\":{\"session_id\":\"" + sessionId.ToString("D") + "\"," +
            "\"world_id\":\"01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b\"," +
            "\"actor_id\":\"" + DevAuthToken.DefaultSubject + "\"," +
            "\"tick_hz\":20,\"server_version\":\"0.1.0\"}}";

        static string CommandResultFrame(Guid commandId, string status, string reasonCode) =>
            "{\"message_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"message_type\":\"COMMAND_RESULT\",\"schema_version\":1,\"tick\":1242," +
            "\"correlation_id\":null," +
            "\"payload\":{\"command_id\":\"" + commandId.ToString("D") + "\"," +
            "\"status\":\"" + status + "\"," +
            "\"reason_code\":" + (reasonCode == null ? "null" : "\"" + reasonCode + "\"") + "}}";

        static string PingReplyFrame(Guid commandId, uint probeSeq) =>
            "{\"message_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"message_type\":\"PING_REPLY\",\"schema_version\":1,\"tick\":1242," +
            "\"correlation_id\":null," +
            "\"payload\":{\"command_id\":\"" + commandId.ToString("D") + "\"," +
            "\"probe_seq\":" + probeSeq.ToString(CultureInfo.InvariantCulture) + "}}";

        static string UnknownTypeFrame() =>
            "{\"message_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"message_type\":\"TOTALLY_NEW_MESSAGE\",\"schema_version\":1,\"tick\":1," +
            "\"correlation_id\":null,\"payload\":{}}";
    }
}
