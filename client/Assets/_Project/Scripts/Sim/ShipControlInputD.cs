// Hand-written. Dequantized control input for the prediction core.
//
// Only one constructor exists, and it takes wire integers. There is deliberately NO
// constructor that accepts raw doubles: I-36 says the client predicts from "the quantised
// integer it itself sent", not from the raw mouse/keyboard reading, or prediction and the
// server integrate from different numbers and drift with no bug anywhere (ADR-0012 section 2,
// design section 6.1). Making the mistake impossible to construct is cheaper than a code
// review catching it forever.

namespace Starfall.Sim
{
    public readonly struct ShipControlInputD
    {
        public readonly uint InputSeq;

        /// <summary>Local-axis thrust intent, dequantized to roughly [-1, 1] (ADR-0009 section 2).
        /// +X ship-right, +Y ship-up, +Z ship-forward (ADR-0009 section 1).</summary>
        public readonly Vec3d Thrust;

        /// <summary>Manual roll rate intent, dequantized to roughly [-1, 1].</summary>
        public readonly double Roll;

        /// <summary>Target attitude quaternion, dequantized but NOT renormalised - step 1 of
        /// ADR-0010 section 2 does that, including the degenerate-input fallback.</summary>
        public readonly Quatd AimRaw;

        public readonly bool Brake;
        public readonly bool FlightAssist;

        /// <summary>The only constructor: wire integers in, ADR-0009 section 2 dequantization
        /// applied once, here.</summary>
        public ShipControlInputD(
            uint inputSeq,
            long thrustXMilli, long thrustYMilli, long thrustZMilli,
            long rollMilli,
            long aimXMicro, long aimYMicro, long aimZMicro, long aimWMicro,
            bool brake, bool flightAssist)
        {
            InputSeq = inputSeq;
            Thrust = new Vec3d(
                Quantization.DequantizeControlAxis(thrustXMilli),
                Quantization.DequantizeControlAxis(thrustYMilli),
                Quantization.DequantizeControlAxis(thrustZMilli));
            Roll = Quantization.DequantizeControlAxis(rollMilli);
            AimRaw = new Quatd(
                Quantization.DequantizeQuaternionComponent(aimXMicro),
                Quantization.DequantizeQuaternionComponent(aimYMicro),
                Quantization.DequantizeQuaternionComponent(aimZMicro),
                Quantization.DequantizeQuaternionComponent(aimWMicro));
            Brake = brake;
            FlightAssist = flightAssist;
        }
    }
}
