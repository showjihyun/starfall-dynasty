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
        /// Live receive path: same date handling, but unknown members are ignored and counted
        /// so an additive (compatible) server change does not drop whole messages.
        /// Implemented in p0-02 together with the transport layer; this member exists so that
        /// slice does not have to re-litigate the default.
        /// </summary>
        public static JsonSerializerSettings Runtime =>
            throw new NotImplementedException(
                "The Runtime profile lands with the p0-02 transport layer. " +
                "p0-01 has no receive path, so only Strict is implemented (ADR-0002 section 4).");

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
