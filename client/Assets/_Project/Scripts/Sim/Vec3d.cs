// Hand-written. Double-precision vector for the prediction core (ADR-0009 section 5, I-36).
//
// This type exists ONLY because UnityEngine.Vector3 is float32. Starfall.Sim has
// noEngineReferences: true, so UnityEngine types cannot even be referenced here - that is
// the cheapest guard against float32 silently leaking into prediction (client hand-off,
// 02_client_ack.md section 2).

using System;

namespace Starfall.Sim
{
    /// <summary>Double-precision 3-vector. No implicit conversion to/from any Unity type
    /// exists anywhere in this assembly - the conversion happens once, in Starfall.Flight,
    /// when a predicted state is handed to something that draws it.</summary>
    public readonly struct Vec3d : IEquatable<Vec3d>
    {
        public readonly double X;
        public readonly double Y;
        public readonly double Z;

        public Vec3d(double x, double y, double z)
        {
            X = x;
            Y = y;
            Z = z;
        }

        public static readonly Vec3d Zero = new Vec3d(0.0, 0.0, 0.0);

        public static Vec3d operator +(Vec3d a, Vec3d b) => new Vec3d(a.X + b.X, a.Y + b.Y, a.Z + b.Z);
        public static Vec3d operator -(Vec3d a, Vec3d b) => new Vec3d(a.X - b.X, a.Y - b.Y, a.Z - b.Z);
        public static Vec3d operator -(Vec3d a) => new Vec3d(-a.X, -a.Y, -a.Z);
        public static Vec3d operator *(Vec3d a, double s) => new Vec3d(a.X * s, a.Y * s, a.Z * s);
        public static Vec3d operator *(double s, Vec3d a) => new Vec3d(a.X * s, a.Y * s, a.Z * s);
        public static Vec3d operator /(Vec3d a, double s) => new Vec3d(a.X / s, a.Y / s, a.Z / s);

        public static double Dot(Vec3d a, Vec3d b) => a.X * b.X + a.Y * b.Y + a.Z * b.Z;

        public static Vec3d Cross(Vec3d a, Vec3d b) => new Vec3d(
            a.Y * b.Z - a.Z * b.Y,
            a.Z * b.X - a.X * b.Z,
            a.X * b.Y - a.Y * b.X);

        /// <summary>sqrt(x^2+y^2+z^2), spelled out per ADR-0010 section 3 - never
        /// System.Math.Hypot equivalents, IEEE-754 only defines +-*&#47; and sqrt.</summary>
        public double Length() => Math.Sqrt(X * X + Y * Y + Z * Z);

        public Vec3d Normalized() => this / Length();

        public bool Equals(Vec3d other) => X.Equals(other.X) && Y.Equals(other.Y) && Z.Equals(other.Z);
        public override bool Equals(object obj) => obj is Vec3d other && Equals(other);
        public override int GetHashCode() => HashCode.Combine(X, Y, Z);
        public override string ToString() => "(" + X + ", " + Y + ", " + Z + ")";
    }
}
