// SC-49, SC-50, SC-51: the Unity client against a running starfall-game-server.
//
// These run in EditMode rather than PlayMode on purpose. The Editor runs the same Mono
// runtime and the same Starfall.Net code either way, and EditMode gives a deterministic pump
// loop plus a report file, where a PlayMode run would depend on frame timing. The one thing
// EditMode cannot cover is domain reload behaviour (U-5b), which is measured separately.
//
// They are gated by STARFALL_LIVE_TESTS=1 and Ignore themselves otherwise, so the offline
// suite (G-1) stays green without a server. An ignored test shows up as skipped in the
// report, which is the honest signal - it is not counted as a pass.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Threading;
using Newtonsoft.Json.Linq;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Net;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class LiveServerTests
    {
        const string GateVariable = "STARFALL_LIVE_TESTS";
        const int ReadyTimeoutMs = 15000;
        const int RoundTripTimeoutMs = 15000;
        const int ReconnectTimeoutMs = 40000;

        /// <summary>Anything above the server's 16 KiB inbound limit (ADR-0005 section 2).</summary>
        const int OversizedFrameBytes = 20 * 1024;

        string _url;
        string _token;

        PcWebSocketTransport _transport;
        RealtimeClient _client;
        RecordingLogSink _log;
        List<string> _rawFrames;

        [SetUp]
        public void SetUp()
        {
            if (Environment.GetEnvironmentVariable(GateVariable) != "1")
            {
                Assert.Ignore("live server tests are gated: set " + GateVariable + "=1 and start " +
                              "starfall-game-server first. Not a pass - the report counts this as skipped.");
            }

            string secret = DevAuthToken.ResolveSecret();
            if (secret == null)
                Assert.Fail(DevAuthToken.SecretVariable + " is not set, but " + GateVariable + "=1 asked for a live run.");

            _url = Environment.GetEnvironmentVariable(StarfallNetHost.UrlVariable);
            if (string.IsNullOrEmpty(_url)) _url = StarfallNetHost.DefaultUrl;
            _token = DevAuthToken.FromEnvironment();

            _log = new RecordingLogSink();
            _rawFrames = new List<string>();
            _transport = new PcWebSocketTransport(_log);
            _transport.MessageReceived += _rawFrames.Add;
            _client = new RealtimeClient(_transport, _log, jitterSeed: 20260919);
        }

        [TearDown]
        public void TearDown()
        {
            if (_client != null)
            {
                _client.DisconnectBlocking(ClientCloseReason.ClientClosed, 2000);
                _client.Dispose();
                _client = null;
            }
            if (_log != null)
            {
                foreach (string line in _log.Lines) TestContext.WriteLine(line);
            }
        }

        // ------------------------------------------------------------------ SC-49 / SC-50

        [Test]
        public void Live_ThreePings_RoundTripInOrder()
        {
            _client.Connect(_url, _token);
            SessionIdentity identity = WaitForReady(ReadyTimeoutMs);

            // (a) SESSION_READY is the first contract message on the connection, and the
            // client learns who it is rather than asserting it (ADR-0008 section 5).
            Assert.That(_rawFrames.Count, Is.GreaterThanOrEqualTo(1));
            TestContext.WriteLine("first frame on the wire: " + _rawFrames[0]);
            Assert.That(TypeOf(_rawFrames[0]), Is.EqualTo("SESSION_READY"),
                "SESSION_READY must be the first contract message (ADR-0005 section 3)");

            Assert.That(identity.TickHz, Is.EqualTo(20));
            Assert.That(identity.ActorId.ToString("D"), Is.EqualTo(DevAuthToken.ResolveSubject()),
                "the server must derive actor_id from the authenticated subject (I-10)");
            Assert.That(identity.CorrelationId, Is.Not.EqualTo(Guid.Empty));

            TestContext.WriteLine("SESSION session_id=" + identity.SessionId.ToString("D") +
                                  " correlation_id=" + identity.CorrelationId.ToString("D") +
                                  " actor_id=" + identity.ActorId.ToString("D") +
                                  " world_id=" + identity.WorldId.ToString("D") +
                                  " server_version=" + identity.ServerVersion);

            // (b) three commands, each answered by COMMAND_RESULT then PING_REPLY.
            var outcomes = new List<CommandOutcome>();
            _client.Pending.Settled += outcomes.Add;

            var sent = new List<Guid>();
            for (int i = 0; i < 3; i++)
            {
                Guid? commandId = _client.SendPing();
                Assert.That(commandId.HasValue, Is.True, "ping " + i + " did not go out");
                sent.Add(commandId.Value);
            }

            PumpUntil(() => outcomes.Count == 3, RoundTripTimeoutMs, "3 commands to settle");

            foreach (CommandOutcome outcome in outcomes)
            {
                TestContext.WriteLine("command " + outcome.CommandId.ToString("D") + " -> " +
                                      outcome.Status + " in " +
                                      outcome.ElapsedMs.ToString("F1", CultureInfo.InvariantCulture) + " ms");
                Assert.That(outcome.Status, Is.EqualTo(CommandStatus.Completed));
            }

            AssertFrameOrder(sent);

            // Nothing in the log may claim an ordering violation: the client raises that as an
            // error when a PING_REPLY overtakes its COMMAND_RESULT.
            Assert.That(_log.Contains("violates I-15"), Is.False);

            // Close cleanly so the server records CLIENT_CLOSED, which SC-50 checks in the DB.
            _client.Disconnect(ClientCloseReason.ClientClosed);
            PumpUntil(() => _transport.State == TransportState.Disconnected, 5000, "clean close");

            TestContext.WriteLine("SC-50 QUERY correlation_id=" + identity.CorrelationId.ToString("D"));
            TestContext.WriteLine("SC-50 QUERY session_id=" + identity.SessionId.ToString("D"));
        }

        /// <summary>
        /// I-15 checked on the raw frames rather than on the client's own bookkeeping: the
        /// bookkeeping is what we are trying to verify, so it cannot also be the evidence.
        /// </summary>
        void AssertFrameOrder(List<Guid> sent)
        {
            var results = new Dictionary<Guid, int>();
            var replies = new Dictionary<Guid, int>();
            var replyProbe = new Dictionary<Guid, uint>();

            for (int i = 0; i < _rawFrames.Count; i++)
            {
                JObject frame = ContractJson.ReadObject(_rawFrames[i]);
                string type = (string)frame["message_type"];
                JToken payload = frame["payload"];
                if (payload == null || payload["command_id"] == null) continue;

                Guid commandId = Guid.Parse((string)payload["command_id"]);
                if (type == "COMMAND_RESULT")
                {
                    Assert.That(results.ContainsKey(commandId), Is.False,
                        "exactly one COMMAND_RESULT per command_id (I-15)");
                    results[commandId] = i;
                    Assert.That((string)payload["status"], Is.EqualTo("ACCEPTED"));
                    Assert.That(payload["reason_code"].Type, Is.EqualTo(JTokenType.Null),
                        "ACCEPTED must carry a null reason_code (I-14)");
                    Assert.That(frame["correlation_id"].Type, Is.EqualTo(JTokenType.Null),
                        "COMMAND_RESULT.correlation_id is always null in this slice");
                }
                else if (type == "PING_REPLY")
                {
                    Assert.That(replies.ContainsKey(commandId), Is.False, "one PING_REPLY per command");
                    replies[commandId] = i;
                    replyProbe[commandId] = (uint)payload["probe_seq"];
                }
            }

            int checkedPairs = 0;
            for (int i = 0; i < sent.Count; i++)
            {
                Guid id = sent[i];
                Assert.That(results.ContainsKey(id), Is.True, "no COMMAND_RESULT for " + id);
                Assert.That(replies.ContainsKey(id), Is.True, "no PING_REPLY for " + id);
                Assert.That(results[id], Is.LessThan(replies[id]),
                    "COMMAND_RESULT must arrive before PING_REPLY for " + id + " (I-15)");
                Assert.That(replyProbe[id], Is.EqualTo((uint)i), "probe_seq mismatch for " + id);
                checkedPairs++;

                TestContext.WriteLine("frame order ok: COMMAND_RESULT[" + results[id] + "] -> PING_REPLY[" +
                                      replies[id] + "] for " + id.ToString("D") + " probe_seq=" + replyProbe[id]);
            }

            TestContext.WriteLine("command/reply pairs checked: " + checkedPairs);
            Assert.That(checkedPairs, Is.EqualTo(3));
        }

        // ------------------------------------------------------------------ SC-51

        [Test]
        public void Live_ServerInitiatedClose_ReconnectsAsANewSession()
        {
            _client.Connect(_url, _token);
            SessionIdentity first = WaitForReady(ReadyTimeoutMs);
            TestContext.WriteLine("SC-51 QUERY correlation_id=" + first.CorrelationId.ToString("D") + " (first)");

            // One command left in flight, so the "dropping N" line has a non-zero N and the
            // no-resend rule is actually exercised rather than vacuously true.
            Guid? inFlight = _client.SendPing();
            Assert.That(inFlight.HasValue, Is.True);

            // Make the SERVER close us. A client-initiated close would stop the reconnect loop
            // and prove nothing. Oversized frames are a protocol violation (ADR-0005 section
            // 2) and the server drops the connection once the budget is spent.
            string oversized = new string('x', OversizedFrameBytes);
            for (int i = 0; i < 12; i++) _transport.Send(oversized);

            PumpUntil(() => !_client.IsReady, ReadyTimeoutMs, "the server to close the first session");
            TestContext.WriteLine("first session closed by the server; attempt counter is now " + _client.Attempt);

            Assert.That(_log.Contains("starfall.net: dropping 1 in-flight command(s) on disconnect (no resend, I-23)"),
                Is.True, "the in-flight command must be failed locally and reported in one line");
            Assert.That(_client.Attempt, Is.EqualTo(1),
                "one failed session, so exactly one retry counted (reset happens only on SESSION_READY)");

            // The reconnect loop is supposed to come back on its own, after a jittered wait.
            SessionIdentity second = WaitForReady(ReconnectTimeoutMs, alreadyConnecting: true);
            TestContext.WriteLine("SC-51 QUERY correlation_id=" + second.CorrelationId.ToString("D") + " (second)");

            Assert.That(second.SessionId, Is.Not.EqualTo(first.SessionId),
                "a reconnect is a new session, not a resumed one (I-23)");
            Assert.That(second.CorrelationId, Is.Not.EqualTo(first.CorrelationId));
            Assert.That(_client.Attempt, Is.EqualTo(0), "SESSION_READY is the only reset point");

            // Nothing was re-sent: the new session must be able to round trip on its own.
            var outcomes = new List<CommandOutcome>();
            _client.Pending.Settled += outcomes.Add;
            Guid? fresh = _client.SendPing();
            Assert.That(fresh.HasValue, Is.True);
            PumpUntil(() => outcomes.Count == 1, RoundTripTimeoutMs, "a round trip on the new session");
            Assert.That(outcomes[0].Status, Is.EqualTo(CommandStatus.Completed));
            Assert.That(outcomes[0].CommandId, Is.Not.EqualTo(inFlight.Value),
                "the dropped command must not have been resent (I-23)");

            _client.Disconnect(ClientCloseReason.ClientClosed);
            PumpUntil(() => _transport.State == TransportState.Disconnected, 5000, "clean close");
        }

        // ------------------------------------------------------------------ helpers

        SessionIdentity WaitForReady(int timeoutMs, bool alreadyConnecting = false)
        {
            PumpUntil(() => _client.IsReady, timeoutMs,
                alreadyConnecting ? "the client to reconnect and become ready" : "SESSION_READY");
            return _client.Session.Value;
        }

        void PumpUntil(Func<bool> condition, int timeoutMs, string what)
        {
            DateTime deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);
            while (DateTime.UtcNow < deadline)
            {
                _client.Pump();
                if (condition()) return;
                Thread.Sleep(10);
            }

            foreach (string line in _log.Lines) TestContext.WriteLine(line);
            Assert.Fail("timed out after " + timeoutMs + " ms waiting for " + what);
        }

        static string TypeOf(string json)
        {
            JObject frame = ContractJson.ReadObject(json);
            string typeName;
            return ContractDispatch.TryGetTypeName(frame, out typeName) ? typeName : null;
        }
    }
}
