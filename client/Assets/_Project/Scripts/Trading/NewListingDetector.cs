// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-10: a mineral appearing in a
// MARKET_STATE message's listings that was NOT in the previous one is a "new listing" notice -
// except the session's FIRST message, which never fires the notice (those are pre-existing
// listings, not new).

using System.Collections.Generic;
using System.Linq;

namespace Starfall.Trading
{
    public static class NewListingDetector
    {
        /// <summary>previousMineralIds is null for the session's first MARKET_STATE message -
        /// callers pass null explicitly (not an empty set) so this can tell "first message,
        /// never notify" apart from "second message, previous had zero listings".</summary>
        public static IReadOnlyList<string> Detect(IReadOnlyCollection<string> previousMineralIds, IReadOnlyCollection<string> currentMineralIds)
        {
            var result = new List<string>();
            if (previousMineralIds == null) return result;

            foreach (string mineralId in currentMineralIds)
                if (!previousMineralIds.Contains(mineralId))
                    result.Add(mineralId);
            return result;
        }
    }
}
