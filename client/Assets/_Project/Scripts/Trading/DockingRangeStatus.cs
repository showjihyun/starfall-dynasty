// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-01/UI-02/UI-03: exchange marker
// geometry, off-screen distance label, and the docking distance/speed traffic lights. Mirrors
// Starfall.Mining.MiningRangeEvaluator's discipline (surface distance via sqrt is fine for
// DISPLAY even though the server's own judgement uses squared comparison, ADR-0010 section 3 -
// sqrt is monotonic so the true/false boundary is identical) but lives in Starfall.Trading
// rather than depending on Starfall.Mining, keeping the two feature modules sibling (no
// cross-reference), matching the asmdef graph's existing shape (Flight/Mining do not reference
// each other either).

using System.Globalization;

namespace Starfall.Trading
{
    /// <summary>UI-01: a market's marker geometry, entirely from data/ (public, ADR-0015) -
    /// never from a server message. The docking zone radius is radius_m + dock_range_from_surface_m
    /// (design doc section 8.3 UI-01: "도킹 구역(320 m)"), distinct from the marker's own
    /// radius_m (120 m) so the two shapes draw differently (ring+pillar vs wireframe sphere).</summary>
    public readonly struct ExchangeMarkerGeometry
    {
        public readonly double RadiusM;
        public readonly double DockingZoneRadiusM;

        public ExchangeMarkerGeometry(double radiusM, double dockRangeFromSurfaceM)
        {
            RadiusM = radiusM;
            DockingZoneRadiusM = radiusM + dockRangeFromSurfaceM;
        }
    }

    /// <summary>UI-01: the docking-zone wire sphere's two display colours - design doc section
    /// 8.3 UI-01 "도킹 구역 구는 도킹 판정(UI-03)이 모두 초록이면 색이 바뀐다". Pure function of
    /// DockingRangeStatus.BothGreen alone - this is a display decision, never the sale judgement
    /// itself (principle 1).</summary>
    public enum DockingZoneColor { Default, BothConditionsGreen }

    public static class ExchangeMarkerColorRule
    {
        public static DockingZoneColor Resolve(DockingRangeStatus status) =>
            status.BothGreen ? DockingZoneColor.BothConditionsGreen : DockingZoneColor.Default;
    }

    /// <summary>UI-02: "Cradle Exchange · 4.2 km" (surface distance) or metres below
    /// 2,000 m (design doc section 8.3 UI-02: "거리 &lt; 2 km면 m 단위"). The 2,000 m threshold
    /// is also UI-03's visibility gate (same number, different purpose - UI-02 picks the unit,
    /// UI-03 decides whether the docking HUD shows at all).</summary>
    public static class ExchangeDistanceLabel
    {
        public const double MetresVsKilometresThresholdM = 2000.0;

        public static string Format(string marketDisplayName, double surfaceDistanceM)
        {
            string distance = surfaceDistanceM < MetresVsKilometresThresholdM
                ? surfaceDistanceM.ToString("F0", CultureInfo.InvariantCulture) + " m"
                : (surfaceDistanceM / 1000.0).ToString("F1", CultureInfo.InvariantCulture) + " km";
            return marketDisplayName + " · " + distance;
        }

        /// <summary>UI-02: "표식이 화면 밖·뒤에 있으면 화면 가장자리에 화살표" - a pure function
        /// of the marker's VIEWPORT-space position (Camera.WorldToViewportPoint's result,
        /// computed by the caller - Starfall.Trading has noEngineReferences and cannot touch
        /// UnityEngine.Vector3 itself). viewportZ &lt;= 0 means behind the camera (off-screen
        /// regardless of x/y, which WorldToViewportPoint leaves meaningless there); otherwise
        /// off-screen iff x or y falls outside [0, 1].</summary>
        public static bool IsOffScreen(double viewportX, double viewportY, double viewportZ)
        {
            if (viewportZ <= 0.0) return true; // behind the camera
            return viewportX < 0.0 || viewportX > 1.0 || viewportY < 0.0 || viewportY > 1.0;
        }
    }

    /// <summary>UI-03: the two traffic-light conditions a sale is judged by for display -
    /// docking-zone distance and ship speed. Visible only within 2,000 m of the market's
    /// surface (design doc section 8.3 UI-03: "표면 거리 ≤ 2 km일 때만 보인다").</summary>
    public readonly struct DockingRangeStatus
    {
        public readonly double SurfaceDistanceM;
        public readonly bool WithinDockingZone;
        public readonly bool SpeedOk;
        public readonly bool Visible;

        public DockingRangeStatus(double surfaceDistanceM, bool withinDockingZone, bool speedOk, bool visible)
        {
            SurfaceDistanceM = surfaceDistanceM;
            WithinDockingZone = withinDockingZone;
            SpeedOk = speedOk;
            Visible = visible;
        }

        /// <summary>Both green - design doc section 8.3 UI-03: "두 ●이 모두 초록이면 UI-07이
        /// 열린다" (display gate only; the server is still the sole judge of SELL_MINERAL,
        /// principle 1).</summary>
        public bool BothGreen => WithinDockingZone && SpeedOk;
    }

    public static class DockingRangeEvaluator
    {
        public static DockingRangeStatus Evaluate(
            double distanceToMarketCenterM, double marketRadiusM, double shipSpeedMps,
            double dockRangeFromSurfaceM, double maxShipSpeedMps)
        {
            double surfaceDistanceM = distanceToMarketCenterM - marketRadiusM;
            bool withinDockingZone = surfaceDistanceM <= dockRangeFromSurfaceM;
            bool speedOk = shipSpeedMps <= maxShipSpeedMps;
            bool visible = surfaceDistanceM <= ExchangeDistanceLabel.MetresVsKilometresThresholdM;
            return new DockingRangeStatus(surfaceDistanceM, withinDockingZone, speedOk, visible);
        }
    }
}
