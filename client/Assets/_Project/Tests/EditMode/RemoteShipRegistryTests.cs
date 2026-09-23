// C5. RemoteShipRegistry: absence from ships[] means despawned NOW (design section 6.4),
// distinct from the snapshot stream going quiet (RemoteShipBufferTests' extrapolate/freeze).

using System;
using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Remote;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RemoteShipRegistryTests
    {
        static readonly Guid ShipA = new Guid("00000000-0000-0000-0000-00000000000a");
        static readonly Guid ShipB = new Guid("00000000-0000-0000-0000-00000000000b");

        static ShipSimState Zero => ShipSimState.Zero;

        [Test]
        public void OnSnapshot_NewShipId_CreatesABuffer()
        {
            var registry = new RemoteShipRegistry();
            registry.OnSnapshot(100, new[] { (ShipA, Zero, "ACTIVE") });

            Assert.That(registry.Buffers.Count, Is.EqualTo(1));
            Assert.That(registry.TryGetBuffer(ShipA, out RemoteShipBuffer buffer), Is.True);
            Assert.That(buffer.Samples.Count, Is.EqualTo(1));
        }

        [Test]
        public void OnSnapshot_ShipAbsentFromLatestSnapshot_IsRemovedImmediately()
        {
            // AC-15(c) vs (d): this is (d) - absence in a snapshot that DID arrive means
            // despawned. It must not wait for extrapolate_max_ms; the ship is gone this frame.
            var registry = new RemoteShipRegistry();
            registry.OnSnapshot(100, new[] { (ShipA, Zero, "ACTIVE"), (ShipB, Zero, "ACTIVE") });
            Assert.That(registry.Buffers.Count, Is.EqualTo(2));

            registry.OnSnapshot(102, new[] { (ShipA, Zero, "ACTIVE") }); // ShipB gone: despawned

            Assert.That(registry.Buffers.Count, Is.EqualTo(1));
            Assert.That(registry.TryGetBuffer(ShipB, out _), Is.False, "a ship absent from ships[] must be removed the same snapshot, not lingered by the client");
            Assert.That(registry.TryGetBuffer(ShipA, out _), Is.True);
        }

        [Test]
        public void OnSnapshot_LingeringShip_IsKept_NotRemoved()
        {
            // LINGERING is still present in ships[] (design section 6.4: "presence == LINGERING는
            // 제거가 아니다") - only actual absence removes it.
            var registry = new RemoteShipRegistry();
            registry.OnSnapshot(100, new[] { (ShipA, Zero, "ACTIVE") });
            registry.OnSnapshot(102, new[] { (ShipA, Zero, "LINGERING") });

            Assert.That(registry.Buffers.Count, Is.EqualTo(1));
            Assert.That(registry.TryGetBuffer(ShipA, out RemoteShipBuffer buffer), Is.True);
            Assert.That(buffer.Samples.Count, Is.EqualTo(2));
        }

        [Test]
        public void OnSnapshot_NoNewSnapshot_DoesNotRemoveAnything()
        {
            // The "stream goes quiet" case: if OnSnapshot is simply never called again (no
            // WORLD_SNAPSHOT arriving at all), nothing here removes the buffer - it just goes
            // stale, which is what drives RemoteShipBuffer's own extrapolate-then-freeze.
            var registry = new RemoteShipRegistry();
            registry.OnSnapshot(100, new[] { (ShipA, Zero, "ACTIVE") });

            Assert.That(registry.Buffers.Count, Is.EqualTo(1));
            // (no further OnSnapshot call)
            Assert.That(registry.Buffers.Count, Is.EqualTo(1), "absence of a NEW snapshot must not despawn anything by itself");
        }
    }
}
