# 블록 7 조종 절차 한 장 — SC-61 · SC-62 · SC-64 · SC-65

> **이 한 장이 세션의 전부다.** 이 슬라이스에서 절차가 부정확해 세션을 두 번 버렸다.
> **SC-63 은 이 세션의 항목이 아니다**(13차 개정 → 블록 10). 블록 7 리포트에 SC-63 verdict 를 적지 않는다.

---

## 0. 먼저 알아야 하는 것 — **화면에 아무것도 안 보인다**

`ObserverSession` 은 `GreyboxSession` 에서 **씬 비주얼·카메라·HUD 를 빼낸** 버전이다
(`ObserverSession.cs:1-10`, `03_client_impl.md` §"ObserverSession.cs"). **SC-59 때 보던 HUD 는
여기 없다** — `OnGUI` 는 `GreyboxSession.cs:1141` 에만 있다.

**그러므로 조종사는 계기 없이 난다.** 피드백은 두 곳뿐이다:

| 채널 | 무엇을 보는가 |
|---|---|
| **Unity Console** | 접속·세션·재조정 이벤트 로그 (`starfall.observer.A: ...`) |
| **`observer-a.csv` 를 tail** | 매 행 flush 된다(`ObserverCsv.cs:93-95` 가 "사람이 tail 할 수 있게"라고 적고 있다). **속도를 읽는 유일한 실시간 수단** |

→ **두 사람이 필요하거나, 조종사가 두 번째 모니터에 tail 창을 띄워야 한다.** 관제 명령은 §3 에 있다.

---

## 1. 환경 (Play 누르기 **전**)

```
STARFALL_TWO_SESSION_AUTOBUILD=1
STARFALL_DEV_AUTH_SECRET=<.env 의 값>
```

- `STARFALL_GREYBOX_AUTOBUILD` 와 `STARFALL_NET_AUTOCONNECT` 는 **켜지 않는다.**
- **SC-59 녹화 세션(블록 6)과 동시에 돌리지 않는다.** 둘 다 같은 기본 subject 로 인증하므로
  나중에 뜬 쪽이 먼저 뜬 쪽을 **close 4001 로 밀어낸다**(`03_client_impl.md` §6 "운영 주의사항 1건",
  SC-88 이 설계한 동작 — 버그가 아니다). 블록 6 창을 닫고 시작한다.
- 환경 변수는 **Unity Editor 를 띄우는 셸**에 설정한다. Editor 가 이미 떠 있으면 껐다 켠다.
- 서버가 떠 있어야 한다: `cargo run -p starfall-game-server` (`server/` 에서). 종료는 stdin 에
  `shutdown` 한 줄 — **하드 킬 금지.**
- CSV 기본 경로: `_workspace/p1-01-ship-movement/two-session/observer-{a,b}.csv`
  (`STARFALL_OBSERVER_A_CSV` / `..._B_CSV` 로 재정의 가능).

---

## 2. 비행 **전** 확인 — 여섯 줄. **하나라도 아니면 날지 않는다**

> R10 후속의 교훈: 날고 나서 CSV 를 열어 보면 세션 하나를 통째로 버린다.
> 아래는 전부 **Play 누른 직후 5초 안에** 확인된다.

| # | 어디서 | 보여야 하는 것 | 아니면 |
|---|---|---|---|
| 1 | Console | `starfall.two_session: booting A(subject=…, csv=…) and B(subject=…, csv=…)` **한 줄** | `AUTOBUILD` 가 안 먹었다 → Play 중지, 환경 변수 다시 |
| 2 | Console | 그 줄의 **subject 두 개가 서로 다르다** (끝 한 hex 자리 차이) | 하네스가 A·B 를 같은 actor 로 띄웠다 → 즉시 중지 |
| 3 | Console | `starfall.observer.A: session ready, actor_id=…` 와 `starfall.observer.B: session ready, actor_id=…` **둘 다**, **actor_id 가 서로 다르다** | 한 줄만 뜨면 서버가 한쪽을 거부했다 |
| 4 | Console | **`SUPERSEDED` · close `4001` 이 없다** | 블록 6 세션이 살아 있다(§1) |
| 5 | 파일 | 두 CSV 가 생겼고 **첫 줄이 12열 헤더**(`…,render_offset_mm,render_offset_deg`) | client 쪽 헤더가 옛 10열이다 → 판정 도구가 `NotImplementedYet` 으로 죽는다 |
| 6 | 파일 | **`observer-b.csv` 에 `ship_id` 가 2종**(B 자기 함선 + A 의 함선) | **B 가 A 를 못 본다** → SC-61 이 `B 의 CSV 에 함선 … 이 없다` 로 죽는다. 서버가 두 세션을 같은 성계에 스폰하지 않은 것이다 — client 가 "블록 7 에서 가장 먼저 볼 두 가지"로 적어 둔 위험 (2) 다 |

**5·6 확인 명령** (레포 루트, 새 터미널):

```
head -1 _workspace/p1-01-ship-movement/two-session/observer-b.csv
awk -F, 'NR>1{print $3}' _workspace/p1-01-ship-movement/two-session/observer-b.csv | sort -u | wc -l
```

`2` 가 나와야 한다. **1 이면 날지 않는다.**

---

## 3. 관제 창 — 속도를 읽는 법 (조종 중 계속 켜 둔다)

```
# Git Bash, 레포 루트. A 의 CSV 를 실시간으로 읽는다. **ship 별로 한 줄씩 나온다** —
# 속도가 올라가는 쪽이 A 의 함선이고, 계속 0 인 쪽이 B 의 함선이다(그 자체가 SC-62 의 실시간 확인).
tail -f _workspace/p1-01-ship-movement/two-session/observer-a.csv \
 | awk -F, 'NR>1{v=sqrt($8*$8+$9*$9+$10*$10)/1000; printf "tick=%s ship=%.8s speed=%6.1f m/s off=%s mm\n",$1,$3,v,$11}'
```

읽는 값 셋:
- `speed` — **SC-65 는 100 m/s 이상**에서만 표본을 만든다. **SC-64 는 1 m/s 미만**에서만 만든다.
- `off` (`render_offset_mm`) — §5 의 판정에 쓴다. **비행 중 한 번이라도 0 이 아닌 값을 봤는지 기억한다.**

---

## 4. 조종 — 정확히 이 순서, 이 시간

**키 배치**(`ShipInputSampler.cs:99-110`): `W` 전방 · `S` 후방 · `A/D` 좌/우 · `R/F` 상/하 ·
`Q/E` 롤 · `X` 브레이크 · `Z` 비행 보조 토글. 조준은 **마우스**.

> **마우스를 건드리지 않는다.** 조준이 돌면 궤적이 휘고, SC-61 의 "1 m 이상 되돌아감" 단조 검사가
> 그 곡선에서 걸린다. 이 세션에서 재는 것은 선회가 아니다.

| 단계 | 무엇을 | 얼마나 | 만드는 표본 |
|---|---|---|---|
| **① 정지 유지** | 아무 키도 누르지 않는다. 마우스에서 손을 뗀다 | **15초** (관제 창 `speed` 가 계속 `0.0`) | **SC-64** (정지 비교 ≤ 2 m) |
| **② 전방 추력** | `W` 를 **누른 채 유지** | **30초** — 손을 떼지 않는다 | **SC-61** (단조·이동 거리) |
| **③ 순항 확인** | ② 안에서, 관제 창이 `speed` **140.0** 근처로 올라가는지 본다(0→140 에 **4.0초**) | ②의 나머지 ≈26초 | **SC-65** (뒤쪽 28 ± 12 m) |
| **④ 제동** | `W` 를 떼고 **`X` 를 누른 채 유지** | 관제 창 `speed` 가 **1.0 미만**이 될 때까지 (최대 감속 50 m/s² → **약 3초**) | SC-64 표본 보강 |
| **⑤ 정지 유지** | 아무 키도 누르지 않는다 | **15초** | **SC-64** |

**총 약 78초.** ②를 30초보다 짧게 하면 SC-61 의 근거 AC(AC-16 "30초 전방 추력")를 벗어난다.

**이 78초가 SC-62 의 두 방어를 동시에 만족시킨다** — 계약 15차가 요구하는 수다:
- **(i) B 자기 함선 행의 tick 스팬 ≥ 400 tick(20초)** 이고 A 행 스팬의 **90 % 이상**. 세션 전체를
  끊김 없이 관측하면 자동으로 만족한다. **중간에 Play 를 멈췄다 다시 켜면 못 만족한다.**
- **(ii) A 의 순변위 ≥ 100 m.** ②의 30초 전방 추력이면 **약 4 km** 라 여유가 크다. 다만
  **순변위**이므로 왕복 비행은 안 된다 — §4 가 마우스를 금지하는 또 하나의 이유다.

> ### ⚠ 세션을 죽이는 것 — **창을 전환하지 마라** (R15 세션에서 실제로 일어났다)
>
> Unity 프로젝트가 **`runInBackground: 0`** 이다. 조종 중 **다른 창을 클릭하는 순간 `Update()` 가
> 멈추고 CSV 기록이 끊긴다.** 다시 돌아오면 재개되므로 **파일은 멀쩡해 보이고 스팬도 그대로다** —
> 가운데가 비어 있을 뿐이다.
>
> **계약 (i) 은 이것을 통과시킨다.** 스팬 400 tick 과 커버리지 90 % 는 **양 끝만** 본다.
> 실행으로 확인했다: 창 한가운데 242 tick 을 통째로 비워도 **SC-62 가 PASS 로 나온다**
> (selftest `mid_window_gap_still_passes_gate_i = PASS`).
>
> **그래서 도구가 관측을 낸다**: `SC-62.recorded_row_gaps_ticks`. 정상 세션은 **2**(스냅샷이
> 2 tick 마다)이고, 끊긴 세션은 훨씬 크다. **§6 에서 이 값을 반드시 적는다.**
>
> **조종사에게**: 관제 창(§3)은 **다른 모니터**에 두거나, 없으면 **tail 을 보지 말고 시간으로
> 센다**(②는 30초, ④는 3초). Alt-Tab 한 번이 세션 하나다.

**하지 말 것:**
- 마우스 이동 · `Q`/`E`(롤) · `A/D/R/F`(측면·상하) — 궤적이 휜다.
- `Z`(비행 보조 토글) — ④의 감속 모델이 바뀐다.
- 경계를 넘지 않는다. 스폰은 **반경 2,500 m 링** 위이고(`data/world/systems/cradle.json:26`),
  30초 전방 추력의 이동 거리는 최대 **약 4.2 km**, soft 경계는 **10 km** · hard 는 **12 km** 다
  (`:16-17`). 최악(바깥 방향)이라도 2.5 + 4.2 = **6.7 km** 로 여유가 있다. **②를 60초 넘게 끌면**
  경계 끌림(`boundary_pull_mps2 25.0`)이 섞여 SC-61 의 단조·거리 검사가 무의미해진다.

---

## 5. 비행 **후** — 순서를 지킨다

1. **Play 를 정상으로 중지한다**(Editor 를 죽이지 않는다). 그래야 Console 에 세션 종료 로그가 찍힌다:

   ```
   starfall.observer.A: SC-56 (e) session-end render smoothing stats - render_smooth_band_total=…,
   render_offset_nonzero_frames_total=…, render_offset_max_m=… (n=…), render_offset_decay_frames_total=…
   ```

   **이 한 줄이 §6 의 세 갈래 구분에 쓰는 유일한 증거다**(`ObserverSession.cs:160-172` 가
   "Observer 에는 HUD 도 `RenderLocalShip()` 도 없어 **여기 말고는 감쇠 루프가 돌았는지 확인할 곳이 없다**"
   고 적고 있다). **A·B 두 줄 모두 Console 에서 복사해 증거에 남긴다.**

2. 두 CSV 를 증거 디렉터리로 복사한다(원본을 덮어쓰기 전에).

3. 판정 도구를 돌린다(`--ship` 은 **A 의 `ship_id`**. `observer-a.csv` 에서 B 것이 아닌 쪽):

   ```
   python tests/e2e/two_client_view.py compare --a <A.csv> --b <B.csv> --ship <A의 ship_id> \
     --evidence <증거>/compare.json
   python tests/e2e/two_client_view.py s3 --a <A.csv> --b <B.csv> --ship <A의 ship_id> \
     --evidence <증거>/s3.json
   ```

   종료 코드: `0` 통과 / `1` FAIL / `4` **미검증** (표본 없음·판정 기준 미지정) / `3` 헤더 불일치.

---

## 6. 리포트 필수 기재 — **여덟 줄** (자명 통과 방지)

> qa r11 §4 의 표를 architect 가 승인하며 두 줄 고쳤고(R13 §5.2), R14 에서 두 줄이 더 붙었다.

| 적을 것 | 어디서 | 왜 |
|---|---|---|
| `pairs_compared` | `compare` | **0 이면 SC-61·62 는 `미검증(표본 없음)` 이지 PASS 가 아니다** |
| `s3` 의 `SC-64.samples` · `SC-65.samples` | `s3` | 둘 다 `미검증(표본 없음)` 을 낼 수 있고, **그것은 조종을 안 한 것이다**(§4 ①·③을 건너뛴 세션) |
| **SC-65 표본의 `speed_mps` 최소값** | `s3` → `SC-65.samples_head` | `--moving-speed-mps 100` **문턱 위에서** 실제로 순항했는가 |
| `max_render_offset_mm` · `render_offset_attribution` | `s3` → `SC-64` | SC-64 가 2 m 를 넘었을 때 **귀속의 유일한 근거** |
| **B 가 봇이 아니라 두 번째 Observer 세션**임을 명시 | §2 확인 1·3 의 Console 줄 | 봇이면 SC-65 는 부호가 뒤집혀 `미검증(환경, E8)` 이다(계약 §0.11) |
| ~~SC-63 verdict~~ | — | **적지 않는다.** 13차 개정으로 블록 10 항목이다 |
| **`render_offset_mm` 이 실제로 비-0 인가** | `s3` → `render_offset_presence` + §5-1 의 Console 줄 | **아래 §6.1** |
| **SC-62 의 다섯 값** — `max_deviation_mm` · `samples` · `tick_span` · `a_net_displacement_m` · `tolerance_m_passed_in` | `compare` → `SC-62` | 계약 15차 방법 칸이 다섯을 다 요구한다. **판정량은 끝점 거리가 아니라 최대 이탈**이다(끝점 거리는 왕복 이탈을 0 으로 읽는다). `exceeds_one_lsb_1mm` 이 true 면 **통과하더라도 한 문장으로 설명한다** |
| **`recorded_row_gaps_ticks`** | `compare` → `SC-62` | **정상은 2.** 크면 창 안쪽에서 기록이 끊긴 것이고, **(i) 은 그것을 통과시킨다**(§4 의 경고 상자). 판정하지 않고 값을 적는다 |
| **SC-62 방어 (i)(ii)(iii)** | `compare` → `SC-62.defense_i_sampling.ok` · `defense_ii_contrast.ok` · `tolerance_m_passed_in` | 각각 `미검증(표본 없음)` · `미검증(대조 없음)` · `미검증(판정 기준 미지정)` 을 낸다. **(i) 이 §4 의 조종 시간을 결정한다** — 아래 |

### 6.1 `render_offset_mm` 전 행 0 — **세 경우를 값으로 구분할 수 없다**

architect 가 R13 §5.2 에 새로 넣은 줄이다. 열은 들어갔지만 **그 열에 무엇이 실리는지는 아직
아무도 실행으로 보지 않았다.** 전 행 0 이면 원인이 셋이고, CSV 만으로는 갈리지 않는다.
**§5-1 의 종료 로그 세 숫자가 가른다:**

| 관측 | 원인 | 처분 |
|---|---|---|
| `render_smooth_band_total == 0` | **(i) 보정이 한 번도 안 일어났다** — 평활화 밴드에 드는 재조정이 없었다. 구현 결함이 아니다 | SC-64 는 그대로 판정. **"이 세션은 평활화 경로를 밟지 않았다"를 리포트에 적는다** — 그 열이 이 세션에서 아무것도 증명하지 않았다는 뜻이다 |
| `render_smooth_band_total > 0` **이고** `render_offset_nonzero_frames_total == 0` | **client 가 오프셋을 세우지 않았다** | client 에게 수정 요청 (FAIL) |
| `render_offset_nonzero_frames_total > 0` **이고** `render_offset_decay_frames_total == 0` | **(ii) `Update()` 감쇠 누락** — 오프셋을 세워 놓고 줄이지 않는다. SC-56 (e) ④ 가 겨냥한 결함 | client 에게 수정 요청 (FAIL) |
| **세 숫자가 전부 찍히지 않는다**(로그 줄 자체가 없다) | **(iii) client 미구현** — 또는 Play 를 정상 종료하지 않았다 | 먼저 §5-1 을 다시 확인. 정상 종료였는데도 없으면 미구현 |

**그리고 `render_offset_nonzero_frames_total > 0` 인데 CSV 의 `render_offset_mm` 이 전 행 0 이면
그 자체가 결함이다** — 프레임에는 오프셋이 섰는데 **스냅샷을 쓰는 순간에만 0** 이라는 뜻이고,
그러면 SC-64·65 가 재는 것은 화면이 아니다(12차 개정이 막으려던 바로 그 형태).

---

## 7. 이 세션이 닫지 **못하는** 것

- **SC-63** — 블록 10(봇 2대). 이 CSV 에는 원시 층이 한 열도 없다.
- **SC-59** — 블록 6. HUD 가 필요하고 이 하네스에는 HUD 가 없다(§0).
- 육안 판정 일반 — 카메라가 없다.
