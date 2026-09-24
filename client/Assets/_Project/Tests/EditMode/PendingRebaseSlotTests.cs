// qa r6 F-2: ObserverSession's H-9 fix was unverified - qa deleted its ApplyPendingRebase() call
// and the suite stayed green at 237/235, because nothing executes ObserverSession. These tests
// cover the extracted state machine ObserverSession now actually runs (PendingRebaseSlot.cs), so
// the same deletion is detectable at the level where the behaviour lives.

using System;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class PendingRebaseSlotTests
    {
        static ShipSimState StateAt(double x) => new ShipSimState(
            new Vec3d(x, 0, 0), new Vec3d(0, 0, 0), Quatd.Identity, new Vec3d(0, 0, 0), 0.0);

        [Test]
        public void OutOfOrderBatch_OnlyHighestTickIsTaken_AndItsPayloadTravelsWithIt()
        {
            // The exact drain order F-2 is about: Pump() hands over 100, 102, 101 in one frame.
            // Asserting the payload (not just the tick) is what catches a slot that picks the
            // right tick but keeps an older snapshot's state - the rewind that used to be etched
            // into the QA CSV.
            var slot = new PendingRebaseSlot();
            Guid ship = Guid.NewGuid();

            slot.TryQueue(100, StateAt(100), 10, ship, "active");
            slot.TryQueue(102, StateAt(102), 12, ship, "active");
            slot.TryQueue(101, StateAt(101), 11, ship, "active");

            Assert.That(slot.TryTake(out long tick, out ShipSimState confirmed, out uint? ack, out Guid takenShip, out string presence), Is.True);
            Assert.That(tick, Is.EqualTo(102L));
            Assert.That(confirmed.Position.X, Is.EqualTo(102.0), "the taken state must be the one that arrived with tick 102");
            Assert.That(ack, Is.EqualTo((uint?)12));
            Assert.That(takenShip, Is.EqualTo(ship));
            Assert.That(presence, Is.EqualTo("active"));
        }

        [Test]
        public void TryQueue_ReportsWhetherItReplaced()
        {
            var slot = new PendingRebaseSlot();

            Assert.That(slot.TryQueue(100, StateAt(100), 1, Guid.Empty, "a"), Is.True, "nothing queued yet");
            Assert.That(slot.TryQueue(102, StateAt(102), 2, Guid.Empty, "a"), Is.True, "strictly newer");
            Assert.That(slot.TryQueue(102, StateAt(102), 3, Guid.Empty, "a"), Is.False, "same tick");
            Assert.That(slot.TryQueue(101, StateAt(101), 4, Guid.Empty, "a"), Is.False, "older");
        }

        [Test]
        public void TryTake_EmptiesTheSlot_SoAFrameWithNoSnapshotsReconcilesNothing()
        {
            // This is the assertion that would have caught the deleted ApplyPendingRebase() call
            // qa r6 used to prove F-2 unverified: a slot that never empties re-applies last
            // frame's snapshot forever, and a slot that is never filled never applies anything.
            var slot = new PendingRebaseSlot();
            slot.TryQueue(100, StateAt(100), 1, Guid.Empty, "a");

            Assert.That(slot.HasPending, Is.True);
            Assert.That(slot.TryTake(out _, out _, out _, out _, out _), Is.True);
            Assert.That(slot.HasPending, Is.False);
            Assert.That(slot.TryTake(out _, out _, out _, out _, out _), Is.False,
                "a second frame with no new snapshot must not re-apply the previous one");
        }

        [Test]
        public void EmptySlot_TakesNothing()
        {
            // §7b(1) pairing: the test above only means something if an untouched slot is empty
            // rather than holding a default-constructed tick 0.
            Assert.That(new PendingRebaseSlot().HasPending, Is.False);
            Assert.That(new PendingRebaseSlot().TryTake(out _, out _, out _, out _, out _), Is.False);
        }

        [Test]
        public void Clear_DropsAQueuedSnapshot_SoANewSessionCannotInheritTheOldOne()
        {
            // OnSessionReady calls this. Without it a fresh controller gets reconciled against
            // the previous session's snapshot on its first frame.
            var slot = new PendingRebaseSlot();
            slot.TryQueue(500, StateAt(500), 5, Guid.NewGuid(), "active");

            slot.Clear();

            Assert.That(slot.HasPending, Is.False);
            Assert.That(slot.TryTake(out _, out _, out _, out _, out _), Is.False);
            Assert.That(slot.TryQueue(1, StateAt(1), 1, Guid.Empty, "a"), Is.True,
                "after Clear the slot must accept a lower tick again - otherwise a new session is stuck behind the old one's high-water mark");
        }
    }
}
