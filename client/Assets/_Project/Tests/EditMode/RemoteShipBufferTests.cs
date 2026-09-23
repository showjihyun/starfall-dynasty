// C5. RemoteShipBuffer: bracketed interpolation, then capped extrapolation, then freeze
// (ADR-0012 section 5, AC-15(c)).

using NUnit.Framework;
using Starfall.Remote;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RemoteShipBufferTests
    {
        const double TickDurationSeconds = 1.0 / 20.0; // tick_hz = 20

        static ShipSimState StateAt(double z, double vz) =>
            new ShipSimState(new Vec3d(0, 0, z), new Vec3d(0, 0, vz), Quatd.Identity, Vec3d.Zero, 0);

        [Test]
        public void GetDisplay_BetweenTwoSamples_Interpolates()
        {
            var buffer = new RemoteShipBuffer();
            buffer.AddSample(tick: 100, StateAt(z: 0, vz: 140), "ACTIVE");
            buffer.AddSample(tick: 102, StateAt(z: 14, vz: 140), "ACTIVE"); // 2 ticks * 0.05s * 140 m/s = 14 m

            RemoteShipBuffer.Display display = buffer.GetDisplay(renderTick: 101, TickDurationSeconds, extrapolateMaxTicks: 5);

            TestContext.WriteLine("mode=" + display.Mode + " z=" + display.State.Position.Z);
            Assert.That(display.Mode, Is.EqualTo(RemoteShipBuffer.DisplayMode.Interpolated));
            Assert.That(display.State.Position.Z, Is.EqualTo(7.0).Within(1e-9), "renderTick=101 is exactly halfway between tick 100 and 102");
        }

        [Test]
        public void GetDisplay_PastLatestSample_WithinCap_Extrapolates()
        {
            var buffer = new RemoteShipBuffer();
            buffer.AddSample(tick: 100, StateAt(z: 0, vz: 140), "ACTIVE");

            // 3 ticks past the last sample, cap is 5 ticks - must still extrapolate, not freeze.
            RemoteShipBuffer.Display display = buffer.GetDisplay(renderTick: 103, TickDurationSeconds, extrapolateMaxTicks: 5);

            TestContext.WriteLine("mode=" + display.Mode + " z=" + display.State.Position.Z);
            Assert.That(display.Mode, Is.EqualTo(RemoteShipBuffer.DisplayMode.Extrapolated));
            Assert.That(display.State.Position.Z, Is.EqualTo(3 * TickDurationSeconds * 140.0).Within(1e-9));
        }

        [Test]
        public void GetDisplay_PastExtrapolationCap_FreezesAndZeroesVelocity()
        {
            var buffer = new RemoteShipBuffer();
            buffer.AddSample(tick: 100, StateAt(z: 0, vz: 140), "ACTIVE");

            RemoteShipBuffer.Display atCap = buffer.GetDisplay(renderTick: 105, TickDurationSeconds, extrapolateMaxTicks: 5);
            RemoteShipBuffer.Display wayPast = buffer.GetDisplay(renderTick: 200, TickDurationSeconds, extrapolateMaxTicks: 5);

            TestContext.WriteLine("at cap (tick 105): mode=" + atCap.Mode + " z=" + atCap.State.Position.Z);
            TestContext.WriteLine("way past (tick 200): mode=" + wayPast.Mode + " z=" + wayPast.State.Position.Z);

            Assert.That(wayPast.Mode, Is.EqualTo(RemoteShipBuffer.DisplayMode.Frozen));
            Assert.That(wayPast.State.Velocity, Is.EqualTo(Vec3d.Zero), "a frozen ship must show zero velocity - it stopped, it did not vanish or keep flying");

            // The frozen position must be the position AT THE CAP, not still creeping forward:
            // extrapolating 100 ticks worth of velocity would put it far past where 5 ticks would.
            Assert.That(wayPast.State.Position.Z, Is.EqualTo(atCap.State.Position.Z).Within(1e-6),
                "a frozen ship must not keep moving past the extrapolation cap point");
        }

        [Test]
        public void AddSample_IgnoresOutOfOrderOrDuplicateTicks()
        {
            var buffer = new RemoteShipBuffer();
            buffer.AddSample(tick: 100, StateAt(0, 0), "ACTIVE");
            buffer.AddSample(tick: 100, StateAt(999, 0), "ACTIVE"); // duplicate tick, must be dropped
            buffer.AddSample(tick: 99, StateAt(999, 0), "ACTIVE");  // out of order, must be dropped
            buffer.AddSample(tick: 101, StateAt(7, 0), "ACTIVE");

            Assert.That(buffer.Samples.Count, Is.EqualTo(2));
            Assert.That(buffer.Samples[0].Tick, Is.EqualTo(100));
            Assert.That(buffer.Samples[1].Tick, Is.EqualTo(101));
        }

        [Test]
        public void GetDisplay_NoSamples_ReportsNoData()
        {
            var buffer = new RemoteShipBuffer();
            RemoteShipBuffer.Display display = buffer.GetDisplay(renderTick: 0, TickDurationSeconds, extrapolateMaxTicks: 5);
            Assert.That(display.Mode, Is.EqualTo(RemoteShipBuffer.DisplayMode.NoData));
        }

        [Test]
        public void GetDisplay_PresenceReflectsTheLaterBracketSample()
        {
            var buffer = new RemoteShipBuffer();
            buffer.AddSample(tick: 100, StateAt(0, 0), "ACTIVE");
            buffer.AddSample(tick: 102, StateAt(0, 0), "LINGERING"); // session ended between these two snapshots

            RemoteShipBuffer.Display display = buffer.GetDisplay(renderTick: 101, TickDurationSeconds, extrapolateMaxTicks: 5);
            Assert.That(display.Presence, Is.EqualTo("LINGERING"));
        }
    }
}
