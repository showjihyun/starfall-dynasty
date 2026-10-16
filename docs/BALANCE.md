# 밸런스 시트

`cargo run -p server --release --example balance -- --write docs/BALANCE.md`로 생성한다(결정적이므로 규칙이나 수치를 바꾸면 이 표의 diff로 영향이 보인다).

각 칸은 함장 1명(모든 모듈 Lv)과 해당 NPC가 Vesta Front에서 550 m 거리로 시작해, 조준 사격하며 90초까지 싸운 결과다. `W 12s 84%` = 12초에 승리, 선체 84% 남음 · `L 31s` = 31초에 격침 · `- 70%` = 시간 내 승부 없음.

| 함선 | Lv | Raider L1 | Gunship L2 | Raider L3 x4 | Warden L3 | Warlord L4 |
|---|---|---|---|---|---|---|
| Wren | 1 | W 9s 100% | L 14s | L 5s | L 10s | L 3s |
| Wren | 5 | W 2s 100% | W 5s 100% | W 11s 100% | W 6s 100% | L 9s |
| Wren | 9 | W 1s 100% | W 1s 100% | W 3s 100% | W 2s 100% | W 8s 100% |
| Talon | 1 | W 4s 100% | W 10s 69% | L 6s | L 13s | L 3s |
| Talon | 5 | W 1s 100% | W 4s 100% | W 8s 100% | W 5s 100% | L 11s |
| Talon | 9 | W 1s 100% | W 1s 100% | W 3s 100% | W 1s 100% | W 7s 100% |
| Bastion | 1 | W 4s 100% | W 12s 88% | L 9s | W 18s 6% | L 4s |
| Bastion | 5 | W 1s 100% | W 2s 100% | W 6s 100% | W 3s 100% | L 16s |
| Bastion | 9 | W 1s 100% | W 1s 100% | W 2s 100% | W 1s 100% | W 4s 100% |
| Mule | 1 | W 8s 100% | L 19s | L 6s | L 14s | L 3s |
| Mule | 5 | W 2s 100% | W 5s 100% | W 12s 100% | W 6s 100% | L 12s |
| Mule | 9 | W 1s 100% | W 1s 100% | W 3s 100% | W 2s 100% | W 8s 100% |
| Seer | 1 | W 6s 100% | W 15s 61% | L 6s | L 14s | L 3s |
| Seer | 5 | W 1s 100% | W 4s 100% | W 11s 100% | W 6s 100% | L 12s |
| Seer | 9 | W 1s 100% | W 1s 100% | W 3s 100% | W 2s 100% | W 7s 100% |
