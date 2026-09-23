// Hand-written. Raw control intent -> quantized SET_SHIP_CONTROL command, and the reverse:
// the SAME quantized ints -> a ShipControlInputD for prediction.
//
// ToDequantizedInput is the I-36 enforcement point: the caller predicts from the payload this
// class just built (the integers actually going on the wire), never from the raw floats that
// fed it. There is no shortcut from "raw input" straight to "predicted", by construction.

using System;
using Starfall.Contracts.Generated;
using Starfall.Sim;

namespace Starfall.Flight
{
    public static class SetShipControlBuilder
    {
        /// <summary>Builds the command to send. thrust/roll are expected in roughly [-1, 1]
        /// (Quantization.QuantizeControlAxis clamps regardless); aimWorldTarget need not be
        /// normalised - the server (and the predicting client, symmetrically) renormalise or
        /// treat a near-zero norm as degenerate (ADR-0010 section 2 step 1).</summary>
        public static SetShipControlCommand Build(
            Guid commandId,
            uint inputSeq,
            Vec3d thrust,
            double roll,
            Quatd aimWorldTarget,
            bool brake,
            bool flightAssist,
            string clientSentAtIso8601OrNull)
        {
            return new SetShipControlCommand
            {
                CommandId = commandId,
                CommandType = SetShipControlCommand.CommandTypeConst,
                SchemaVersion = SetShipControlCommand.SchemaVersionConst,
                ClientSentAt = clientSentAtIso8601OrNull,
                Payload = new SetShipControlCommand.SetShipControlPayload
                {
                    InputSeq = inputSeq,
                    ThrustXMilli = (int)Quantization.QuantizeControlAxis(thrust.X),
                    ThrustYMilli = (int)Quantization.QuantizeControlAxis(thrust.Y),
                    ThrustZMilli = (int)Quantization.QuantizeControlAxis(thrust.Z),
                    RollMilli = (int)Quantization.QuantizeControlAxis(roll),
                    AimXMicro = (int)Quantization.QuantizeQuaternionComponent(aimWorldTarget.X),
                    AimYMicro = (int)Quantization.QuantizeQuaternionComponent(aimWorldTarget.Y),
                    AimZMicro = (int)Quantization.QuantizeQuaternionComponent(aimWorldTarget.Z),
                    AimWMicro = (int)Quantization.QuantizeQuaternionComponent(aimWorldTarget.W),
                    Brake = brake,
                    FlightAssist = flightAssist,
                },
            };
        }

        /// <summary>I-36: the prediction input, built from the SAME quantized integers just put
        /// on the wire - never from the raw floats that produced them. Call this with the
        /// payload <see cref="Build"/> returned, not with a fresh computation from input.</summary>
        public static ShipControlInputD ToDequantizedInput(SetShipControlCommand.SetShipControlPayload payload)
        {
            return new ShipControlInputD(
                payload.InputSeq,
                payload.ThrustXMilli, payload.ThrustYMilli, payload.ThrustZMilli,
                payload.RollMilli,
                payload.AimXMicro, payload.AimYMicro, payload.AimZMicro, payload.AimWMicro,
                payload.Brake, payload.FlightAssist);
        }
    }
}
