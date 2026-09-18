// Hand-written (not generated). UUIDv7 (RFC 9562) generation for client-created ids.
//
// Why not new Guid(byte[]): that constructor reads the first three groups little-endian.
// Handing it canonical big-endian bytes produces an id whose VERSION nibble is wrong while
// the VARIANT nibble still looks right, so a sloppy pattern check passes. Measured:
//     canonical                  019957fe-5283-7abc-8def-1a1b1c1d1e1f   (version 7)
//     new Guid(bigEndianBytes)   fe579901-8352-bc7a-8def-1a1b1c1d1e1f   (version b)
// Building the canonical string and parsing it removes the endianness question entirely.
//
// .NET 9+ has Guid.CreateVersion7(), but Unity targets netstandard2.1 and does not have it.

using System;
using System.Security.Cryptography;

namespace Starfall.Contracts
{
    /// <summary>
    /// Creates UUIDv7 identifiers in the canonical lowercase hyphenated form required by
    /// <c>contracts/common/primitives.schema.json#/$defs/UuidV7</c>.
    /// <para>
    /// The client generates <c>command_id</c> only. A retry of the same intent reuses the same
    /// id so the server can apply it at most once; the embedded timestamp is an index-locality
    /// detail and is never a game fact (ADR-0002 section 5).
    /// </para>
    /// </summary>
    public static class UuidV7
    {
        static readonly char[] HexDigits = "0123456789abcdef".ToCharArray();
        static readonly RandomNumberGenerator Rng = RandomNumberGenerator.Create();

        static readonly DateTime UnixEpoch = new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc);

        /// <summary>Creates a UUIDv7 stamped with the current UTC time.</summary>
        public static Guid NewGuid() => FromUnixTimeMilliseconds(CurrentUnixTimeMilliseconds());

        /// <summary>
        /// Creates a UUIDv7 stamped with an explicit Unix millisecond timestamp. Exposed so a
        /// test can generate many ids inside one millisecond without racing the clock.
        /// </summary>
        public static Guid FromUnixTimeMilliseconds(long unixMilliseconds)
        {
            if (unixMilliseconds < 0 || unixMilliseconds > 0xFFFFFFFFFFFFL)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(unixMilliseconds),
                    "UUIDv7 stores the timestamp in 48 bits (0 .. 281474976710655).");
            }

            var random = new byte[10];
            Rng.GetBytes(random);

            var bytes = new byte[16];
            bytes[0] = (byte)(unixMilliseconds >> 40);
            bytes[1] = (byte)(unixMilliseconds >> 32);
            bytes[2] = (byte)(unixMilliseconds >> 24);
            bytes[3] = (byte)(unixMilliseconds >> 16);
            bytes[4] = (byte)(unixMilliseconds >> 8);
            bytes[5] = (byte)unixMilliseconds;

            // Byte 6 high nibble = version 7; low nibble plus byte 7 are random (rand_a).
            bytes[6] = (byte)(0x70 | (random[0] & 0x0F));
            bytes[7] = random[1];

            // Byte 8 high bits = variant 10xx; the rest is random (rand_b).
            bytes[8] = (byte)(0x80 | (random[2] & 0x3F));
            for (int i = 9; i < 16; i++) bytes[i] = random[i - 6];

            return new Guid(Canonical(bytes));
        }

        /// <summary>Milliseconds since the Unix epoch. netstandard2.1 has DateTimeOffset, but
        /// going through UTC ticks keeps the arithmetic obvious.</summary>
        public static long CurrentUnixTimeMilliseconds() =>
            (long)(DateTime.UtcNow - UnixEpoch).TotalMilliseconds;

        /// <summary>
        /// Formats 16 bytes as 8-4-4-4-12 lowercase hex. Written by hand because
        /// <c>Convert.ToHexString</c> does not exist in netstandard2.1.
        /// </summary>
        static string Canonical(byte[] bytes)
        {
            var chars = new char[36];
            int outIndex = 0;
            int byteIndex = 0;

            // Group lengths in BYTES: 4-2-2-2-6 -> 8-4-4-4-12 hex characters.
            int[] groupLengths = { 4, 2, 2, 2, 6 };
            for (int group = 0; group < groupLengths.Length; group++)
            {
                for (int k = 0; k < groupLengths[group]; k++)
                {
                    byte value = bytes[byteIndex++];
                    chars[outIndex++] = HexDigits[value >> 4];
                    chars[outIndex++] = HexDigits[value & 0x0F];
                }
                if (group < groupLengths.Length - 1) chars[outIndex++] = '-';
            }

            return new string(chars);
        }
    }
}
