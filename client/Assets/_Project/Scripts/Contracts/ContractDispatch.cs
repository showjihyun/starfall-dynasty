// Hand-written (not generated). Opens raw contract JSON, reads the envelope type constant
// and turns it into a generated DTO.
//
// This is the function the p0-02 transport layer will call on every received frame. The
// date-preservation guard in the EditMode tests sits on this code path on purpose: a guard
// on a test-only helper would not protect anything.

using System;
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
        /// constant. The JSON is opened with <see cref="ContractJson.ReadObject"/>, so
        /// RealTime / GameTime strings keep their exact wire form.
        /// </summary>
        /// <returns>false with <paramref name="error"/> set when the frame is unreadable,
        /// carries no known type, or violates the contract.</returns>
        public static bool TryRead(string json, out string typeName, out object contract, out string error)
        {
            typeName = null;
            contract = null;
            error = null;

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

            return TryRead(envelope, out typeName, out contract, out error);
        }

        /// <summary>
        /// Same as <see cref="TryRead(string, out string, out object, out string)"/> for an
        /// envelope that was already opened with <see cref="ContractJson.ReadObject"/>.
        /// </summary>
        public static bool TryRead(JObject envelope, out string typeName, out object contract, out string error)
        {
            typeName = null;
            contract = null;
            error = null;

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
                contract = envelope.ToObject(dtoType, JsonSerializer.Create(ContractJson.Strict));
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
