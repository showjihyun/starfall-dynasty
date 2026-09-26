// H-7 (T-4/T-5/T-6, architect R4 판정 section 8 / 11). Rewrite of the original R6 finding test.
//
// The FIRST draft of this file (see git history / 03_client_impl.md R6/R7) reimplemented
// GreyboxSession.Update()'s while loop inline, so it tested a COPY, not the real code - a
// change to the real loop could regress silently while this file stayed green (the shape this
// slice hit four times before this file, per architect's own count, and a fifth time here).
//
// This version calls Starfall.Flight.TickCatchUp.Compute directly - the SAME pure function
// GreyboxSession.Update() now calls (GreyboxSession.cs, T-3: no _tickAccumulator while-loop
// arithmetic remains there, verified by grep in CI/QA per SC-89 (c)). What this file adds beyond
// TickCatchUpTests.cs is the WIRE-LEVEL claim: driving Plan's output through the real
// RealtimeClient/FakeRealtimeTransport pairing to confirm sends actually landing on (or being
// withheld from) the transport, not just the Plan struct's numbers.

using System.Collections.Generic;
using System.Globalization;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Flight;
using Starfall.Net;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class GreyboxSendBurstTests
    {
        // ADR-0011 section 5.2 / S10: a session may submit at most this many commands in one
        // server tick before the tick is counted as a protocol violation.
        const int ServerPerTickCommandCap = 8;
        const double TickDuration = 1.0 / 20.0;

        FakeRealtimeTransport _transport;
        RealtimeClient _client;
        uint _localInputSeq = 1;

        [SetUp]
        public void SetUp()
        {
            _transport = new FakeRealtimeTransport();
            _client = new RealtimeClient(_transport, new RecordingLogSink(), jitterSeed: 20260923);

            _client.Connect("ws://127.0.0.1:8080/ws", "token");
            _transport.SimulateOpen();
            _client.Pump();

            _transport.SimulateMessage(SessionReadyFrame());
            _client.Pump();
            Assert.That(_client.IsReady, Is.True, "test setup: client must be ready before the burst it drives");
        }

        [TearDown]
        public void TearDown()
        {
            if (_client != null) _client.Dispose();
        }

        // ------------------------------------------------------------------ T-4/T-6: the original R4 evidence case, now driven through Plan + the real transport

        [Test]
        public void Update_AfterA460msHitch_MustNotBurstMoreThanOneTickOfSendsPerFrame()
        {
            const double hitchSeconds = 0.46; // 9.2 ticks - the R4 evidence hitch

            (int sendsThisFrame, int carryForwardTicksThisFrame) = SimulateOneFrame(hitchSeconds);

            TestContext.WriteLine("hitch_s=" + hitchSeconds.ToString(CultureInfo.InvariantCulture) +
                                  " sends_this_frame=" + sendsThisFrame +
                                  " carry_forward_ticks_this_frame=" + carryForwardTicksThisFrame +
                                  " server_per_tick_cap=" + ServerPerTickCommandCap);

            // §7b: "sendsThisFrame <= 1" is trivially satisfied if nothing was ever sent
            // (sendsThisFrame == 0, e.g. TrySend() silently failing every call) - that state is
            // reachable (a regression in RealtimeClient.TrySendJson could return false forever)
            // and would make this line pass while proving nothing about the K=1 cap. Paired
            // below with sendsThisFrame > 0: a 0.46s/9-tick hitch with K=1 must send exactly 1.
            Assert.That(sendsThisFrame, Is.LessThanOrEqualTo(1),
                "TickCatchUp.Plan sends only the last tick of a backlog (K=1, ADR-0012 section " +
                "6.2) - a hitch must never turn into a burst of sends regardless of its length");
            Assert.That(sendsThisFrame, Is.EqualTo(1),
                "pairing assertion (§7b): the hitch backlog (9 ticks) exceeds K=1, so exactly " +
                "one send must actually have happened - otherwise 'never more than 1' above is " +
                "true for the wrong reason (zero sends, not one throttled send)");
            // §7b: "_transport.Sent.Count == sendsThisFrame" is trivially satisfied at 0 == 0 -
            // now unreachable given the sendsThisFrame == 1 pairing directly above.
            Assert.That(_transport.Sent.Count, Is.EqualTo(sendsThisFrame),
                "every 'sent' tick must have actually gone on the wire with no client-side " +
                "throttling - confirms this is a wire-level claim, not just a local count");

            // T-6 (R3 pairing discipline): without this, a Plan that (bug) predicted but never
            // reported any carry-forward would make "sends stayed <= 1" vacuously true for the
            // wrong reason (nothing was predicted at all, not "9 ticks were predicted and only
            // 1 sent"). This is exactly the pairing SC-89 (b) requires of the real session.
            Assert.That(carryForwardTicksThisFrame, Is.GreaterThan(0),
                "the hitch must actually have produced carry-forward prediction - otherwise " +
                "the send-count assertion above proves nothing happened, not that it was handled correctly");
        }

        // ------------------------------------------------------------------ T-5: property tests, random frame deltas including a hitch

        [Test]
        public void PropertyTest_RandomFrameDeltasIncludingA460msHitch_HoldsAllThreeInvariants()
        {
            var random = new System.Random(20260923);
            double accumulator = 0.0;
            int totalSends = 0;
            int totalPredicted = 0;
            long totalCarryForwardOrDormant = 0;
            double wallClockSeconds = 0.0;
            bool hitchInjected = false;

            while (wallClockSeconds < 10.0)
            {
                double frameDelta = wallClockSeconds > 4.0 && !hitchInjected
                    ? 0.46
                    : 0.010 + random.NextDouble() * 0.010;
                if (frameDelta >= 0.46) hitchInjected = true;

                wallClockSeconds += frameDelta;
                accumulator += frameDelta;

                TickCatchUp.Plan plan = TickCatchUp.Compute(
                    accumulator, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);
                accumulator = plan.RemainingAccumulatorSeconds;

                int sendsThisFrame = DriveThroughTransport(plan);
                int carryForwardTicksThisFrame = plan.TicksToPredict - plan.TicksToSend;

                // §7b: trivially satisfied at sendsThisFrame == 0 every frame (nothing ever
                // sent) - paired below via the run-total totalSends > 0 check, and per-frame
                // via invariant 3's use of the actually-observed sendsThisFrame (not a plan
                // field) so a silent-send-failure regression shows up there too.
                Assert.That(sendsThisFrame, Is.LessThanOrEqualTo(1), "invariant 1: never more than K=1 send per frame");

                // T-5 fix (was tautological): the original line compared
                // "plan.TicksToPredict - plan.TicksToSend + plan.TicksToSend" to
                // "plan.TicksToPredict" - both sides reduce to the same Plan field
                // algebraically, so this held for ANY Plan.Compute output, correct or not
                // (confirmed: injecting a defect into TickCatchUp.Compute that lets TicksToSend
                // exceed TicksToPredict left this line green). The invariant SC-89 (a)③ actually
                // targets is that ticks EXECUTED as carry-forward plus ticks EXECUTED as real
                // wire sends account for every predicted tick - so the right-hand side of the
                // send term must be the wire-observed sendsThisFrame from DriveThroughTransport
                // above, not plan.TicksToSend read back out of the same struct.
                Assert.That(carryForwardTicksThisFrame + sendsThisFrame, Is.EqualTo(plan.TicksToPredict),
                    "invariant 3: carry-forward ticks + ACTUALLY SENT ticks == predicted ticks");

                totalSends += sendsThisFrame;
                totalPredicted += plan.TicksToPredict;
                totalCarryForwardOrDormant += carryForwardTicksThisFrame;
            }

            Assert.That(hitchInjected, Is.True, "test setup: the hitch must actually have run");

            // §7b pairing for invariant 1: without this, "sendsThisFrame <= 1" every frame is
            // trivially satisfied by never sending anything over the whole 10s run.
            Assert.That(totalSends, Is.GreaterThan(0),
                "pairing assertion (§7b): the run must actually have sent something over the " +
                "wire, or invariant 1 holding every frame proves nothing");

            double expectedTicks = wallClockSeconds / TickDuration;
            Assert.That(totalPredicted, Is.EqualTo(expectedTicks).Within(1),
                "invariant 2: predicted ticks track wall-clock time to within ±1 tick over the run");

            // T-6 pairing for the whole run, same discipline as the single-frame test above.
            Assert.That(totalCarryForwardOrDormant, Is.GreaterThan(0),
                "paired assertion (T-6/architect R3 discipline): the injected hitch must have " +
                "produced at least one carry-forward tick, or invariant 1 holding is meaningless");

            TestContext.WriteLine("wall_clock_s=" + wallClockSeconds + " total_sends=" + totalSends +
                                  " total_predicted=" + totalPredicted + " total_carry_forward_or_dormant=" + totalCarryForwardOrDormant);
        }

        // ------------------------------------------------------------------ helpers

        /// <summary>Runs one Plan through the real transport, sending only the last
        /// plan.TicksToSend of plan.TicksToPredict ticks - mirrors GreyboxSession.Update()'s
        /// adapter loop exactly (which tick indices are "send slots"), but does not touch
        /// PredictedShipController: prediction correctness is covered by TickCatchUpTests,
        /// RebaseHoldTests and PredictionHistoryTests in isolation. This file's job is only the
        /// wire-level send count.</summary>
        (int SendsThisFrame, int CarryForwardTicksThisFrame) SimulateOneFrame(double hitchSeconds)
        {
            TickCatchUp.Plan plan = TickCatchUp.Compute(
                hitchSeconds, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);
            int sends = DriveThroughTransport(plan);
            return (sends, plan.TicksToPredict - plan.TicksToSend);
        }

        int DriveThroughTransport(TickCatchUp.Plan plan)
        {
            int sends = 0;
            for (int i = 0; i < plan.TicksToPredict; i++)
            {
                bool isSendSlot = i >= plan.TicksToPredict - plan.TicksToSend;
                if (isSendSlot && TrySend()) sends++;
            }
            return sends;
        }

        bool TrySend()
        {
            uint seq = _localInputSeq++;
            SetShipControlCommand command = SetShipControlBuilder.Build(
                UuidV7.NewGuid(), seq, new Vec3d(0.0, 0.0, 1.0), roll: 0.0, aimWorldTarget: Quatd.Identity,
                brake: false, flightAssist: true, clientSentAtIso8601OrNull: null);
            string json = ContractJson.Serialize(command);
            return _client.TrySendJson(json);
        }

        static string SessionReadyFrame() =>
            "{\"message_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"message_type\":\"SESSION_READY\",\"schema_version\":1,\"tick\":1200," +
            "\"correlation_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"payload\":{\"session_id\":\"" + UuidV7.NewGuid().ToString("D") + "\"," +
            "\"world_id\":\"01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b\"," +
            "\"actor_id\":\"01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4c\"," +
            "\"tick_hz\":20,\"server_version\":\"0.1.0\"}}";
    }
}
