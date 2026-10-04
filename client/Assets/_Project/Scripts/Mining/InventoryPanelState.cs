// Hand-written. C3 (p1-02-mining). The inventory panel's only source of truth is
// INVENTORY_STATE (principle 1: client mirrors server state, never computes it). Sprint
// contract's chosen RED test for C3 (01_architect_tasks.md): "인벤토리 패널이 INVENTORY_STATE
// 수신 전에는 바뀌지 않는다" - after MINE_RESOURCE is sent, and after COMMAND_RESULT (even
// ACCEPTED) arrives, the panel must still show the OLD numbers until INVENTORY_STATE itself
// lands. The only way to enforce that structurally is to give this type exactly one mutator.

using System.Collections.Generic;
using Starfall.Contracts.Generated;

namespace Starfall.Mining
{
    public readonly struct InventoryItemView
    {
        public readonly string MineralId;
        public readonly int QuantityKg;

        public InventoryItemView(string mineralId, int quantityKg)
        {
            MineralId = mineralId;
            QuantityKg = quantityKg;
        }
    }

    /// <summary>Mutable, but only through <see cref="ApplyInventoryState"/> - there is no other
    /// method that touches <see cref="Items"/>. A caller that wants to reflect "I just mined
    /// something" before the server confirms it has no method to call here; that is the point
    /// (optimistic-update guard, design doc / principle 1).</summary>
    public sealed class InventoryPanelState
    {
        public IReadOnlyList<InventoryItemView> Items { get; private set; } = System.Array.Empty<InventoryItemView>();

        /// <summary>The only mutator. Replaces the panel's contents wholesale with what the
        /// server just said - INVENTORY_STATE.payload.items is already "one entry per mineral
        /// held, 0 kg = no entry" (contract description), so no merge logic is needed here.</summary>
        public void ApplyInventoryState(InventoryStateMessage message)
        {
            var items = new List<InventoryItemView>(message.Payload.Items.Length);
            foreach (InventoryStateMessage.InventoryStatePayload.InventoryItem item in message.Payload.Items)
                items.Add(new InventoryItemView(item.MineralId, item.QuantityKg));
            Items = items;
        }
    }
}
