// H-9 (수신측 스냅샷 중복 처리, 리더 메시지 2026-09-23). Pure tests for the "which snapshot in a
// batch wins the rebase" decision (Starfall.Greybox.SnapshotRebaseBatch) - see that file's header
// for why this had to be pulled out of GreyboxSession.OnWorldSnapshotCore rather than reasoned
// about only through a live client/transport pairing (the same discipline T-4/T-6 applied to
// TickCatchUp, per this slice's own §7a/§7b history of tests that reimplemented or coupled to a
// copy of the real decision instead of calling it).

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class SnapshotRebaseBatchTests
    {
        [Test]
        public void ShouldReplacePending_NoPendingYet_AlwaysTrue()
        {
            Assert.That(SnapshotRebaseBatch.ShouldReplacePending(candidateTick: 100, pendingTick: null), Is.True);
        }

        [Test]
        public void ShouldReplacePending_CandidateNewerThanPending_True()
        {
            // The H-9 headline case: two snapshots land in the same Pump() drain, tick 101
            // arrives after tick 100 is already queued - 101 must win.
            Assert.That(SnapshotRebaseBatch.ShouldReplacePending(candidateTick: 101, pendingTick: 100), Is.True);
        }

        [Test]
        public void ShouldReplacePending_CandidateOlderThanPending_False()
        {
            // Out-of-order arrival within one drain (should not normally happen over an ordered
            // WS stream, but the decision must not regress CurrentState if it ever does): a
            // stale tick 99 arriving after 100 is already queued must NOT displace it.
            Assert.That(SnapshotRebaseBatch.ShouldReplacePending(candidateTick: 99, pendingTick: 100), Is.False);
        }

        [Test]
        public void ShouldReplacePending_CandidateEqualsPending_False()
        {
            // A same-tick resend/duplicate frame: nothing newer to gain, keep the first-seen one.
            Assert.That(SnapshotRebaseBatch.ShouldReplacePending(candidateTick: 100, pendingTick: 100), Is.False);
        }

        // §7b self-check on this test file's own new item: "the property this asserts holds
        // trivially if ShouldReplacePending always returns true (or always false)" - the four
        // cases above are chosen so an always-true implementation fails
        // ShouldReplacePending_CandidateOlderThanPending_False and
        // ShouldReplacePending_CandidateEqualsPending_False, and an always-false implementation
        // fails ShouldReplacePending_NoPendingYet_AlwaysTrue and
        // ShouldReplacePending_CandidateNewerThanPending_True. Neither degenerate
        // implementation passes all four - RED-checked below by simulating both degenerate forms
        // inline instead of only trusting the real implementation.
        [Test]
        public void ShouldReplacePending_DegenerateAlwaysTrueOrAlwaysFalse_WouldFailAtLeastOneCase()
        {
            // qa r5 F-5: the version of this test that shipped in R12 asserted
            // `AlwaysTrue(99, 100) && AlwaysFalse(101, 100)` is false - which the compiler folds
            // to `true && false`, so it never called ShouldReplacePending at all and would have
            // passed against a deleted implementation. That is the same disease as T-5's
            // invariant 3 (§7a). Rewritten to run the REAL function and both degenerate forms
            // over one shared case table, and to assert each degenerate DISAGREES with the real
            // one somewhere. If ShouldReplacePending ever degenerates into either form, the
            // disagreement vanishes and this test goes red.
            (long candidate, long? pending, bool expected)[] cases =
            {
                (100, null, true),   // nothing queued yet - always take it
                (102, 100,  true),   // strictly newer - replace
                (100, 100,  false),  // same tick - keep what is queued
                (101, 102,  false),  // older than what is queued - drop
            };

            bool AlwaysTrue(long candidate, long? pending) => true;
            bool AlwaysFalse(long candidate, long? pending) => false;

            int alwaysTrueDisagreements = 0;
            int alwaysFalseDisagreements = 0;

            foreach ((long candidate, long? pending, bool expected) in cases)
            {
                bool real = SnapshotRebaseBatch.ShouldReplacePending(candidate, pending);

                Assert.That(real, Is.EqualTo(expected),
                    $"ShouldReplacePending({candidate}, {(pending.HasValue ? pending.Value.ToString() : "null")})");

                if (AlwaysTrue(candidate, pending) != expected) alwaysTrueDisagreements++;
                if (AlwaysFalse(candidate, pending) != expected) alwaysFalseDisagreements++;
            }

            // The point of the test: neither constant function reproduces this table, so passing
            // the loop above is evidence about the real implementation and not about the table.
            Assert.That(alwaysTrueDisagreements, Is.GreaterThan(0),
                "an always-true implementation must fail at least one case, or this table cannot detect it");
            Assert.That(alwaysFalseDisagreements, Is.GreaterThan(0),
                "an always-false implementation must fail at least one case, or this table cannot detect it");
        }

        // ------------------------------------------------------------------ batch-selection scenario (§7a: the condition actually occurred)

        [Test]
        public void Batch_OfThreeArrivingOutOfPumpOrder_OnlyHighestTickEverWins()
        {
            // Simulates OnWorldSnapshotCore's queuing loop directly (not through GreyboxSession -
            // no MonoBehaviour/transport needed for this pure decision) for a 3-message drain
            // arriving tick 100, 102, 101 in that order (RealtimeClient.Pump() delivers whatever
            // order the transport's queue held - see RealtimeClient.cs OnMessage). §7a: the
            // condition this guards against (more than one snapshot in a batch) is exercised
            // explicitly here, not just asserted possible.
            long?[] arrivalOrder = { 100, 102, 101 };
            long? pending = null;

            foreach (long tick in arrivalOrder)
            {
                if (SnapshotRebaseBatch.ShouldReplacePending(tick, pending))
                    pending = tick;
            }

            Assert.That(pending, Is.EqualTo(102L),
                "the highest tick in the batch (102) must win regardless of arrival order within it");
        }
    }
}
