//! 증거 DB 읽기 전용 질의 — 계약이 "봇 수신 + SQL 행 수" 를 한 항목의 판정 입력으로 요구할 때
//! (SC-81 새 역사 기록 0 건, SC-82 디스폰 행·C 의 첫 접속). **쓰기는 하지 않는다**: 질의는
//! `SELECT` 로 시작해야 하고 `psql -v ON_ERROR_STOP=1` 로 돈다(오류를 빈 결과로 삼키지 않는다 —
//! `tests/e2e/db.py` 의 `psql_strict` 와 같은 규칙).
//!
//! 레포 루트에서 `docker compose exec -T postgres psql` 을 부른다(봇은 레포 루트에서 실행한다).

use std::process::Command;

/// 한 값(첫 행 첫 열). 행이 없으면 `Ok(None)`.
pub fn scalar(sql: &str) -> Result<Option<String>, String> {
    let rows = rows(sql)?;
    Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
}

/// 정수 한 값. 행이 없거나 정수가 아니면 오류 — 0 으로 바꾸지 않는다.
pub fn scalar_i64(sql: &str) -> Result<i64, String> {
    let v = scalar(sql)?.ok_or_else(|| format!("행이 없다: {sql}"))?;
    v.trim()
        .parse::<i64>()
        .map_err(|_| format!("정수가 아니다: {v:?} ← {sql}"))
}

/// 전체 행(열 구분자 `|`).
pub fn rows(sql: &str) -> Result<Vec<Vec<String>>, String> {
    let head = sql.trim_start().to_ascii_lowercase();
    if !(head.starts_with("select") || head.starts_with("with")) {
        return Err(format!("읽기 전용 질의만 허용한다: {sql}"));
    }
    let out = Command::new("docker")
        .args([
            "compose",
            "exec",
            "-T",
            "postgres",
            "psql",
            "-v",
            "ON_ERROR_STOP=1",
            "-U",
            "starfall",
            "-d",
            "starfall",
            "-At",
            "-F",
            "|",
            "-c",
            sql,
        ])
        .output()
        .map_err(|e| format!("docker 실행 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "psql 실패({:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split('|').map(str::to_owned).collect())
        .collect())
}

/// UUID 문자열이 SQL 에 들어가도 되는 모양인가(질의에 값을 끼울 때 쓴다 — 인용 없이 넣지 않는다).
pub fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_writes() {
        assert!(rows("DELETE FROM worlds").is_err());
        assert!(rows("update worlds set name='x'").is_err());
        assert!(is_uuid("01a0ed7b-8fa5-7634-9088-a85f1b6b644e"));
        assert!(!is_uuid("x'; drop table worlds; --"));
    }
}
