// Hand-written. Full predictable ship state (ADR-0010 section 2, ADR-0012 section 1).
// p, v, q, omega_aim, omega_roll - all five, because ADR-0012 section 3 step 2 rebases ALL
// five on reconciliation, not just position and orientation. Leaving angular velocity at its
// predicted value while snapping position/orientation is the bug ADR-0010 section 1.1 exists
// to prevent.

namespace Starfall.Sim
{
    public readonly struct ShipSimState
    {
        public readonly Vec3d Position;
        public readonly Vec3d Velocity;
        public readonly Quatd Orientation;

        /// <summary>omega_aim - world-frame angular velocity of the attitude controller.
        /// Does NOT include roll (ADR-0010 section 1.1).</summary>
        public readonly Vec3d AngularVelocityAim;

        /// <summary>omega_roll - scalar rate about the ship-local +Z axis.</summary>
        public readonly double AngularVelocityRoll;

        public ShipSimState(Vec3d position, Vec3d velocity, Quatd orientation, Vec3d angularVelocityAim, double angularVelocityRoll)
        {
            Position = position;
            Velocity = velocity;
            Orientation = orientation;
            AngularVelocityAim = angularVelocityAim;
            AngularVelocityRoll = angularVelocityRoll;
        }

        public static readonly ShipSimState Zero =
            new ShipSimState(Vec3d.Zero, Vec3d.Zero, Quatd.Identity, Vec3d.Zero, 0.0);
    }
}
