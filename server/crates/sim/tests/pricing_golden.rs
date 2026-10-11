//! S3b — golden: `pricing_version` 이 시장 파일 가격 절의 정규화 해시 + 고정 입력
//! 대금 표에 묶인다(스펙 I-90, ADR-0014 §5 와 같은 방식 — `starfall-history` 의
//! `mineral_discovery.rs::golden` 모듈을 그대로 본뜬다).
//!
//! **값의 원천은 파일 하나다**(원칙 9): `data/market/cradle-exchange.json` 을 직접
//! 읽는다. `starfall_sim::trading::price` 자체에는 이 값의 두 번째 사본이 없다
//! (`PriceRule` 은 호출자가 채우는 평범한 구조체다) — 그래서 "코드에만 있는 값이
//! 파일과 갈라지는" 종류의 드리프트는 애초에 생기지 않는다. golden 이 잡는 것은 그보다
//! 좁다: **파일 값이 바뀌었는데 `pricing_version` 을 안 올린 경우**.
#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use starfall_contracts::data::MarketTable;
use starfall_sim::{PriceRule, sell};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn market_path() -> PathBuf {
    repo_root().join("data/market/cradle-exchange.json")
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{} 읽기 실패: {e}", path.display()))
}

fn market() -> MarketTable {
    let text = read_text(&market_path());
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} 파싱 실패: {e}", market_path().display()))
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pricing_golden")
}

/// 정규화: **원본 JSON**(파싱한 Rust 타입이 아니다)에서 설명문(`designer_note`·
/// `rules.note`)을 빼고 키 정렬 정규형으로 재직렬화해 해시한다 — `history` 의
/// `normalise_and_hash` 와 같은 이유(Debug 포맷이 아니라 값에만 반응해야 한다).
fn normalise_and_hash(raw_json: &str) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(raw_json).expect("시장 파일이 유효한 JSON 이어야 한다");
    if let Some(obj) = value.as_object_mut() {
        obj.remove("designer_note");
        if let Some(rules) = obj.get_mut("rules").and_then(|r| r.as_object_mut()) {
            rules.remove("note");
        }
    }
    let canonical = serde_json::to_string(&value).expect("정규화된 값의 재직렬화는 항상 성공한다");
    let digest = Sha256::digest(canonical.as_bytes());
    format!("{digest:x}")
}

/// 광물마다 `floor_band`·band 0~7 의 가격·고정 입력 대금(band_kg·2×band_kg)을
/// 결정적으로 뽑는다. `data/` 값으로만 만들어지는 **고정 입력**이다 — 디자인 §3.6
/// 상수표(§0.6 P1~P10)와는 다른, golden 전용 probe.
fn fixed_input_output(table: &MarketTable) -> Vec<String> {
    table
        .listings
        .iter()
        .map(|listing| {
            let rule = PriceRule {
                base_price_cr_per_kg: listing.base_price_cr_per_kg.get(),
                band_kg: i64::from(listing.band_kg.get()),
                price_floor_pct: table.rules.price_floor_pct,
                price_drop_pct_per_band: table.rules.price_drop_pct_per_band,
            };
            let prices: Vec<String> = (0..=rule.floor_band())
                .map(|b| rule.price_at_band(b).to_string())
                .collect();
            let one_band = sell(&rule, 0, rule.band_kg);
            let two_bands = sell(&rule, 0, 2 * rule.band_kg);
            format!(
                "{}|floor_band={}|prices=[{}]|sell(0,band_kg)={}|sell(0,2*band_kg)={}",
                listing.mineral_id.as_str(),
                rule.floor_band(),
                prices.join(","),
                one_band.proceeds_cr,
                two_bands.proceeds_cr,
            )
        })
        .collect()
}

#[test]
fn pricing_golden_binds_version_to_hash_and_table() {
    let raw_json = read_text(&market_path());
    let table = market();
    assert!(
        table
            .pricing_version
            .as_str()
            .starts_with(&format!("{}@", table.id.as_str())),
        "pricing_version 은 market id + '@' 로 시작해야 한다(스펙 I-90)"
    );

    let content_hash = normalise_and_hash(&raw_json);
    let output = fixed_input_output(&table);

    let hash_path = golden_dir().join(format!("{}.hash.txt", table.pricing_version.as_str()));
    let output_path = golden_dir().join(format!("{}.output.txt", table.pricing_version.as_str()));

    if std::env::var("STARFALL_REPLAY_BLESS").as_deref() == Ok("1") {
        fs::create_dir_all(golden_dir()).expect("golden 디렉터리 생성 실패");
        fs::write(&hash_path, &content_hash).expect("golden 해시 쓰기 실패");
        fs::write(&output_path, output.join("\n")).expect("golden 출력 쓰기 실패");
    }

    let expected_hash = fs::read_to_string(&hash_path).unwrap_or_else(|e| {
        panic!(
            "golden 해시 파일이 없다({}): {e} — STARFALL_REPLAY_BLESS=1 로 먼저 만든다",
            hash_path.display()
        )
    });
    let expected_output = fs::read_to_string(&output_path)
        .unwrap_or_else(|e| panic!("golden 출력 파일이 없다({}): {e}", output_path.display()));

    assert_eq!(
        content_hash,
        expected_hash.trim(),
        "시장 파일 가격 절 내용이 golden 해시와 달라졌다 — pricing_version 을 올렸는가?"
    );
    assert_eq!(
        output.join("\n"),
        expected_output.trim_end(),
        "고정 입력에 대한 가격 규칙 출력이 golden 과 달라졌다"
    );
}

/// 음성 대조 — `pricing_version` 을 올리지 않고 가격 값만 바꾼 시장 파일은 golden
/// 해시 비교에서 **실제로** 실패해야 한다(golden 이 죽은 assert 가 아님을 관측한다).
/// 메모리 위의 `serde_json::Value` 만 바꾼다 — 실제 시장 파일은 읽기만 한다.
#[test]
fn pricing_golden_rejects_changed_price_without_version_bump() {
    let raw_json = read_text(&market_path());
    let before_hash = normalise_and_hash(&raw_json);

    let mut value: serde_json::Value =
        serde_json::from_str(&raw_json).expect("시장 파일이 유효한 JSON 이어야 한다");
    let pricing_version = value["pricing_version"]
        .as_str()
        .expect("pricing_version 은 문자열이어야 한다")
        .to_string();
    // 값만 바꾼다(pricing_version 은 그대로) — 첫 상장의 base_price_cr_per_kg.
    let listings = value["listings"]
        .as_array_mut()
        .expect("listings 는 배열이다");
    let first = listings.first_mut().expect("상장이 1개 이상이어야 한다");
    let current = first["base_price_cr_per_kg"]
        .as_i64()
        .expect("base_price_cr_per_kg 은 정수다");
    first["base_price_cr_per_kg"] = serde_json::json!(current + 1);
    let mutated_json = serde_json::to_string(&value).expect("직렬화는 항상 성공한다");

    let after_hash = normalise_and_hash(&mutated_json);
    assert_ne!(
        before_hash, after_hash,
        "값을 바꿨는데 정규화 해시가 그대로면 golden 이 그 변경을 못 잡는다"
    );

    let expected_hash_path = golden_dir().join(format!("{pricing_version}.hash.txt"));
    let golden_hash = fs::read_to_string(&expected_hash_path)
        .expect("golden 해시 파일이 있어야 한다(pricing_golden_binds_version_to_hash_and_table 이 먼저 만든다)");
    assert_ne!(
        after_hash,
        golden_hash.trim(),
        "가격 값이 바뀐 시장 파일의 해시가 golden 과 여전히 같다 — golden 이 드리프트를 못 잡는다"
    );
}

/// JSON 키 순서·공백이 달라도(값은 그대로) 같은 해시가 나와야 한다 — Debug 포맷이
/// 아니라 값에만 해시가 반응한다는 성질(history 의 같은 테스트와 짝).
#[test]
fn pricing_golden_hash_is_insensitive_to_json_key_order_and_whitespace() {
    let raw_json = read_text(&market_path());
    let canonical_hash = normalise_and_hash(&raw_json);

    let value: serde_json::Value =
        serde_json::from_str(&raw_json).expect("시장 파일이 유효한 JSON 이어야 한다");
    // 값은 그대로, 들여쓰기만 다른 재직렬화(serde_json 은 키 정렬로 쓰므로 이미
    // 키 순서는 고정이다 — 여기서는 공백만 바꾼다).
    let reordered_text =
        serde_json::to_string_pretty(&value).expect("pretty 직렬화는 항상 성공한다");
    assert_ne!(
        raw_json, reordered_text,
        "전제가 틀렸다 — 공백이 실제로 달라야 의미 있는 음성 대조다"
    );
    assert_eq!(normalise_and_hash(&reordered_text), canonical_hash);
}
