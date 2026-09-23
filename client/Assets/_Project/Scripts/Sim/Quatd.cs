// Hand-written. Double-precision quaternion for the prediction core (ADR-0009 section 5,
// ADR-0010 section 2, I-36). Component order (x, y, z, w), Hamilton product - matching the
// wire (orientation_x/y/z/w_micro) and ADR-0010 section 2 exactly.

using System;

namespace Starfall.Sim
{
    /// <summary>Double-precision quaternion. Not normalised by construction - callers
    /// normalise explicitly at the points ADR-0010 section 2 specifies (steps 1, 4, 5), same
    /// as the server does, so the two sides re-normalise at the same places.</summary>
    public readonly struct Quatd : IEquatable<Quatd>
    {
        public readonly double X;
        public readonly double Y;
        public readonly double Z;
        public readonly double W;

        public Quatd(double x, double y, double z, double w)
        {
            X = x;
            Y = y;
            Z = z;
            W = w;
        }

        /// <summary>The multiplicative identity: no rotation.</summary>
        public static readonly Quatd Identity = new Quatd(0.0, 0.0, 0.0, 1.0);

        /// <summary>q_pure(w) = (w.x, w.y, w.z, 0) - ADR-0010 section 2 notation, used to turn
        /// an angular velocity vector into a quaternion for the (dt/2)*(w_pure * q) integration
        /// step.</summary>
        public static Quatd Pure(Vec3d w) => new Quatd(w.X, w.Y, w.Z, 0.0);

        /// <summary>Hamilton product, a (Multiply) b == a *dot b in ADR-0010 section 2's
        /// notation. NOT commutative - the caller's operand order is load-bearing.</summary>
        public static Quatd operator *(Quatd a, Quatd b) => new Quatd(
            a.W * b.X + a.X * b.W + a.Y * b.Z - a.Z * b.Y,
            a.W * b.Y - a.X * b.Z + a.Y * b.W + a.Z * b.X,
            a.W * b.Z + a.X * b.Y - a.Y * b.X + a.Z * b.W,
            a.W * b.W - a.X * b.X - a.Y * b.Y - a.Z * b.Z);

        public static Quatd operator +(Quatd a, Quatd b) => new Quatd(a.X + b.X, a.Y + b.Y, a.Z + b.Z, a.W + b.W);
        public static Quatd operator -(Quatd a) => new Quatd(-a.X, -a.Y, -a.Z, -a.W);
        public static Quatd operator *(Quatd a, double s) => new Quatd(a.X * s, a.Y * s, a.Z * s, a.W * s);
        public static Quatd operator /(Quatd a, double s) => new Quatd(a.X / s, a.Y / s, a.Z / s, a.W / s);

        public Quatd Conjugate() => new Quatd(-X, -Y, -Z, W);

        public double Length() => Math.Sqrt(X * X + Y * Y + Z * Z + W * W);

        public Quatd Normalized() => this / Length();

        /// <summary>vec(q): the (x, y, z) part only.</summary>
        public Vec3d Vector() => new Vec3d(X, Y, Z);

        /// <summary>rot(q, v) = vec(q (x) (v,0) (x) conj(q)) - ADR-0010 section 2's exact
        /// notation, spelled out as two literal Hamilton products rather than the algebraically
        /// equivalent 9-multiply shortcut. The shortcut is NOT guaranteed to produce the same
        /// f64 bits (different operation order, different rounding), and determinism (ADR-0010
        /// section 3) requires both languages to do the identical sequence of operations, not
        /// merely the same mathematics.</summary>
        public Vec3d Rotate(Vec3d v)
        {
            Quatd p = new Quatd(v.X, v.Y, v.Z, 0.0);
            Quatd r = this * p * Conjugate();
            return new Vec3d(r.X, r.Y, r.Z);
        }

        /// <summary>Shortest-path angle between two orientations, in degrees. NOT part of the
        /// deterministic integrator - a diagnostic/comparison value only (reconciliation error
        /// reporting, HUD, boundary tests), so Math.Acos here does not touch ADR-0010 section
        /// 3's transcendental-function ban, which applies to state advancement only.</summary>
        public static double AngleDegrees(Quatd a, Quatd b)
        {
            double dot = a.X * b.X + a.Y * b.Y + a.Z * b.Z + a.W * b.W;
            if (dot < 0.0) dot = -dot; // quaternions double-cover rotations: q and -q are the same attitude
            if (dot > 1.0) dot = 1.0; // guard acos domain against quantisation/renormalisation noise
            return 2.0 * Math.Acos(dot) * (180.0 / Math.PI);
        }

        public bool Equals(Quatd other) =>
            X.Equals(other.X) && Y.Equals(other.Y) && Z.Equals(other.Z) && W.Equals(other.W);
        public override bool Equals(object obj) => obj is Quatd other && Equals(other);
        public override int GetHashCode() => HashCode.Combine(X, Y, Z, W);
        public override string ToString() => "(" + X + ", " + Y + ", " + Z + ", " + W + ")";
    }
}
