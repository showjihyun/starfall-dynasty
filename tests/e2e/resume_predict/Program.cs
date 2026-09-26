// qa 소유. SC-11 (3) 독립 계산 — 끊기기 직전 스냅샷 상태에서 [이월 입력 k tick → 휴면 입력] 으로
// N tick 을 적분해 **매 tick 의 양자화 상태**를 낸다. 비교(어느 tick 이 어느 스냅샷과 맞는가)는
// 호출자(tests/e2e/resume_check.py)가 한다.
//
// 휴면 입력은 ADR-0011 §6.1 표 그대로다: 추력·롤 0, 목표 자세 = **그 tick 의 현재 자세**,
// brake=false, flight_assist=true. 목표 자세는 와이어 정수로만 만들 수 있으므로(ShipControlInputD 의
// 설계) 현재 자세를 마이크로로 양자화해 넣는다 — 오차 ~1e-6 은 2단계 불감대 안이라 결과가 같다.
//
// 입력 JSON (파일 경로를 인자로):
//   { "ship_class_path": "...", "star_system_path": "...", "tick_hz": 20, "ticks": N,
//     "start": { position_x_mm, ..., angular_velocity_roll_mdeg_s },     // 와이어 ShipState 정수
//     "carry": { "ticks": k, "payload": { SET_SHIP_CONTROL payload } } }   // 선택
//     "perturb": { "count": K, "seed": s }   // 선택: T0 정수를 ±0.5 양자 흔든 K 개 시작점 → 봉투
//     "dormant_flight_assist": false           // 선택: 음성 대조(틀린 휴면 모델)
// 출력(stdout): { "states": [ { "offset": 1, position_x_mm, ... }, ... ],
//                 "envelope": [ { "offset": 1, "position_x_mm": [min, max], ... }, ... ] }

using System;
using System.IO;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using Starfall.Sim;

static class Program
{
    static long L(JObject o, string k) => (long)o[k];

    static readonly string[] Fields =
    {
        "position_x_mm", "position_y_mm", "position_z_mm",
        "velocity_x_mm_s", "velocity_y_mm_s", "velocity_z_mm_s",
        "orientation_x_micro", "orientation_y_micro", "orientation_z_micro", "orientation_w_micro",
        "angular_velocity_x_mdeg_s", "angular_velocity_y_mdeg_s", "angular_velocity_z_mdeg_s",
        "angular_velocity_roll_mdeg_s",
    };

    // 와이어 정수 + 양자 단위 흔들림 u(-0.5..0.5). u = 0 이면 ShipStateWire.ToSimState 와 같은 값이다
    // (정수를 같은 배율로 나눈 뒤 쿼터니언을 정규화한다).
    static ShipSimState FromWire(JObject w, Func<double> u)
    {
        double F(string k) => L(w, k) + u();
        return new ShipSimState(
            new Vec3d(F("position_x_mm") / Quantization.PositionScale,
                      F("position_y_mm") / Quantization.PositionScale,
                      F("position_z_mm") / Quantization.PositionScale),
            new Vec3d(F("velocity_x_mm_s") / Quantization.VelocityScale,
                      F("velocity_y_mm_s") / Quantization.VelocityScale,
                      F("velocity_z_mm_s") / Quantization.VelocityScale),
            new Quatd(F("orientation_x_micro") / Quantization.QuaternionScale,
                      F("orientation_y_micro") / Quantization.QuaternionScale,
                      F("orientation_z_micro") / Quantization.QuaternionScale,
                      F("orientation_w_micro") / Quantization.QuaternionScale).Normalized(),
            new Vec3d(F("angular_velocity_x_mdeg_s") / Quantization.AngularVelocityScale,
                      F("angular_velocity_y_mdeg_s") / Quantization.AngularVelocityScale,
                      F("angular_velocity_z_mdeg_s") / Quantization.AngularVelocityScale),
            F("angular_velocity_roll_mdeg_s") / Quantization.AngularVelocityScale);
    }

    static JObject ToWire(int offset, ShipSimState s) => new JObject
    {
        ["offset"] = offset,
        ["position_x_mm"] = Quantization.QuantizePosition(s.Position.X),
        ["position_y_mm"] = Quantization.QuantizePosition(s.Position.Y),
        ["position_z_mm"] = Quantization.QuantizePosition(s.Position.Z),
        ["velocity_x_mm_s"] = Quantization.QuantizeVelocity(s.Velocity.X),
        ["velocity_y_mm_s"] = Quantization.QuantizeVelocity(s.Velocity.Y),
        ["velocity_z_mm_s"] = Quantization.QuantizeVelocity(s.Velocity.Z),
        ["orientation_x_micro"] = Quantization.QuantizeQuaternionComponent(s.Orientation.X),
        ["orientation_y_micro"] = Quantization.QuantizeQuaternionComponent(s.Orientation.Y),
        ["orientation_z_micro"] = Quantization.QuantizeQuaternionComponent(s.Orientation.Z),
        ["orientation_w_micro"] = Quantization.QuantizeQuaternionComponent(s.Orientation.W),
        ["angular_velocity_x_mdeg_s"] = Quantization.QuantizeAngularVelocity(s.AngularVelocityAim.X),
        ["angular_velocity_y_mdeg_s"] = Quantization.QuantizeAngularVelocity(s.AngularVelocityAim.Y),
        ["angular_velocity_z_mdeg_s"] = Quantization.QuantizeAngularVelocity(s.AngularVelocityAim.Z),
        ["angular_velocity_roll_mdeg_s"] = Quantization.QuantizeAngularVelocity(s.AngularVelocityRoll),
    };

    static ShipControlInputD Payload(JObject p) => new ShipControlInputD(
        (uint)(long)p["input_seq"],
        L(p, "thrust_x_milli"), L(p, "thrust_y_milli"), L(p, "thrust_z_milli"),
        L(p, "roll_milli"),
        L(p, "aim_x_micro"), L(p, "aim_y_micro"), L(p, "aim_z_micro"), L(p, "aim_w_micro"),
        (bool)p["brake"], (bool)p["flight_assist"]);

    static ShipControlInputD Dormant(Quatd q, bool assist) => new ShipControlInputD(
        0, 0, 0, 0, 0,
        Quantization.QuantizeQuaternionComponent(q.X),
        Quantization.QuantizeQuaternionComponent(q.Y),
        Quantization.QuantizeQuaternionComponent(q.Z),
        Quantization.QuantizeQuaternionComponent(q.W),
        false, assist);

    static JObject[] Run(ShipSimState state, int ticks, int carryTicks, ShipControlInputD carry,
                         bool assist, ShipClassStats ship, ShipIntegrator.Boundary boundary, double dt)
    {
        var outp = new JObject[ticks];
        for (int i = 1; i <= ticks; i++)
        {
            ShipControlInputD input = i <= carryTicks ? carry : Dormant(state.Orientation, assist);
            state = ShipIntegrator.Step(state, input, ship, boundary, dt).State;
            outp[i - 1] = ToWire(i, state);
        }
        return outp;
    }

    static int Main(string[] args)
    {
        if (args.Length != 1)
        {
            Console.Error.WriteLine("usage: ResumePredict <request.json>");
            return 2;
        }
        JObject req = JObject.Parse(File.ReadAllText(args[0]));
        ShipClassStats ship = ShipClassStats.FromJson(File.ReadAllText((string)req["ship_class_path"]));
        StarSystemData sys = StarSystemData.FromJson(File.ReadAllText((string)req["star_system_path"]));
        var boundary = new ShipIntegrator.Boundary(sys.SoftBoundaryRadiusM, sys.HardBoundaryRadiusM, sys.BoundaryPullMps2);
        double dt = 1.0 / (int)req["tick_hz"];
        int ticks = (int)req["ticks"];
        JObject start = (JObject)req["start"];
        // 음성 대조용: false 로 주면 잘못된 휴면 모델(보조 끔)로 돈다 — 대조가 틀린 모델을 잡는가.
        bool assist = req["dormant_flight_assist"] == null || (bool)req["dormant_flight_assist"];

        int carryTicks = 0;
        ShipControlInputD carry = default;
        if (req["carry"] is JObject c)
        {
            carryTicks = (int)c["ticks"];
            carry = Payload((JObject)c["payload"]);
        }

        JObject[] center = Run(FromWire(start, () => 0.0), ticks, carryTicks, carry, assist, ship, boundary, dt);

        // 양자화 봉투: T0 스냅샷은 서버 f64 상태를 반올림한 것이라, 진짜 시작 상태는 각 정수의
        // ±0.5 양자 안 어딘가다. 그 상자에서 시작점을 뽑아 같은 적분을 돌린 결과의 [min, max] 가
        // "스냅샷에서 출발한 독립 계산이 낼 수 있는 값"의 범위다(허용 오차를 고르는 것이 아니라
        // 계약 전제 — 양자화 — 에서 유도한다).
        int count = req["perturb"] is JObject pj ? (int)pj["count"] : 0;
        int seed = req["perturb"] is JObject pk ? (int)pk["seed"] : 1;
        var rng = new Random(seed);
        var lo = new long[ticks, Fields.Length];
        var hi = new long[ticks, Fields.Length];
        for (int i = 0; i < ticks; i++)
            for (int f = 0; f < Fields.Length; f++)
                lo[i, f] = hi[i, f] = (long)center[i][Fields[f]];
        for (int k = 0; k < count; k++)
        {
            // 절반은 상자의 꼭짓점(±0.5), 절반은 내부 균등 — 꼭짓점이 극값을 내기 쉽다.
            Func<double> u = k % 2 == 0
                ? () => rng.Next(2) == 0 ? -0.4999 : 0.4999
                : () => rng.NextDouble() - 0.5;
            JObject[] run = Run(FromWire(start, u), ticks, carryTicks, carry, assist, ship, boundary, dt);
            for (int i = 0; i < ticks; i++)
                for (int f = 0; f < Fields.Length; f++)
                {
                    long v = (long)run[i][Fields[f]];
                    if (v < lo[i, f]) lo[i, f] = v;
                    if (v > hi[i, f]) hi[i, f] = v;
                }
        }
        var env = new JArray();
        for (int i = 0; i < ticks; i++)
        {
            var e = new JObject { ["offset"] = i + 1 };
            for (int f = 0; f < Fields.Length; f++)
                e[Fields[f]] = new JArray(lo[i, f], hi[i, f]);
            env.Add(e);
        }
        Console.WriteLine(new JObject
        {
            ["dt"] = dt,
            ["dormant_flight_assist"] = assist,
            ["perturb_count"] = count,
            ["states"] = new JArray(center),
            ["envelope"] = env,
        }.ToString(Formatting.None));
        return 0;
    }
}
