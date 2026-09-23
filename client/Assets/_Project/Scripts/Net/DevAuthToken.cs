// Development-only credential. Rules: docs/adr/0008-dev-auth-token.md.
//
// This token has no expiry, no revocation, no rotation and no rate limiting. What makes it
// safe enough today is not cryptography, it is the deployment boundary: the server binds to
// 127.0.0.1 and the secret lives only on a developer machine. ADR-0008 section 4 lists every
// gap on purpose. The moment the server listens anywhere else, this file stops being
// acceptable.

using System;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace Starfall.Net
{
    /// <summary>
    /// Builds the <c>Authorization: Bearer</c> value for <c>GET /ws</c>:
    /// <c>&lt;subject_uuid_v7&gt;.&lt;hex(HMAC_SHA256(secret, subject_uuid_v7))&gt;</c>.
    /// <para>
    /// The client does not choose its <c>actor_id</c>. It proves it holds the secret for one
    /// subject; the server decides what that subject may be and tells the client in
    /// SESSION_READY (ADR-0008 section 5).
    /// </para>
    /// </summary>
    public static class DevAuthToken
    {
        /// <summary>Environment variable holding the shared development secret. Never read
        /// from a file, never defaulted in code - a default secret reaches production.</summary>
        public const string SecretVariable = "STARFALL_DEV_AUTH_SECRET";

        /// <summary>Environment variable that overrides <see cref="DefaultSubject"/>, so QA can
        /// move this identity without a rebuild if it ever collides with a bot identity.</summary>
        public const string SubjectVariable = "STARFALL_DEV_ACTOR_SUBJECT";

        /// <summary>
        /// The Unity client's development identity, fixed in
        /// <c>_workspace/p0-02-networking-spike/02_client_ack.md</c> section 2 so QA can assert
        /// it is disjoint from the 30 bot identities. Satisfies the UuidV7 contract pattern
        /// (version nibble 7, variant 8).
        /// </summary>
        public const string DefaultSubject = "01a0b1c2-7e57-7c11-8e57-000000000001";

        /// <summary>
        /// The second identity for the SC-64/65 "two sessions in one Unity process" observer
        /// harness (sprint contract section 0.11 / client ack issue 6): one process holds
        /// session A (the normal, potentially-interactive identity above, unaffected) and
        /// session B (this one) at the same time, each with its own socket, actor_id and CSV.
        /// One hex digit past <see cref="DefaultSubject"/> so it stays trivially disjoint from
        /// both that identity and the 30 bot identities (p0-02 02_client_ack.md section 2),
        /// without needing an environment override.
        /// </summary>
        public const string SecondObserverSubject = "01a0b1c2-7e57-7c11-8e57-000000000002";

        /// <summary>Subject id to authenticate as: the override variable when set, otherwise
        /// <see cref="DefaultSubject"/>.</summary>
        public static string ResolveSubject()
        {
            string overridden = Environment.GetEnvironmentVariable(SubjectVariable);
            return string.IsNullOrEmpty(overridden) ? DefaultSubject : overridden.Trim();
        }

        /// <summary>The shared secret, or null when the variable is unset or empty.</summary>
        public static string ResolveSecret()
        {
            string secret = Environment.GetEnvironmentVariable(SecretVariable);
            return string.IsNullOrEmpty(secret) ? null : secret;
        }

        /// <summary>
        /// Builds the bearer value from the environment.
        /// </summary>
        /// <exception cref="InvalidOperationException">The secret is not set. The message names
        /// the variable rather than saying "auth failed": a missing variable and a wrong
        /// secret fail at different places and the developer needs to know which one this is.
        /// </exception>
        public static string FromEnvironment()
        {
            string secret = ResolveSecret();
            if (secret == null)
            {
                throw new InvalidOperationException(
                    SecretVariable + " is not set in this process's environment. The Unity " +
                    "Editor inherits it from the shell that launched it, so export it before " +
                    "starting the Editor (see .env.example).");
            }
            return Create(ResolveSubject(), secret);
        }

        /// <summary>Builds the bearer value for an explicit subject and secret.</summary>
        public static string Create(string subject, string secret)
        {
            if (string.IsNullOrEmpty(subject)) throw new ArgumentException("subject is required", nameof(subject));
            if (string.IsNullOrEmpty(secret)) throw new ArgumentException("secret is required", nameof(secret));

            using (var hmac = new HMACSHA256(Encoding.UTF8.GetBytes(secret)))
            {
                byte[] mac = hmac.ComputeHash(Encoding.UTF8.GetBytes(subject));
                return subject + "." + ToLowerHex(mac);
            }
        }

        /// <summary>Lowercase hex. Convert.ToHexString does not exist in netstandard2.1, and
        /// it would produce uppercase anyway.</summary>
        static string ToLowerHex(byte[] bytes)
        {
            var text = new StringBuilder(bytes.Length * 2);
            for (int i = 0; i < bytes.Length; i++)
                text.Append(bytes[i].ToString("x2", CultureInfo.InvariantCulture));
            return text.ToString();
        }
    }
}
