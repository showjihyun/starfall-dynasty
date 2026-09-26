// Hand-written. Loads data/movement/sync-tuning.json (contracts/data/sync-tuning.schema.json).
// D-1 removed tick_hz, quantisation and integrator identity from this file on purpose - they
// are law (ADR-0009, ADR-0010), not tunables - so this loader does not look for them.
//
// AC-10(d): SYNC_TUNING is kind "data", so there is no generated DTO. This IS the client's
// only way to read snapshot_hz, remote_interp_delay_ms and the reconciliation thresholds
// (ADR-0012 section 7).

using System;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    public sealed class SyncTuningData
    {
        // snapshot
        public double SnapshotHz;
        public int MaxEntitiesPerSnapshot;
        public double EgressBudgetKibSPerSession;

        // input
        public double ClientSendHz;
        public int CarryForwardMaxTicks;
        public double RateLimitHz;
        public double ProtocolViolationHz;

        // prediction
        public double RemoteInterpDelayMs;
        public double RemoteExtrapolateMaxMs;

        public double ReconcileIgnoreThresholdM;
        public double ReconcileSmoothThresholdM;
        public double ReconcileSmoothDurationMs;
        public double ReconcileHardSnapThresholdM;

        public double ReconcileOrientationIgnoreThresholdDeg;
        public double ReconcileOrientationSmoothThresholdDeg;
        public double ReconcileOrientationHardSnapDeg;

        public static SyncTuningData FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject snapshot = Section(root, "snapshot");
            JObject input = Section(root, "input");
            JObject prediction = Section(root, "prediction");

            return new SyncTuningData
            {
                SnapshotHz = Required(snapshot, "snapshot_hz"),
                MaxEntitiesPerSnapshot = (int)Required(snapshot, "max_entities_per_snapshot"),
                EgressBudgetKibSPerSession = Required(snapshot, "egress_budget_kib_s_per_session"),

                ClientSendHz = Required(input, "client_send_hz"),
                CarryForwardMaxTicks = (int)Required(input, "carry_forward_max_ticks"),
                RateLimitHz = Required(input, "rate_limit_hz"),
                ProtocolViolationHz = Required(input, "protocol_violation_hz"),

                RemoteInterpDelayMs = Required(prediction, "remote_interp_delay_ms"),
                RemoteExtrapolateMaxMs = Required(prediction, "remote_extrapolate_max_ms"),

                ReconcileIgnoreThresholdM = Required(prediction, "reconcile_ignore_threshold_m"),
                ReconcileSmoothThresholdM = Required(prediction, "reconcile_smooth_threshold_m"),
                ReconcileSmoothDurationMs = Required(prediction, "reconcile_smooth_duration_ms"),
                ReconcileHardSnapThresholdM = Required(prediction, "reconcile_hard_snap_threshold_m"),

                ReconcileOrientationIgnoreThresholdDeg = Required(prediction, "reconcile_orientation_ignore_threshold_deg"),
                ReconcileOrientationSmoothThresholdDeg = Required(prediction, "reconcile_orientation_smooth_threshold_deg"),
                ReconcileOrientationHardSnapDeg = Required(prediction, "reconcile_orientation_hard_snap_deg"),
            };
        }

        static JObject Section(JObject root, string name) =>
            (JObject)root[name] ?? throw new InvalidOperationException("sync-tuning JSON has no '" + name + "' block");

        static double Required(JObject obj, string field)
        {
            JToken token = obj[field];
            if (token == null)
                throw new InvalidOperationException("sync-tuning block has no '" + field + "'");
            return (double)token;
        }
    }
}
