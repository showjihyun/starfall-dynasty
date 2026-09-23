// Hand-written. Loads every data/ships/*.json under a directory and indexes by the file's
// `id` field - NOT the file name (spec: "파일 이름이 아니라 파일 안의 id 필드가 키다" - a
// renamed file must not break a wire ship_class_id lookup).

using System;
using System.Collections.Generic;
using System.IO;

namespace Starfall.Sim
{
    public sealed class ShipClassCatalog
    {
        readonly Dictionary<string, ShipClassStats> _byId;

        ShipClassCatalog(Dictionary<string, ShipClassStats> byId) => _byId = byId;

        public static ShipClassCatalog LoadFromDirectory(string shipsDir)
        {
            var byId = new Dictionary<string, ShipClassStats>(StringComparer.Ordinal);
            if (!Directory.Exists(shipsDir)) return new ShipClassCatalog(byId);

            foreach (string file in Directory.GetFiles(shipsDir, "*.json", SearchOption.TopDirectoryOnly))
            {
                ShipClassStats stats = ShipClassStats.FromJson(File.ReadAllText(file));
                byId[stats.Id] = stats; // last one wins; duplicate-id rejection is the server's job (I-38), not the client's
            }
            return new ShipClassCatalog(byId);
        }

        public bool TryGet(string shipClassId, out ShipClassStats stats) => _byId.TryGetValue(shipClassId, out stats);

        public int Count => _byId.Count;
    }
}
