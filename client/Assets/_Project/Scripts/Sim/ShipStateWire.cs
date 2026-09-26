// Hand-written. Wire ShipState (Starfall.Contracts.Generated.WorldSnapshotMessage's nested
// array element) <-> ShipSimState. One place for this so C4 (self-ship reconciliation) and C5
// (other-ship interpolation) dequantize identically instead of each rolling their own.

using Starfall.Contracts.Generated;

namespace Starfall.Sim
{
    public static class ShipStateWire
    {
        /// <summary>Dequantizes one WORLD_SNAPSHOT ship entry into a predictable/interpolatable
        /// state. The orientation IS renormalised here (ADR-0009 section 2: "양자화된
        /// 쿼터니언은 정확히 단위 길이가 아니다 - 소비자는 쓰기 전에 정규화한다"). Every
        /// consumer of this factory (C4 reconciliation, C5 interpolation) needs a unit
        /// quaternion - reconciliation's angle-error computation (Quatd.AngleDegrees) assumes
        /// unit length, and slerp is only defined for unit quaternions.</summary>
        public static ShipSimState ToSimState(WorldSnapshotMessage.WorldSnapshotPayload.ShipState wire)
        {
            var orientation = new Quatd(
                Quantization.DequantizeQuaternionComponent(wire.OrientationXMicro),
                Quantization.DequantizeQuaternionComponent(wire.OrientationYMicro),
                Quantization.DequantizeQuaternionComponent(wire.OrientationZMicro),
                Quantization.DequantizeQuaternionComponent(wire.OrientationWMicro)).Normalized();

            return new ShipSimState(
                position: new Vec3d(
                    Quantization.DequantizePosition(wire.PositionXMm),
                    Quantization.DequantizePosition(wire.PositionYMm),
                    Quantization.DequantizePosition(wire.PositionZMm)),
                velocity: new Vec3d(
                    Quantization.DequantizeVelocity(wire.VelocityXMmS),
                    Quantization.DequantizeVelocity(wire.VelocityYMmS),
                    Quantization.DequantizeVelocity(wire.VelocityZMmS)),
                orientation: orientation,
                angularVelocityAim: new Vec3d(
                    Quantization.DequantizeAngularVelocity(wire.AngularVelocityXMdegS),
                    Quantization.DequantizeAngularVelocity(wire.AngularVelocityYMdegS),
                    Quantization.DequantizeAngularVelocity(wire.AngularVelocityZMdegS)),
                angularVelocityRoll: Quantization.DequantizeAngularVelocity(wire.AngularVelocityRollMdegS));
        }
    }
}
