// Hand-written. C3 (p1-02-mining). MINE_RESOURCE rejection sentences for RejectReasonCode
// (contracts/messages/COMMAND_RESULT.schema.json). The set is NOT closed - the schema's own
// description says "values may be ADDED in a later version" (ADR-0005 section 4) - so a switch
// with no default would either throw or silently print nothing for a value this build predates.
// The fallback text applies the same principle C1 confirmed for the DTO layer (ReasonCode is a
// plain C# string, not an enum, precisely so an unknown value does not crash or get mistaken for
// acceptance): show it, reject it, never assume success (02_client_ack.md C1 "모르는 닫힌 값 생존").

using System;

namespace Starfall.Mining
{
    public static class RejectReasonText
    {
        /// <summary>Never throws, never returns null. An unrecognised code (including null,
        /// which the schema forbids for REJECTED but a future bug should still not crash the UI
        /// over) renders as the generic fallback sentence with the raw code appended for
        /// debugging.</summary>
        public static string Resolve(string reasonCode)
        {
            switch (reasonCode)
            {
                case "MALFORMED_COMMAND": return "명령 형식이 올바르지 않습니다.";
                case "UNKNOWN_COMMAND_TYPE": return "알 수 없는 명령입니다.";
                case "SCHEMA_VERSION_UNSUPPORTED": return "지원하지 않는 계약 버전입니다.";
                case "DUPLICATE_COMMAND_ID": return "이미 처리된 명령입니다.";
                case "SERVER_BUSY": return "서버가 혼잡합니다. 잠시 후 다시 시도하세요.";
                case "TOO_MANY_IN_FLIGHT": return "처리 대기 중인 명령이 너무 많습니다.";
                case "RATE_LIMITED": return "명령을 너무 자주 보냈습니다.";
                case "STALE_INPUT": return "오래된 입력입니다.";
                case "TARGET_UNKNOWN": return "이 성계에 그런 광맥이 없습니다.";
                case "COOLDOWN_ACTIVE": return "채굴 재사용 대기 중입니다.";
                case "TARGET_OUT_OF_RANGE": return "광맥이 사거리 밖에 있습니다.";
                case "SHIP_TOO_FAST": return "함선 속도가 너무 빠릅니다.";
                case "RESOURCE_DEPLETED": return "광맥이 고갈되었습니다.";
                case "RECORDING_BACKLOG": return "기록이 밀려 있어 잠시 후 다시 시도하세요.";
                case "CAPACITY_EXCEEDED": return "인벤토리 표현 한도를 초과합니다.";
                default: return "알 수 없는 사유로 거절되었습니다. (" + (reasonCode ?? "null") + ")";
            }
        }
    }
}
