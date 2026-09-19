// Hand-written (not generated). Serializer profiles for contract JSON.
// Rules: docs/adr/0002-contract-format-and-codegen.md section 4.
//
// This file lives in Starfall.Contracts, which has noEngineReferences: true.
// UnityEngine.Debug / Application / Random are deliberately unavailable here.

using System;
using System.IO;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Starfall.Contracts
{
    /// <summary>
    /// The only place contract JSON is read or written. Two profiles exist on purpose
    /// (ADR-0002 section 4): the server is strict about commands because it does not trust
    /// clients, while the client is tolerant of server messages because the server may be
    /// deployed first.
    /// </summary>
    public static class ContractJson
    {
        /// <summary>
        /// Tests and development builds. Surfaces contract violations instead of hiding them:
        /// an unknown member or a missing required field throws.
        /// <para>
        /// <see cref="DateParseHandling.None"/> is not optional. With the default reader,
        /// Newtonsoft turns an ISO-8601 string into a DateTime and writes it back into a
        /// string property in a different format, so "2026-09-17T14:05:09.123Z" silently
        /// becomes "09/17/2026 14:05:09" and the milliseconds are gone.
        /// </para>
        /// </summary>
        public static JsonSerializerSettings Strict
        {
            get
            {
                // A fresh instance each time: JsonSerializerSettings is mutable, and a shared
                // static would let one caller's tweak leak into every other call site.
                return new JsonSerializerSettings
                {
                    DateParseHandling = DateParseHandling.None,
                    MissingMemberHandling = MissingMemberHandling.Error,
                    NullValueHandling = NullValueHandling.Include,
                    FloatParseHandling = FloatParseHandling.Double,
                    Culture = System.Globalization.CultureInfo.InvariantCulture,
                };
            }
        }

        /// <summary>
        /// The one thing Newtonsoft gives us to tell "the sender knows a member we do not"
        /// apart from every other deserialization error.
        /// <para>
        /// Measured, not assumed (p0-02 client review): <c>ErrorContext.Member</c> is non-null
        /// for BOTH an unknown member and a missing required property, and
        /// <c>ErrorContext.Path</c> equals the member name in the "null on a non-nullable
        /// field" case, so neither can discriminate. The message prefix can. Newtonsoft does
        /// not localize its messages and Unity pins com.unity.nuget.newtonsoft-json 3.2.2
        /// (Newtonsoft 13.0.2), so this is stable for this project - and
        /// <c>Runtime_PrefixDependency_IsPinned</c> fails if a future upgrade changes it,
        /// which is the point: the upgrade breaks a test, not the live receive path.
        /// </para>
        /// </summary>
        public const string IgnoredMemberMessagePrefix = "Could not find member";

        /// <summary>
        /// Raised once per member the <see cref="Runtime"/> profile ignored, with the JSON
        /// path of that member. Starfall.Net subscribes and logs a warning; this assembly
        /// cannot log by itself (noEngineReferences: true).
        /// </summary>
        public static event Action<string> IgnoredMember;

        /// <summary>
        /// Live receive path: same date handling as <see cref="Strict"/>, but a member this
        /// build does not know is ignored with a warning instead of dropping the whole
        /// message - the server is deployed first and may know types we do not (ADR-0002
        /// section 4).
        /// <para>
        /// <b>An unknown member is the only thing this profile tolerates.</b> A missing
        /// required field and a null in a non-nullable field still throw (ADR-0005 section 5).
        /// Without that line a "tolerant" profile becomes an "unvalidated" profile: handling
        /// every error would accept <c>missing-session-id.json</c> and hand the caller a
        /// session whose id is <c>Guid.Empty</c>.
        /// </para>
        /// </summary>
        public static JsonSerializerSettings Runtime => CreateRuntime(RaiseIgnoredMember);

        /// <summary>
        /// <see cref="Runtime"/> with an explicit sink instead of the static event. The
        /// receive path builds one of these once and reuses it; tests use it to capture the
        /// warnings without touching global state.
        /// </summary>
        /// <param name="onIgnoredMember">Receives the JSON path of each ignored member. May
        /// be null, in which case unknown members are tolerated silently.</param>
        public static JsonSerializerSettings CreateRuntime(Action<string> onIgnoredMember)
        {
            var settings = new JsonSerializerSettings
            {
                DateParseHandling = DateParseHandling.None,
                MissingMemberHandling = MissingMemberHandling.Error,
                NullValueHandling = NullValueHandling.Include,
                FloatParseHandling = FloatParseHandling.Double,
                Culture = System.Globalization.CultureInfo.InvariantCulture,
            };

            settings.Error += (sender, args) =>
            {
                Newtonsoft.Json.Serialization.ErrorContext context = args.ErrorContext;
                if (context.Error == null) return;
                if (!IsIgnorableMember(context.Error.Message)) return;

                context.Handled = true;
                if (onIgnoredMember != null) onIgnoredMember(context.Path);
            };

            return settings;
        }

        /// <summary>True when the message describes a member the DTO does not declare.</summary>
        public static bool IsIgnorableMember(string errorMessage) =>
            errorMessage != null &&
            errorMessage.StartsWith(IgnoredMemberMessagePrefix, StringComparison.Ordinal);

        static void RaiseIgnoredMember(string path)
        {
            Action<string> handler = IgnoredMember;
            if (handler != null) handler(path);
        }

        /// <summary>Deserializes contract JSON with the <see cref="Runtime"/> profile.</summary>
        public static T DeserializeRuntime<T>(string json)
        {
            if (json == null) throw new ArgumentNullException(nameof(json));
            return JsonConvert.DeserializeObject<T>(json, Runtime);
        }

        /// <summary>Deserializes contract JSON with the <see cref="Strict"/> profile.</summary>
        public static T DeserializeStrict<T>(string json)
        {
            if (json == null) throw new ArgumentNullException(nameof(json));
            return JsonConvert.DeserializeObject<T>(json, Strict);
        }

        /// <summary>Deserializes contract JSON with the <see cref="Strict"/> profile.</summary>
        public static object DeserializeStrict(string json, Type type)
        {
            if (json == null) throw new ArgumentNullException(nameof(json));
            if (type == null) throw new ArgumentNullException(nameof(type));
            return JsonConvert.DeserializeObject(json, type, Strict);
        }

        /// <summary>Serializes a DTO back to contract JSON.</summary>
        public static string Serialize(object value) => JsonConvert.SerializeObject(value, Strict);

        /// <summary>
        /// Parses JSON into a <see cref="JToken"/> without letting the reader rewrite
        /// date-shaped strings. Every path that opens raw contract JSON as a token tree —
        /// dispatch, tests, tooling — goes through here rather than <c>JObject.Parse</c>.
        /// </summary>
        public static JToken ReadToken(string json)
        {
            if (json == null) throw new ArgumentNullException(nameof(json));
            using (var stringReader = new StringReader(json))
            using (var jsonReader = new JsonTextReader(stringReader))
            {
                jsonReader.DateParseHandling = DateParseHandling.None;
                jsonReader.FloatParseHandling = FloatParseHandling.Double;
                return JToken.ReadFrom(jsonReader);
            }
        }

        /// <summary>Parses JSON into a <see cref="JObject"/> without date rewriting.</summary>
        public static JObject ReadObject(string json)
        {
            JToken token = ReadToken(json);
            if (token is JObject obj) return obj;
            throw new JsonSerializationException(
                "Contract JSON must be an object at the top level, found " + token.Type + ".");
        }
    }
}
