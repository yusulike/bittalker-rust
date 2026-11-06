//! Korean number converter: converts integers to Korean text for TTS
//! pronunciation. Direct port of `src/utils/korean_numbers.py`.

// 기본 숫자
const DIGITS: [&str; 10] = ["", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
// 자릿수 단위 (10, 100, 1000)
const SMALL_UNITS: [&str; 4] = ["", "십", "백", "천"];
// 큰 단위 (만, 억, 조)
const BIG_UNITS: [&str; 5] = ["", "만", "억", "조", "경"];

/// Convert an integer to Korean text.
///
/// Examples:
///     95500 → "구만 오천오백"
///     1234 → "천이백삼십사"
///     100000 → "십만"
pub fn number_to_korean(num: i64) -> String {
    if num == 0 {
        return "영".to_string();
    }
    if num < 0 {
        return format!("마이너스 {}", number_to_korean(-num));
    }

    let mut result: Vec<String> = Vec::new();

    // Split into groups of 4 digits (만, 억, 조...)
    let mut groups: Vec<u32> = Vec::new();
    let mut n = num as u64;
    while n > 0 {
        groups.push((n % 10000) as u32);
        n /= 10000;
    }

    let total_groups = groups.len();
    for (i, group) in groups.iter().rev().enumerate() {
        let group = *group;
        if group == 0 {
            continue;
        }
        let mut group_text = four_digits_to_korean(group);
        let big_unit_idx = total_groups - 1 - i;
        if big_unit_idx < BIG_UNITS.len() {
            group_text.push_str(BIG_UNITS[big_unit_idx]);
        }
        result.push(group_text);
    }

    result.join(" ").trim().to_string()
}

/// Convert a number 0-9999 to Korean.
fn four_digits_to_korean(mut num: u32) -> String {
    if num == 0 {
        return String::new();
    }

    let mut parts: Vec<String> = Vec::new();
    for i in 0..4 {
        let digit = num % 10;
        num /= 10;
        if digit == 0 {
            continue;
        }
        let unit = SMALL_UNITS[i];
        // "일"은 생략하는 경우가 있음 (십, 백, 천 앞에서)
        if digit == 1 && i > 0 {
            parts.push(unit.to_string());
        } else {
            parts.push(format!("{}{}", DIGITS[digit as usize], unit));
        }
    }
    parts.reverse();
    parts.join("")
}

/// Format a price for Korean TTS.
///
/// Example: 95500 → "구만 오천오백"
pub fn format_price_korean(price: i64) -> String {
    number_to_korean(price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        let cases: &[(i64, &str)] = &[
            (0, "영"),
            (1, "일"),
            (10, "십"),
            (15, "십오"),
            (100, "백"),
            (123, "백이십삼"),
            (1000, "천"),
            (1234, "천이백삼십사"),
            (10000, "일만"),
            (95500, "구만 오천오백"),
            (95450, "구만 오천사백오십"),
            (100000, "십만"),
            (123456, "십이만 삼천사백오십육"),
        ];
        for (n, expected) in cases {
            assert_eq!(&number_to_korean(*n), expected, "input {n}");
        }
    }

    #[test]
    fn negative_number() {
        assert_eq!(number_to_korean(-123), "마이너스 백이십삼");
    }
}
