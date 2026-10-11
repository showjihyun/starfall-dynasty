// Hand-written. C2 (p1-03-trading). Mirrors Starfall.Mining.InventoryPanelState's single-mutator
// discipline (principle 1, AC-16(a)/I-78): current saturation and price change ONLY through
// ApplyMarketState - there is no other method that touches Listings, so this type structurally
// cannot "compute" a current price or saturation (I-78's own wording).
//
// Also tracks the previous message's listing set so NewListingDetector.Detect can be called by
// the caller with (previous, current) - this type does not call that detector itself, it only
// remembers "was there a previous message at all" (null on the very first message, matching
// NewListingDetector's "first message never notifies" contract).

using System.Collections.Generic;
using Starfall.Contracts.Generated;

namespace Starfall.Trading
{
    public readonly struct MarketListingView
    {
        public readonly string MineralId;
        public readonly long PriceCrPerKg;
        public readonly int SaturationKg;
        public readonly System.Guid? LastSaleActorId;
        public readonly int LastSaleQuantityKg;
        public readonly long LastSaleTick;

        public MarketListingView(string mineralId, long priceCrPerKg, int saturationKg,
            System.Guid? lastSaleActorId, int lastSaleQuantityKg, long lastSaleTick)
        {
            MineralId = mineralId;
            PriceCrPerKg = priceCrPerKg;
            SaturationKg = saturationKg;
            LastSaleActorId = lastSaleActorId;
            LastSaleQuantityKg = lastSaleQuantityKg;
            LastSaleTick = lastSaleTick;
        }
    }

    public sealed class MarketMirrorState
    {
        public IReadOnlyList<MarketListingView> Listings { get; private set; } = System.Array.Empty<MarketListingView>();

        /// <summary>null before the first ApplyMarketState call - NewListingDetector.Detect's
        /// "first message, never notify" contract depends on telling this apart from "previous
        /// message had zero listings" (an empty, non-null set).</summary>
        public IReadOnlyCollection<string> PreviousMineralIds { get; private set; }

        public long CurrentTick { get; private set; }

        bool _hasAppliedBefore;

        /// <summary>The only mutator.</summary>
        public void ApplyMarketState(MarketStateMessage message)
        {
            if (_hasAppliedBefore)
            {
                var previousIds = new HashSet<string>(System.StringComparer.Ordinal);
                foreach (MarketListingView existing in Listings) previousIds.Add(existing.MineralId);
                PreviousMineralIds = previousIds;
            }
            // else: stays null - this is the session's first message (NewListingDetector's
            // "first message, never notify" contract depends on null here, not an empty set).
            _hasAppliedBefore = true;

            var listings = new List<MarketListingView>(message.Payload.Listings.Length);
            foreach (var listing in message.Payload.Listings)
            {
                System.Guid? lastSaleActorId = null;
                int lastSaleQuantityKg = 0;
                long lastSaleTick = 0;
                if (listing.LastSale != null)
                {
                    lastSaleActorId = listing.LastSale.ActorId;
                    lastSaleQuantityKg = listing.LastSale.QuantityKg;
                    lastSaleTick = listing.LastSale.Tick;
                }
                listings.Add(new MarketListingView(
                    listing.MineralId, listing.PriceCrPerKg, listing.SaturationKg,
                    lastSaleActorId, lastSaleQuantityKg, lastSaleTick));
            }
            // UI-05 (design doc section 8.3): the ticker's row order is mineral_id ascending.
            // The schema's own description already promises the server sends it that way, but
            // the DISPLAY enforces it independently of that promise - never trusts message order.
            listings.Sort((a, b) => string.CompareOrdinal(a.MineralId, b.MineralId));
            Listings = listings;
            CurrentTick = message.Tick;
        }

        public bool TryFind(string mineralId, out MarketListingView listing)
        {
            foreach (MarketListingView candidate in Listings)
            {
                if (candidate.MineralId == mineralId) { listing = candidate; return true; }
            }
            listing = default;
            return false;
        }
    }
}
