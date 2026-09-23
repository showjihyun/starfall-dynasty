// Hand-written (not generated). Opens raw contract JSON, reads the envelope type constant
// and turns it into a generated DTO.
//
// This is the function the p0-02 transport layer will call on every received frame. The
// date-preservation guard in the EditMode tests sits on this code path on purpose: a guard
// on a test-only helper would not protect anything.

using System;
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using Starfall.Contracts.Generated;

namespace Starfall.Contracts
{
    /// <summary>Envelope type discriminator -> generated DTO, driven by the contract registry.</summary>
    public static class ContractDispatch
    {
        /// <summary>The envelope properties that carry a registry type name, in lookup order.</summary>
        public static readonly string[] TypeProperties = { "message_type", "command_type", "event_type" };

        /// <summary>
        /// Registry types read directly from the JSON string instead of through a JObject
        /// tree (R4, p1-01-ship-movement sprint contract section 0.7 "핫 경로"). Measured on
        /// this runtime (client, 2026-09-20, Unity-bundled Mono, 31-ship WORLD_SNAPSHOT, 200
        /// reps): tree ("JObject then ToObject", the general path below) costs 0.656 ms/message
        /// and 8 gen0 collections per 200 messages; deserializing straight from the string
        /// costs 0.265 ms/message and 1 gen0 collection per 200 - the difference is garbage
        /// (the tree is built and then thrown away), not CPU. WORLD_SNAPSHOT is the only type
        /// both big (~16 KiB at 31 ships) and frequent (10 Hz) enough for that to matter; every
        /// other type keeps the tree path, which is simpler and cheap enough at its size/rate.
        /// </summary>
        public static readonly HashSet<string> DirectDeserializeTypes =
            new HashSet<string>(StringComparer.Ordinal) { "WORLD_SNAPSHOT" };

        /// <summary>
        /// Reads only the envelope type discriminator, without materialising a JObject tree.
        /// A forward-only token scan: for a real envelope the discriminator is one of the
        /// first properties (envelope fields precede "payload" in every contract in this
        /// project), so in practice this touches a few dozen bytes even though "payload" may
        /// be tens of kilobytes - JsonTextReader.Skip() moves past nested containers without
        /// allocating nodes for them.
        /// </summary>
        /// <summary>May throw <see cref="JsonException"/> on malformed JSON - callers on the
        /// receive path use <see cref="TryRead(string, JsonSerializer, out string, out object, out string)"/>,
        /// which catches it and reports "not valid contract JSON" the same way the old
        /// tree-based path did, rather than swallowing the distinction here.</summary>
        public static bool TryPeekTypeName(string json, out string typeName)
        {
            typeName = null;
            if (json == null) return false;

            using (var stringReader = new StringReader(json))
            using (var jsonReader = new JsonTextReader(stringReader) { DateParseHandling = DateParseHandling.None })
            {
                if (!jsonReader.Read() || jsonReader.TokenType != JsonToken.StartObject) return false;

                while (jsonReader.Read() && jsonReader.TokenType == JsonToken.PropertyName)
                {
                    string propertyName = (string)jsonReader.Value;
                    bool isTypeProperty = Array.IndexOf(TypeProperties, propertyName) >= 0;

                    if (!jsonReader.Read()) return false; // advance to the value token

                    if (isTypeProperty && jsonReader.TokenType == JsonToken.String)
                    {
                        typeName = (string)jsonReader.Value;
                        return !string.IsNullOrEmpty(typeName);
                    }

                    if (jsonReader.TokenType == JsonToken.StartObject || jsonReader.TokenType == JsonToken.StartArray)
                        jsonReader.Skip();
                }
            }
            return false;
        }

        /// <summary>
        /// Reads the registry type name out of an already-parsed envelope.
        /// Returns false rather than throwing: an unknown or malformed frame must not take
        /// the connection down.
        /// </summary>
        public static bool TryGetTypeName(JObject envelope, out string typeName)
        {
            typeName = null;
            if (envelope == null) return false;

            for (int i = 0; i < TypeProperties.Length; i++)
            {
                JToken token = envelope[TypeProperties[i]];
                if (token != null && token.Type == JTokenType.String)
                {
                    typeName = (string)token;
                    return !string.IsNullOrEmpty(typeName);
                }
            }
            return false;
        }

        /// <summary>
        /// Parses raw contract JSON and deserializes it into the DTO registered for its type
        /// constant, using the <see cref="ContractJson.Strict"/> profile.
        /// </summary>
        /// <returns>false with <paramref name="error"/> set when the frame is unreadable,
        /// carries no known type, or violates the contract.</returns>
        public static bool TryRead(string json, out string typeName, out object contract, out string error) =>
            TryRead(json, JsonSerializer.Create(ContractJson.Strict), out typeName, out contract, out error);

        /// <summary>
        /// Same as <see cref="TryRead(string, out string, out object, out string)"/> with a
        /// caller-supplied serializer, so the live receive path can build one
        /// <see cref="ContractJson.Runtime"/> serializer at startup and reuse it for every
        /// frame instead of allocating settings per message.
        /// <para>
        /// For a type in <see cref="DirectDeserializeTypes"/> this deserializes straight from
        /// the string (R4) - it never builds a JObject tree, so
        /// <see cref="ContractJson.ReadObject"/>'s date-preservation guard does not apply and
        /// is not needed: the DTO's string-typed date fields still come through
        /// <see cref="DateParseHandling.None"/> because that is set on the passed-in
        /// serializer/settings, not on the tree reader. Every other type keeps the original
        /// tree-based path unchanged, including the RealTime-preservation behaviour p0-02's
        /// tests depend on.
        /// </para>
        /// </summary>
        public static bool TryRead(
            string json,
            JsonSerializer serializer,
            out string typeName,
            out object contract,
            out string error)
        {
            typeName = null;
            contract = null;
            error = null;

            if (serializer == null) throw new ArgumentNullException(nameof(serializer));

            string peeked;
            try
            {
                if (!TryPeekTypeName(json, out peeked))
                {
                    error = "envelope carries none of: " + string.Join(", ", TypeProperties);
                    return false;
                }
            }
            catch (JsonException ex)
            {
                // Same wording the tree-based path has always used for this case
                // (ContractJson.ReadObject's catch, below) - RealtimeClient's "unreadable
                // frame" log line matches on this text.
                error = "not valid contract JSON: " + ex.Message;
                return false;
            }

            if (!DirectDeserializeTypes.Contains(peeked))
            {
                // General path: unchanged behaviour, tree first, then ToObject.
                JObject envelope;
                try
                {
                    envelope = ContractJson.ReadObject(json);
                }
                catch (JsonException ex)
                {
                    error = "not valid contract JSON: " + ex.Message;
                    return false;
                }

                return TryRead(envelope, serializer, out typeName, out contract, out error);
            }

            Type dtoType;
            if (!ContractTypes.ByName.TryGetValue(peeked, out dtoType))
            {
                error = "unknown contract type '" + peeked + "'";
                return false;
            }

            // serializer.Deserialize(JsonReader, Type) - the actual "no tree" step. Unlike
            // JsonConvert.DeserializeObject(json, type, serializer), which does not exist as an
            // overload, this reads token by token straight into the DTO; no JObject/JToken
            // node is ever allocated for "payload" or its 31-element "ships" array.
            using (var stringReader = new StringReader(json))
            using (var jsonTextReader = new JsonTextReader(stringReader) { DateParseHandling = DateParseHandling.None })
            {
                try
                {
                    contract = serializer.Deserialize(jsonTextReader, dtoType);
                }
                catch (JsonException ex)
                {
                    error = peeked + " did not match the generated DTO: " + ex.Message;
                    return false;
                }
            }

            if (contract == null) return false;
            typeName = peeked;
            return true;
        }

        /// <summary>
        /// Same as <see cref="TryRead(string, out string, out object, out string)"/> for an
        /// envelope that was already opened with <see cref="ContractJson.ReadObject"/>.
        /// </summary>
        public static bool TryRead(JObject envelope, out string typeName, out object contract, out string error) =>
            TryRead(envelope, JsonSerializer.Create(ContractJson.Strict), out typeName, out contract, out error);

        /// <summary>
        /// Same, with a caller-supplied serializer so the live receive path can build one
        /// <see cref="ContractJson.Runtime"/> serializer at startup and reuse it for every
        /// frame instead of allocating settings per message.
        /// </summary>
        public static bool TryRead(
            JObject envelope,
            JsonSerializer serializer,
            out string typeName,
            out object contract,
            out string error)
        {
            typeName = null;
            contract = null;
            error = null;

            if (serializer == null) throw new ArgumentNullException(nameof(serializer));

            if (!TryGetTypeName(envelope, out typeName))
            {
                error = "envelope carries none of: " + string.Join(", ", TypeProperties);
                return false;
            }

            Type dtoType;
            if (!ContractTypes.ByName.TryGetValue(typeName, out dtoType))
            {
                // Not a failure of this client: the server may be deployed first and may know
                // types we do not. Callers log and continue.
                error = "unknown contract type '" + typeName + "'";
                return false;
            }

            try
            {
                contract = envelope.ToObject(dtoType, serializer);
            }
            catch (JsonException ex)
            {
                error = typeName + " did not match the generated DTO: " + ex.Message;
                return false;
            }

            return contract != null;
        }
    }
}
