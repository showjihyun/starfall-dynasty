// C4. Reconciliation and the retained-input history (ADR-0012 section 3).
// Reconcile_SyntheticReplay_... exercises the Reconciliation.Reconcile/PredictedShipController
// machinery itself (round-trip prediction-then-correction semantics) with a hand-built input
// sequence. Reconcile_RealS6Replay_... below is a SEPARATE, complementary check: it replays the
// actual S6 determinism fixture (server/crates/sim/tests/data/replay/, produced by
// `cargo test -p starfall-sim --test determinism`) and compares the C# integrator's output
// directly against the Rust integrator's recorded output - the "same bits" assumption
// (ADR-0010 section 3) that SC-51/52 actually exist to catch. It does not go through
// Reconciliation.Reconcile: that function's retained-history model assumes one freshly-sent
// input per tick (a live client resends every tick, ADR-0012 section 6), but this fixture's
// inputs.jsonl is sparse - the server's carry-forward path (carry_forward_max_ticks=10_000 in
// this fixture, see determinism.rs) re-applies the SAME input_seq for hundreds of ticks in a
// row. Feeding that through Reconcile's "first history entry matching ack_input_seq" lookup
// would compare against a one-tick-stale predicted state instead of the freshest one - not a
// bug in Reconcile (a real 20 Hz client never holds one input_seq that long), just a mismatch
// between this fixture's representation and Reconcile's precondition. A direct
// ShipIntegrator.Step comparison at each snapshot-arrival tick measures exactly what SC-51/52
// ask for ("재조정 직전 위치/자세 오차") without relying on that precondition.
//
// Thresholds are constructed by hand in these tests, never read from data/ (sprint contract
// section 0.7): a designer retune of reconcile_ignore_threshold_m must not turn this suite red
// for a reason that has nothing to do with reconciliation logic.

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using NUnit.Framework;
using Starfall.Contracts.Generated;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconciliationTests
    {
        const double Dt = 1.0 / 20.0;

        static ShipClassStats LoadScoutFixture() =>
            ShipClassStats.FromJson(ContractFixtures.RequireValid("SHIP_CLASS", "example-scout.json").ReadText());

        static readonly ShipIntegrator.Boundary WideOpenBoundary =
            new ShipIntegrator.Boundary(softRadiusM: 10_000.0, hardRadiusM: 12_000.0, pullMps2: 25.0);

        static SyncTuningData TestTuning() => new SyncTuningData
        {
            ReconcileIgnoreThresholdM = 0.005,
            ReconcileSmoothThresholdM = 0.25,
            ReconcileSmoothDurationMs = 200,
            ReconcileHardSnapThresholdM = 5.0,
            ReconcileOrientationIgnoreThresholdDeg = 0.02,
            ReconcileOrientationSmoothThresholdDeg = 1.0,
            ReconcileOrientationHardSnapDeg = 15.0,
        };

        static ShipControlInputD ForwardThrust(uint seq) => new ShipControlInputD(
            seq, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true);

        /// <summary>A turning input: target attitude 90 degrees about world Y, plus a nonzero
        /// manual roll - so both omega_aim (yaw toward the target) and omega_roll (manual roll)
        /// are simultaneously nonzero and NOT related by any fixed axis, which is exactly the
        /// shape SC-55 needs (ADR-0010 section 1.1: "omega_aim은 전방축 성분을 갖지 않는다는
        /// 보장이 없다" - here it provably has significant off-axis content).</summary>
        static ShipControlInputD TurningInput(uint seq) => new ShipControlInputD(
            seq, 0, 0, 500, /* roll */ 700,
            /* aim: 90 deg about Y = (0, sin45, 0, cos45) */ 0, 707_107, 0, 707_107,
            false, true);

        // ------------------------------------------------------------------ helper: drive a
        // "ground truth" trajectory (stand-in for the server) and a client predictor with the
        // SAME input sequence, round-tripping the ground truth through wire quantisation at
        // each simulated snapshot tick (so residual error is exactly the quantisation noise a
        // real WORLD_SNAPSHOT would introduce, nothing more).

        static WorldSnapshotMessage.WorldSnapshotPayload.ShipState ToWireShipState(ShipSimState state)
        {
            return new WorldSnapshotMessage.WorldSnapshotPayload.ShipState
            {
                ShipId = Guid.Empty,
                ActorId = Guid.Empty,
                ShipClassId = "scout-s01",
                Presence = "ACTIVE",
                PositionXMm = Quantization.QuantizePosition(state.Position.X),
                PositionYMm = Quantization.QuantizePosition(state.Position.Y),
                PositionZMm = Quantization.QuantizePosition(state.Position.Z),
                VelocityXMmS = (int)Quantization.QuantizeVelocity(state.Velocity.X),
                VelocityYMmS = (int)Quantization.QuantizeVelocity(state.Velocity.Y),
                VelocityZMmS = (int)Quantization.QuantizeVelocity(state.Velocity.Z),
                OrientationXMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.X),
                OrientationYMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.Y),
                OrientationZMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.Z),
                OrientationWMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.W),
                AngularVelocityXMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.X),
                AngularVelocityYMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.Y),
                AngularVelocityZMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.Z),
                AngularVelocityRollMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityRoll),
            };
        }

        // ------------------------------------------------------------------ C4 headline test (synthetic stand-in for S6)

        [Test]
        public void Reconcile_SyntheticReplay_PositionErrorWithinIgnoreThreshold_IncludingATurningSnapshot()
        {
            ShipClassStats ship = LoadScoutFixture();
            SyncTuningData tuning = TestTuning();

            ShipSimState groundTruth = ShipSimState.Zero;
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);

            const int SnapshotIntervalTicks = 2; // matches sync-tuning's snapshot_interval_ticks = tick_hz/snapshot_hz = 20/10
            uint? ackInputSeq = null;
            int reconciliationsChecked = 0;
            double maxPositionErrorM = 0.0;
            double maxOrientationErrorDeg = 0.0;
            bool sawTurningReconciliation = false;

            for (uint tick = 1; tick <= 60; tick++)
            {
                // Ticks 1-20 straight, 21-40 turning (SC-55's "선회 중 스냅샷"), 41-60 straight again.
                ShipControlInputD input = (tick > 20 && tick <= 40) ? TurningInput(tick) : ForwardThrust(tick);

                groundTruth = ShipIntegrator.Step(groundTruth, input, ship, WideOpenBoundary, Dt).State;
                controller.ApplyInput(input);

                if (tick % SnapshotIntervalTicks != 0) continue;

                // A snapshot "arrives": ground truth round-tripped through wire quantisation,
                // ack_input_seq = the tick just applied (server applied exactly one input/tick).
                ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(groundTruth));
                ackInputSeq = tick;

                Reconciliation.Result result = controller.Reconcile(confirmed, ackInputSeq, tuning);
                if (!result.HasError) continue; // first snapshot after connect has nothing to compare yet

                reconciliationsChecked++;
                maxPositionErrorM = Math.Max(maxPositionErrorM, result.PositionErrorM);
                maxOrientationErrorDeg = Math.Max(maxOrientationErrorDeg, result.OrientationErrorDeg);
                if (tick > 20 && tick <= 40) sawTurningReconciliation = true;

                TestContext.WriteLine("tick " + tick + " : posErr=" + result.PositionErrorM + " m, orientErr=" +
                                      result.OrientationErrorDeg + " deg, turning=" + (tick > 20 && tick <= 40));

                Assert.That(result.PositionErrorM, Is.LessThanOrEqualTo(tuning.ReconcileIgnoreThresholdM),
                    "tick " + tick + ": reconciliation error exceeded the ignore threshold - this is either a " +
                    "quantisation-noise floor bug or (per task doc) the two integrators disagree. Do not raise " +
                    "the threshold; report to architect.");
                Assert.That(result.OrientationErrorDeg, Is.LessThanOrEqualTo(tuning.ReconcileOrientationIgnoreThresholdDeg),
                    "tick " + tick + ": orientation error exceeded the ignore threshold");
            }

            TestContext.WriteLine("reconciliations checked: " + reconciliationsChecked +
                                  ", max position error: " + maxPositionErrorM + " m" +
                                  ", max orientation error: " + maxOrientationErrorDeg + " deg" +
                                  ", included a turning snapshot: " + sawTurningReconciliation);

            Assert.That(reconciliationsChecked, Is.GreaterThan(0), "the loop must actually reconcile at least once");
            Assert.That(sawTurningReconciliation, Is.True,
                "AC-12(e)/SC-55: the replay MUST include a reconciliation taken while turning (both omega_aim " +
                "and omega_roll nonzero) - a straight-line-only replay cannot catch a decomposed-from-sum bug");
        }

        /// <summary>One line of server/crates/sim/tests/data/replay/inputs.jsonl
        /// ("{tick, session, payload}" - determinism.rs's InputLine, S8 §5 "S6 산출물 형식").</summary>
        sealed class ReplayInputLine
        {
            [JsonProperty("tick")] public long Tick { get; set; }
            [JsonProperty("session")] public Guid Session { get; set; }
            [JsonProperty("payload")] public SetShipControlCommand.SetShipControlPayload Payload { get; set; }
        }

        /// <summary>The shape of initial.json (determinism.rs's InitialWorld): tick 0, three
        /// ships, wire-shaped fields (reuses the generated WORLD_SNAPSHOT ShipState - determinism.rs
        /// serializes the SAME starfall_contracts::ShipState type this DTO was generated from).</summary>
        sealed class ReplayInitialWorld
        {
            [JsonProperty("tick")] public long Tick { get; set; }
            [JsonProperty("star_system_id")] public string StarSystemId { get; set; }
            [JsonProperty("ships")] public WorldSnapshotMessage.WorldSnapshotPayload.ShipState[] Ships { get; set; }
        }

        /// <summary>Boundary pull constant for THIS replay fixture (determinism.rs `world()`:
        /// boundary_pull_mps2 = 25.0). Not a designer-tunable gameplay number read from data/
        /// (gate G-b) - it is a fixed property of the S6 test fixture itself, the same category
        /// as the fixture's hardcoded spawn point and initial headings. soft/hard radii, by
        /// contrast, ARE read from the fixture's own wire data below, since those happen to be
        /// on the wire (WorldSnapshotPayload.*BoundaryRadiusMm) and reading them beats copying
        /// them by hand a second time.</summary>
        const double ReplayBoundaryPullMps2 = 25.0;

        /// <summary>Reads a file that a concurrently-running `cargo test -p starfall-sim
        /// --test determinism` may be rewriting (sprint contract note: the determinism test
        /// regenerates this directory's contents byte-identically on every run, but a read
        /// mid-write can still observe a torn file). Retries on I/O or JSON parse failure.</summary>
        static string ReadReplayFileWithRetry(string path, int maxAttempts = 8, int delayMs = 250)
        {
            Exception last = null;
            for (int attempt = 1; attempt <= maxAttempts; attempt++)
            {
                try
                {
                    string text = File.ReadAllText(path);
                    if (string.IsNullOrWhiteSpace(text))
                        throw new IOException("file was empty (likely read mid-write)");
                    return text;
                }
                catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
                {
                    last = ex;
                    System.Threading.Thread.Sleep(delayMs);
                }
            }
            throw new IOException(
                "Could not read " + path + " after " + maxAttempts + " retries - server ("
                + "cargo test -p starfall-sim --test determinism) may be regenerating this " +
                "directory concurrently (sprint contract gate G-k). Last error: " + last?.Message, last);
        }

        static double Percentile(List<double> sortedAscending, double p)
        {
            if (sortedAscending.Count == 0) return 0.0;
            int index = (int)Math.Ceiling(p * sortedAscending.Count) - 1;
            if (index < 0) index = 0;
            if (index >= sortedAscending.Count) index = sortedAscending.Count - 1;
            return sortedAscending[index];
        }

        [Test]
        public void Reconcile_RealS6Replay_PositionAndOrientationErrorWithinIgnoreThreshold()
        {
            string repoRoot = ContractFixtures.RequireRepoRoot();
            string replayDir = Path.Combine(repoRoot, "server", "crates", "sim", "tests", "data", "replay");
            string regenCommand = "cd server && cargo test -p starfall-sim --test determinism";

            // ADR-0010 section 3.1 (architect decision): this directory is tracked in git.
            // Absence is a BROKEN CHECKOUT, not "server has not produced it yet" - a normal
            // checkout always has it. Never Assert.Ignore/Inconclusive here: an Inconclusive
            // result is exactly what made `unity test` exit non-zero for this test before
            // (SC-46), and a missing tracked file is a real failure, not an environment gap.
            if (!Directory.Exists(replayDir))
            {
                Assert.Fail("S6 replay directory not found at " + replayDir + " - this directory is " +
                            "tracked in git (ADR-0010 section 3.1), so its absence means a broken " +
                            "checkout, not a pending implementation. Verify the checkout, or regenerate " +
                            "(the file is byte-identical either way) with: " + regenCommand);
                return;
            }
            foreach (string fileName in new[] { "initial.json", "inputs.jsonl", "snapshots.jsonl" })
            {
                string path = Path.Combine(replayDir, fileName);
                if (!File.Exists(path))
                {
                    Assert.Fail("S6 replay directory exists but " + path + " is missing - tracked file " +
                                "absent from a checkout that has the directory is likely a partial " +
                                "checkout or a bad merge, not a pending implementation. Regenerate with: " +
                                regenCommand);
                    return;
                }
            }

            var initial = JsonConvert.DeserializeObject<ReplayInitialWorld>(
                ReadReplayFileWithRetry(Path.Combine(replayDir, "initial.json")));

            var inputLines = ReadReplayFileWithRetry(Path.Combine(replayDir, "inputs.jsonl"))
                .Split('\n')
                .Select(line => line.Trim())
                .Where(line => line.Length > 0)
                .Select(line => JsonConvert.DeserializeObject<ReplayInputLine>(line))
                .ToList();

            var snapshotLines = ReadReplayFileWithRetry(Path.Combine(replayDir, "snapshots.jsonl"))
                .Split('\n')
                .Select(line => line.Trim())
                .Where(line => line.Length > 0)
                .Select(line => JsonConvert.DeserializeObject<WorldSnapshotMessage.WorldSnapshotPayload>(line))
                .ToList();

            Assert.That(snapshotLines.Count, Is.GreaterThan(1), "need at least one real snapshot beyond tick 0");
            Assert.That(snapshotLines[0].ControlledShipId.HasValue, Is.True, "snapshots.jsonl's first line must name the controlled ship");
            Guid controlledShipId = snapshotLines[0].ControlledShipId.Value;

            WorldSnapshotMessage.WorldSnapshotPayload.ShipState controlledInitialWire =
                initial.Ships.FirstOrDefault(s => s.ShipId == controlledShipId);
            Assert.That(controlledInitialWire, Is.Not.Null,
                "initial.json has no ship matching snapshots.jsonl's controlled_ship_id=" + controlledShipId);

            // Which of inputs.jsonl's sessions drove the controlled ship: this fixture's own
            // session (determinism.rs "alpha") is the one the snapshot stream was recorded
            // from, and it is deliberately given the busiest maneuver plan (thrust, brake, two
            // turns, a manual roll, boundary contact - AC-8's full path coverage), while the
            // other two ships get 3 and 1 commands respectively. Picking the session with the
            // most lines needs no fixture-specific UUID knowledge, and it is self-checking: a
            // wrong pick would fail the error-threshold assertions below immediately and
            // loudly, not silently pass.
            List<ReplayInputLine> controlledSessionLines = inputLines
                .GroupBy(l => l.Session)
                .OrderByDescending(g => g.Count())
                .First()
                .OrderBy(l => l.Tick)
                .ToList();
            Assert.That(controlledSessionLines.Count, Is.GreaterThan(0));
            Assert.That(controlledSessionLines[0].Tick, Is.EqualTo(1),
                "the controlled session's first command must arrive at tick 1 (determinism.rs's maneuver_plan)");

            ShipClassStats ship = LoadScoutFixture();
            double softRadiusM = snapshotLines[0].SoftBoundaryRadiusMm / 1000.0;
            double hardRadiusM = snapshotLines[0].HardBoundaryRadiusMm / 1000.0;
            var boundary = new ShipIntegrator.Boundary(softRadiusM, hardRadiusM, ReplayBoundaryPullMps2);
            int snapshotIntervalTicks = snapshotLines[0].SnapshotIntervalTicks;
            Assert.That(snapshotIntervalTicks, Is.GreaterThan(0));

            ShipSimState state = ShipStateWire.ToSimState(controlledInitialWire);

            int inputIndex = 0;
            ShipControlInputD activeInput = default;
            bool haveInput = false;

            var positionErrorsM = new List<double>();
            var orientationErrorsDeg = new List<double>();
            // AC-12(f) (architect ruling, QA round 2 / CL-1): recorded and reported, never
            // thresholded - there is no tuned tolerance for angular velocity error in this
            // slice, and inventing one here would plant an unjustified gate (architect: "임계값을
            // 만들지 마라"). What IS asserted is coverage, below: that this replay actually
            // exercises a snapshot where both fields are meaningfully nonzero at once.
            var angularVelocityAimErrorsDegS = new List<double>();
            var angularVelocityRollErrorsDegS = new List<double>();
            bool sawNonZeroOmegaAimAndRoll = false;
            int comparedPoints = 0;

            long lastTick = (snapshotLines.Count - 1) * (long)snapshotIntervalTicks;
            for (long tick = 1; tick <= lastTick; tick++)
            {
                while (inputIndex < controlledSessionLines.Count && controlledSessionLines[inputIndex].Tick <= tick)
                {
                    activeInput = SetShipControlBuilder.ToDequantizedInput(controlledSessionLines[inputIndex].Payload);
                    haveInput = true;
                    inputIndex++;
                }
                Assert.That(haveInput, Is.True, "tick " + tick + ": no active input yet - the controlled session's first command should have carried forward from tick 1");

                state = ShipIntegrator.Step(state, activeInput, ship, boundary, Dt).State;

                if (tick % snapshotIntervalTicks != 0) continue;

                int lineIndex = (int)(tick / snapshotIntervalTicks);
                WorldSnapshotMessage.WorldSnapshotPayload snapshot = snapshotLines[lineIndex];
                WorldSnapshotMessage.WorldSnapshotPayload.ShipState confirmedWire =
                    snapshot.Ships.FirstOrDefault(s => s.ShipId == controlledShipId);
                if (confirmedWire == null) continue; // controlled ship not present this tick (not expected in this fixture, but not this test's concern if it happens)

                ShipSimState confirmed = ShipStateWire.ToSimState(confirmedWire);
                double positionErrorM = (state.Position - confirmed.Position).Length();
                double orientationErrorDeg = Quatd.AngleDegrees(state.Orientation, confirmed.Orientation);
                // AC-12(f)/SC-55: the two angular velocity fields, compared separately - a
                // wire-to-sim path that drops or conflates ω_aim/ω_roll (B-1's "합에서 분해" bug)
                // would show up here as a growing error even though position/orientation stay
                // tight (position/orientation do not depend on angular velocity being read
                // correctly, only on it being INTEGRATED correctly from whatever came out of
                // ToSimState - so §1's position/orientation check alone cannot catch this).
                double angularVelocityAimErrorDegS = (state.AngularVelocityAim - confirmed.AngularVelocityAim).Length();
                double angularVelocityRollErrorDegS = Math.Abs(state.AngularVelocityRoll - confirmed.AngularVelocityRoll);

                comparedPoints++;
                positionErrorsM.Add(positionErrorM);
                orientationErrorsDeg.Add(orientationErrorDeg);
                angularVelocityAimErrorsDegS.Add(angularVelocityAimErrorDegS);
                angularVelocityRollErrorsDegS.Add(angularVelocityRollErrorDegS);

                // Same "meaningfully nonzero" bar the synthetic TurningInput test uses (1 deg/s).
                // determinism.rs's roll segment (ticks ~460-500: a manual roll while the ship is
                // still mid-turn from tick 450's facing_a) is the only place in this replay both
                // fields are nonzero together - if wire->sim ever drops one of them, EVERY
                // confirmed snapshot has at least one field pinned at zero and this never fires.
                if (confirmed.AngularVelocityAim.Length() > 1.0 && Math.Abs(confirmed.AngularVelocityRoll) > 1.0)
                    sawNonZeroOmegaAimAndRoll = true;

                Assert.That(positionErrorM, Is.LessThanOrEqualTo(TestTuning().ReconcileIgnoreThresholdM),
                    "tick " + tick + ": C# integrator position diverged from the real S6 (Rust) snapshot by " +
                    positionErrorM + " m - do not raise this threshold, report to architect " +
                    "(ADR-0010 section 3's \"same bits\" assumption may be violated)");
                Assert.That(orientationErrorDeg, Is.LessThanOrEqualTo(TestTuning().ReconcileOrientationIgnoreThresholdDeg),
                    "tick " + tick + ": C# integrator orientation diverged from the real S6 (Rust) snapshot by " +
                    orientationErrorDeg + " deg - do not raise this threshold, report to architect");
            }

            positionErrorsM.Sort();
            orientationErrorsDeg.Sort();
            angularVelocityAimErrorsDegS.Sort();
            angularVelocityRollErrorsDegS.Sort();
            TestContext.WriteLine("[SC-51/52] real S6 replay: compared points=" + comparedPoints +
                                  ", position error p99=" + Percentile(positionErrorsM, 0.99) +
                                  " m, max=" + (positionErrorsM.Count > 0 ? positionErrorsM[positionErrorsM.Count - 1] : 0.0) +
                                  " m, orientation error p99=" + Percentile(orientationErrorsDeg, 0.99) +
                                  " deg, max=" + (orientationErrorsDeg.Count > 0 ? orientationErrorsDeg[orientationErrorsDeg.Count - 1] : 0.0) + " deg");
            TestContext.WriteLine("[AC-12(f)] real S6 replay angular velocity error (reported only, no threshold - " +
                                  "no tuned tolerance exists for this in the current slice): omega_aim p99=" +
                                  Percentile(angularVelocityAimErrorsDegS, 0.99) + " deg/s, max=" +
                                  (angularVelocityAimErrorsDegS.Count > 0 ? angularVelocityAimErrorsDegS[angularVelocityAimErrorsDegS.Count - 1] : 0.0) +
                                  " deg/s, omega_roll p99=" + Percentile(angularVelocityRollErrorsDegS, 0.99) + " deg/s, max=" +
                                  (angularVelocityRollErrorsDegS.Count > 0 ? angularVelocityRollErrorsDegS[angularVelocityRollErrorsDegS.Count - 1] : 0.0) + " deg/s");

            Assert.That(comparedPoints, Is.GreaterThan(0), "the replay must actually compare at least one snapshot");
            Assert.That(sawNonZeroOmegaAimAndRoll, Is.True,
                "AC-12(f)/SC-55: no compared snapshot had both omega_aim and omega_roll meaningfully " +
                "(>1 deg/s) nonzero at once. This replay's roll segment (determinism.rs ticks ~460-500) " +
                "exists specifically to create one. If this fails, ShipStateWire.ToSimState (or the wire " +
                "payload itself) is dropping or conflating one of the two angular velocity fields - " +
                "exactly the B-1 \"합에서 분해\" bug SC-55 exists to catch.");
        }

        // ------------------------------------------------------------------ SC-53: purity

        [Test]
        public void Reconcile_IsPureFunction_SameInputsTwice_SameOutput()
        {
            ShipClassStats ship = LoadScoutFixture();

            var history = new List<InputRecord>();
            ShipSimState predicted = ShipSimState.Zero;
            for (uint seq = 1; seq <= 10; seq++)
            {
                (predicted, history) = PredictionHistory.ApplyInput(history, predicted, ForwardThrust(seq), ship, WideOpenBoundary, Dt);
            }

            ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(ShipIntegrator.Step(
                ShipIntegrator.Step(ShipSimState.Zero, ForwardThrust(1), ship, WideOpenBoundary, Dt).State,
                ForwardThrust(2), ship, WideOpenBoundary, Dt).State));

            Reconciliation.Result first = Reconciliation.Reconcile(history, confirmed, 2, ship, WideOpenBoundary, Dt);
            Reconciliation.Result second = Reconciliation.Reconcile(history, confirmed, 2, ship, WideOpenBoundary, Dt);

            TestContext.WriteLine("run 1: p=" + first.State.Position + " err=" + first.PositionErrorM);
            TestContext.WriteLine("run 2: p=" + second.State.Position + " err=" + second.PositionErrorM);

            Assert.That(second.State.Position, Is.EqualTo(first.State.Position));
            Assert.That(second.State.Velocity, Is.EqualTo(first.State.Velocity));
            Assert.That(second.State.Orientation, Is.EqualTo(first.State.Orientation));
            Assert.That(second.PositionErrorM, Is.EqualTo(first.PositionErrorM));
            Assert.That(second.OrientationErrorDeg, Is.EqualTo(first.OrientationErrorDeg));
            Assert.That(second.RetainedHistory.Count, Is.EqualTo(first.RetainedHistory.Count));

            // Also: calling it twice must not have mutated the ORIGINAL history passed in.
            Assert.That(history.Count, Is.EqualTo(10), "Reconcile must not mutate its history argument");
        }

        [Test]
        public void Reconcile_DoesNotDependOnClockOrCallOrder()
        {
            // A second, stronger phrasing of purity: interleave unrelated work between the two
            // calls (allocate garbage, sleep-free busy loop) - a function reading any ambient
            // clock/timer would be tempted to leak through here. It must not.
            ShipClassStats ship = LoadScoutFixture();
            var history = new List<InputRecord>();
            ShipSimState predicted = ShipSimState.Zero;
            for (uint seq = 1; seq <= 5; seq++)
                (predicted, history) = PredictionHistory.ApplyInput(history, predicted, TurningInput(seq), ship, WideOpenBoundary, Dt);

            ShipSimState confirmed = ShipIntegrator.Step(ShipSimState.Zero, TurningInput(1), ship, WideOpenBoundary, Dt).State;

            Reconciliation.Result before = Reconciliation.Reconcile(history, confirmed, 1, ship, WideOpenBoundary, Dt);

            // Busy work between calls, deliberately including allocation and a different thread
            // of local variables in scope - none of it is a parameter to Reconcile.
            var scratch = new List<int>();
            for (int i = 0; i < 100_000; i++) scratch.Add(i);
            System.Threading.Thread.Sleep(5);

            Reconciliation.Result after = Reconciliation.Reconcile(history, confirmed, 1, ship, WideOpenBoundary, Dt);

            Assert.That(after.State.Position, Is.EqualTo(before.State.Position));
            Assert.That(after.PositionErrorM, Is.EqualTo(before.PositionErrorM));
        }

        // ------------------------------------------------------------------ SC-54: skipped input_seq

        [Test]
        public void Reconcile_HistoryWithASkippedInputSeq_StillConvergesToServerState()
        {
            // Local history has 1, 2, 4, 5 (3 is missing - dropped locally as if REJECTED, or
            // simply never retained). Ground truth is built from the SAME four inputs applied
            // in order (skipping 3 entirely, since it never reached the server either in this
            // scenario) - reconciliation must still converge, because it never assumes
            // contiguous input_seq, only ascending order among what IS retained.
            ShipClassStats ship = LoadScoutFixture();
            uint[] seqsSent = { 1, 2, 4, 5 };

            ShipSimState groundTruth = ShipSimState.Zero;
            var history = new List<InputRecord>();
            ShipSimState predicted = ShipSimState.Zero;
            foreach (uint seq in seqsSent)
            {
                ShipControlInputD input = TurningInput(seq);
                groundTruth = ShipIntegrator.Step(groundTruth, input, ship, WideOpenBoundary, Dt).State;
                (predicted, history) = PredictionHistory.ApplyInput(history, predicted, input, ship, WideOpenBoundary, Dt);
            }

            ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(groundTruth));
            Reconciliation.Result result = Reconciliation.Reconcile(history, confirmed, 5, ship, WideOpenBoundary, Dt);

            TestContext.WriteLine("skipped-seq reconcile: posErr=" + result.PositionErrorM + " retained=" + result.RetainedHistory.Count);
            Assert.That(result.HasError, Is.True);
            Assert.That(result.PositionErrorM, Is.LessThanOrEqualTo(0.005), "must converge to (quantisation-noise-level of) the server state despite the gap at seq=3");
            Assert.That(result.RetainedHistory.Count, Is.EqualTo(0), "all four retained inputs were <= ack_input_seq=5 and must be dropped");
        }

        // ------------------------------------------------------------------ SC-55: two angular velocity fields, not a sum

        [Test]
        public void Reconcile_TurningSnapshot_RestoresBothAngularVelocitiesSeparately_NotFromASum()
        {
            ShipClassStats ship = LoadScoutFixture();

            // Drive to a state where omega_aim and omega_roll are BOTH well away from zero and
            // NOT related by any simple projection (several ticks of TurningInput).
            ShipSimState state = ShipSimState.Zero;
            for (uint seq = 1; seq <= 15; seq++)
                state = ShipIntegrator.Step(state, TurningInput(seq), ship, WideOpenBoundary, Dt).State;

            Assert.That(state.AngularVelocityAim.Length(), Is.GreaterThan(1.0), "test setup: omega_aim must be meaningfully nonzero");
            Assert.That(Math.Abs(state.AngularVelocityRoll), Is.GreaterThan(1.0), "test setup: omega_roll must be meaningfully nonzero");

            WorldSnapshotMessage.WorldSnapshotPayload.ShipState wire = ToWireShipState(state);

            // Correct path: the two fields are read separately (ShipStateWire.ToSimState).
            ShipSimState correct = ShipStateWire.ToSimState(wire);
            Assert.That(NearlyEqual(correct.AngularVelocityAim, state.AngularVelocityAim), Is.True,
                "omega_aim must round-trip through the wire's two fields within quantisation noise");
            Assert.That(correct.AngularVelocityRoll, Is.EqualTo(state.AngularVelocityRoll).Within(0.001));

            // Wrong path, deliberately reconstructed here to PROVE it is wrong: decompose a
            // single combined angular velocity (as if the wire had only carried the vector sum)
            // by projecting onto the ship's forward axis for roll and subtracting for aim -
            // the "손실 분해" ADR-0010 section 1.1 and B-1 warn about.
            Vec3d fwd = correct.Orientation.Rotate(new Vec3d(0, 0, 1));
            Vec3d combined = correct.AngularVelocityAim + fwd * correct.AngularVelocityRoll;
            double decomposedRoll = Vec3d.Dot(combined, fwd);
            Vec3d decomposedAim = combined - fwd * decomposedRoll;

            TestContext.WriteLine("true      omega_aim=" + state.AngularVelocityAim + " omega_roll=" + state.AngularVelocityRoll);
            TestContext.WriteLine("decomposed omega_aim=" + decomposedAim + " omega_roll=" + decomposedRoll);

            // This decomposition is mathematically exact ONLY if omega_aim already has zero
            // forward-axis component - which step 2 of ADR-0010 section 2 does not guarantee
            // between ticks (fwd itself rotates each tick). Demonstrate the two numbers the
            // wire actually carries are not perfectly reproduced by re-deriving from a sum: if
            // they happened to match to full double precision here it would mean this specific
            // scenario failed to exercise the gap, so the test also asserts the scenario is
            // non-degenerate (fwd is not perpendicular to omega_aim by construction of TurningInput).
            double fwdDotOmegaAim = Vec3d.Dot(fwd, state.AngularVelocityAim);
            TestContext.WriteLine("dot(fwd, true omega_aim) = " + fwdDotOmegaAim + " (must be non-negligible for this test to mean anything)");
            Assert.That(Math.Abs(fwdDotOmegaAim), Is.GreaterThan(1e-6),
                "test setup: omega_aim must have a non-zero forward-axis component, or decomposing from the sum " +
                "would accidentally be exact and this test would not be proving anything");
        }

        // Angular velocity quantises at 1/1000 deg/s (ADR-0009 section 2); the worst-case
        // 3-component round-trip error is sqrt(3) * 0.0005 =~ 0.00087 deg/s, so 0.001 is the
        // tight-but-correct bound - not a loosened threshold to make the test pass.
        static bool NearlyEqual(Vec3d a, Vec3d b) => (a - b).Length() < 0.001;

        // ------------------------------------------------------------------ COMMAND_RESULT{REJECTED}

        [Test]
        public void DropRejected_RemovesOnlyTheRejectedInput_NotReplayedOnNextReconcile()
        {
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);

            for (uint seq = 1; seq <= 3; seq++) controller.ApplyInput(ForwardThrust(seq));
            Assert.That(controller.History.Count, Is.EqualTo(3));

            controller.DropRejected(2);
            Assert.That(controller.History.Count, Is.EqualTo(2));
            foreach (InputRecord record in controller.History) Assert.That(record.InputSeq, Is.Not.EqualTo(2u));
        }

        // ------------------------------------------------------------------ reconnect / resume reset

        [Test]
        public void Reset_ClearsHistory_AndTrustsNewBaselineUnconditionally()
        {
            // architect's confirmed gap (재개 §6.2): "세션·입력 상태는 새로 시작,
            // last_applied_input_seq = None" - the retained history from the old session must
            // not leak into the new one.
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);
            for (uint seq = 1; seq <= 5; seq++) controller.ApplyInput(ForwardThrust(seq));
            Assert.That(controller.History.Count, Is.EqualTo(5));

            var resumedBaseline = new ShipSimState(new Vec3d(100, 0, 0), Vec3d.Zero, Quatd.Identity, Vec3d.Zero, 0);
            controller.Reset(resumedBaseline);

            Assert.That(controller.History.Count, Is.EqualTo(0));
            Assert.That(controller.CurrentState.Position, Is.EqualTo(resumedBaseline.Position));
            Assert.That(controller.LastReconcileHadError, Is.False);

            // The new session's input_seq starts at 1 again and must be accepted normally by
            // the local predictor (nothing here rejects a "re-used" seq=1 - only the server's
            // STALE_INPUT rule does, and it is keyed per-session there too).
            controller.ApplyInput(ForwardThrust(1));
            Assert.That(controller.History.Count, Is.EqualTo(1));
            Assert.That(controller.History[0].InputSeq, Is.EqualTo(1u));
        }
    }
}
