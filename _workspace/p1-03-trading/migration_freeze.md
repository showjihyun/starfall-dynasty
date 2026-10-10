# p1-03-trading — 마이그레이션 동결 선언

규칙: ADR-0007 §5(p1-02 승계). 증거 DB `starfall` 로 서버를 띄우면 서버가 아직 적용되지 않은
마이그레이션을 **적용해 버리고**, 적용된 파일은 그 순간 동결된다(고치면 다음 기동이 체크섬
불일치로 막힌다). 그래서 **아직 적용되지 않은** 마이그레이션은 그 소유자가 아래에 한 줄을 적은
뒤에만 증거 DB 로 서버를 띄운다.

- 형식(한 줄, 이 형식 그대로): `frozen: NNNN — <소유자>, <YYYY-MM-DD>`
- 적는 사람: **그 마이그레이션 파일의 소유자만**(0004 는 serverdb3). 기동하는 사람이 대신 적지 않는다.
- 검사: `tests/e2e/server_boot.py` 의 모든 실서버 기동이 `_sqlx_migrations` 와 `server/migrations/*.sql`
  을 대조하고, 미적용 파일에 선언 줄이 없으면 exit 4 로 멈춘다.

0001~0003 은 p1-02 에서 이미 적용·동결(`_workspace/p1-02-mining/migration_freeze.md`).

## 0004 현재 상태 (serverdb3, 2026-10-06 갱신)

- SQL은 구조적으로 두 번 독립 검증됐다(serverdb3 스크래치 DB·architect3 별도 스크래치 DB, 둘 다
  18개 제약 이름·분모 일치 — `03_serverdb_impl.md` §1b, ADR-0016 §5).
- **S4의 `[DB]` 11개(SC-24·26·28·34~37·50·51, `trading_load_state.rs`·`trading_sale.rs`·
  `trading_duplicate.rs`) 전부 통과**(server3의 S3 완료 뒤 `apply_state_write`의 `Wallet`/
  `MarketState` 매치 암·`commit()`의 `MineralSold` 분기까지 끝내고 전부 그린 — `03_serverdb_impl.md`
  "S4 2부" 절, R3 자기 점검 포함). 아래 선언으로 동결한다.

## 선언

```
frozen: 0004 — serverdb3, 2026-10-06
```
